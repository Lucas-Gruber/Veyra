//! Bounded Vaultwarden HTTP transport with official Bitwarden GPL cryptography.
//! Tokens, encrypted ciphers and SDK keys remain in Rust and are never written to disk.
use std::{collections::HashMap, num::NonZeroU32, time::Duration};

use bitwarden_api_api::models::{
    CipherType as ApiCipherType, PrivateKeysResponseModel, SyncResponseModel,
};
use bitwarden_core::{
    key_management::{
        MasterPasswordAuthenticationData, PrivateKeySlotId, SigningKeySlotId, SymmetricKeySlotId,
    },
    Client, ClientSettings, DeviceType,
};
use bitwarden_crypto::{Decryptable, EncString, Kdf, MasterKey, SymmetricCryptoKey};
use bitwarden_vault::{Cipher, CipherRepromptType, CipherType, CipherView, VaultClientExt};
use reqwest::{Client as HttpClient, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::{Host, Url};
use zeroize::{Zeroize, Zeroizing};

const MAX_IDENTITY_BYTES: usize = 256 * 1024;
const MAX_SYNC_BYTES: usize = 32 * 1024 * 1024;
const MAX_ITEMS: usize = 50_000;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    Username,
    Password,
    Totp,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoFactorInput {
    pub provider: u8,
    pub token: String,
}

/// Only safe, fixed descriptions cross IPC. Never serialize upstream errors or bodies.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultError {
    pub code: &'static str,
    pub message: &'static str,
    pub providers: Vec<u8>,
}

impl Drop for TokenResponse {
    fn drop(&mut self) {
        self.access_token.zeroize();
    }
}

impl VaultError {
    pub fn new(code: &'static str, message: &'static str) -> Self {
        Self {
            code,
            message,
            providers: vec![],
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemMetadata {
    pub id: String,
    pub name: String,
    pub username: String,
    pub domain: String,
    pub url: String,
    pub kind: &'static str,
    pub favorite: bool,
    pub folder: String,
    pub color: &'static str,
    pub initials: String,
    pub updated: String,
    pub tags: Vec<String>,
    pub has_password: bool,
    pub has_totp: bool,
    pub read_only: bool,
}

#[derive(Deserialize)]
struct Prelogin {
    #[serde(alias = "Kdf")]
    kdf: i64,
    #[serde(rename = "kdfIterations", alias = "KdfIterations")]
    iterations: i64,
    #[serde(rename = "kdfMemory", alias = "KdfMemory")]
    memory: Option<i64>,
    #[serde(rename = "kdfParallelism", alias = "KdfParallelism")]
    parallelism: Option<i64>,
}

impl Prelogin {
    /// Validate signed source integers before casting; limit both allocation and CPU cost.
    fn checked(&self) -> Result<Kdf, VaultError> {
        let rejected = || {
            VaultError::new(
                "unsupported_kdf",
                "The server's key derivation settings exceed Veyra's supported safety limits.",
            )
        };
        match self.kdf {
            0 if (100_000..=2_000_000).contains(&self.iterations) => Ok(Kdf::PBKDF2 {
                iterations: NonZeroU32::new(self.iterations as u32).ok_or_else(rejected)?,
            }),
            1 if (2..=10).contains(&self.iterations)
                && self.memory.is_some_and(|n| (16..=256).contains(&n))
                && self.parallelism.is_some_and(|n| (1..=8).contains(&n)) =>
            {
                Ok(Kdf::Argon2id {
                    iterations: NonZeroU32::new(self.iterations as u32).ok_or_else(rejected)?,
                    memory: NonZeroU32::new(self.memory.ok_or_else(rejected)? as u32)
                        .ok_or_else(rejected)?,
                    parallelism: NonZeroU32::new(self.parallelism.ok_or_else(rejected)? as u32)
                        .ok_or_else(rejected)?,
                })
            }
            _ => Err(rejected()),
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    #[serde(alias = "Key")]
    key: String,
    #[serde(rename = "privateKey", alias = "PrivateKey")]
    private_key: String,
    #[serde(default, rename = "accountKeys", alias = "AccountKeys")]
    account_keys: Option<PrivateKeysResponseModel>,
    #[serde(default, rename = "resetMasterPassword", alias = "ResetMasterPassword")]
    reset_master_password: bool,
    #[serde(default, rename = "forcePasswordReset", alias = "ForcePasswordReset")]
    force_password_reset: bool,
}

fn invalid_response() -> VaultError {
    VaultError::new(
        "invalid_response",
        "The server returned an unsupported or malformed response.",
    )
}
fn network_error() -> VaultError {
    VaultError::new(
        "network_error",
        "Could not reach the server securely. Check its address, certificate and connection.",
    )
}
fn decryption_error() -> VaultError {
    VaultError::new(
        "decryption_failed",
        "Vault data could not be decrypted and verified. No changes were applied.",
    )
}

/// Server base URLs retain a possible subdirectory but may not carry credentials or queries.
fn server_url(input: &str, allow_loopback: bool) -> Result<Url, VaultError> {
    if input.len() > 2048 || input.chars().any(char::is_control) {
        return Err(VaultError::new(
            "invalid_server",
            "Enter a valid HTTPS Vaultwarden server address.",
        ));
    }
    let mut base = Url::parse(input).map_err(|_| {
        VaultError::new(
            "invalid_server",
            "Enter a valid HTTPS Vaultwarden server address.",
        )
    })?;
    let loopback = matches!(base.host(), Some(Host::Ipv4(ip)) if ip.is_loopback())
        || matches!(base.host(), Some(Host::Ipv6(ip)) if ip.is_loopback());
    if base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || !(base.scheme() == "https" || (allow_loopback && loopback && base.scheme() == "http"))
    {
        return Err(VaultError::new(
            "invalid_server",
            "An HTTPS server address without embedded credentials, query or fragment is required.",
        ));
    }
    let path = format!("{}/", base.path().trim_end_matches('/'));
    base.set_path(&path);
    Ok(base)
}

async fn bounded_body(
    mut response: Response,
    maximum: usize,
) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(VaultError::new(
            "response_too_large",
            "The server response exceeds Veyra's supported size limit.",
        ));
    }
    let mut body = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(|_| network_error())? {
        if body.len().saturating_add(chunk.len()) > maximum {
            return Err(VaultError::new(
                "response_too_large",
                "The server response exceeds Veyra's supported size limit.",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub struct Vaultwarden {
    client: Client,
    http: HttpClient,
    server: Url,
    access_token: Zeroizing<String>,
    expires_at: std::time::Instant,
    encrypted: Vec<Cipher>,
    items: Vec<ItemMetadata>,
    /// Organization and newer unsupported item types are outside this read-only milestone.
    pub skipped_items: usize,
}

impl Vaultwarden {
    pub async fn connect(
        server: String,
        email: String,
        password: Zeroizing<String>,
        two_factor: Option<TwoFactorInput>,
    ) -> Result<Self, VaultError> {
        let allow_loopback = cfg!(debug_assertions)
            && std::env::var("VEYRA_ALLOW_LOOPBACK_TEST_SERVER").is_ok_and(|v| v == "1");
        Self::connect_inner(server, email, password, two_factor, allow_loopback).await
    }

    async fn connect_inner(
        server: String,
        email: String,
        password: Zeroizing<String>,
        two_factor: Option<TwoFactorInput>,
        allow_loopback: bool,
    ) -> Result<Self, VaultError> {
        let server = server_url(&server, allow_loopback)?;
        let email = email.trim().to_lowercase();
        if email.len() > 320
            || !email.contains('@')
            || email.chars().any(char::is_control)
            || password.is_empty()
            || password.len() > 4096
        {
            return Err(VaultError::new(
                "invalid_login",
                "Enter your account email and master password.",
            ));
        }
        let http = HttpClient::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Veyra/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| network_error())?;
        // This is the ONLY prelogin fetch. Checked parameters below are used for both
        // authentication and unwrapping. SDK login_password would fetch again unchecked.
        let prelogin_response = http
            .post(
                server
                    .join("identity/accounts/prelogin")
                    .map_err(|_| invalid_response())?,
            )
            .json(&serde_json::json!({ "email": email }))
            .send()
            .await
            .map_err(|_| network_error())?;
        if !prelogin_response.status().is_success() {
            return Err(network_error());
        }
        let prelogin: Prelogin =
            serde_json::from_slice(&bounded_body(prelogin_response, MAX_IDENTITY_BYTES).await?)
                .map_err(|_| invalid_response())?;
        let kdf = prelogin.checked()?;
        let auth_password = password.clone();
        let auth_email = email.clone();
        let auth_kdf = kdf.clone();
        let authentication_hash = tokio::task::spawn_blocking(move || {
            MasterPasswordAuthenticationData::derive(&auth_password, &auth_kdf, &auth_email)
                .map(|data| Zeroizing::new(data.master_password_authentication_hash.to_string()))
                .map_err(|_| decryption_error())
        })
        .await
        .map_err(|_| decryption_error())??;
        let mut form = Zeroizing::new(Vec::<(String, String)>::new());
        for (name, value) in [
            ("grant_type", "password".to_owned()),
            ("username", email.clone()),
            ("password", authentication_hash.to_string()),
            ("scope", "api offline_access".to_owned()),
            ("client_id", "desktop".to_owned()),
            ("deviceType", "8".to_owned()),
            ("deviceName", "Veyra".to_owned()),
            ("deviceIdentifier", uuid::Uuid::new_v4().to_string()),
        ] {
            form.push((name.to_owned(), value));
        }
        if let Some(mut factor) = two_factor {
            // TOTP is the only verified second-factor flow in this milestone.
            if factor.provider != 0
                || factor.token.len() > 16
                || factor.token.is_empty()
                || !factor.token.bytes().all(|c| c.is_ascii_digit())
            {
                factor.token.zeroize();
                return Err(VaultError::new(
                    "unsupported_two_factor",
                    "Use an authenticator code. Other second-factor methods are not supported yet.",
                ));
            }
            form.push(("twoFactorProvider".into(), "0".into()));
            form.push(("twoFactorToken".into(), std::mem::take(&mut factor.token)));
            form.push(("twoFactorRemember".into(), "0".into()));
        }
        let token_requested_at = std::time::Instant::now();
        let token_response = http
            .post(
                server
                    .join("identity/connect/token")
                    .map_err(|_| invalid_response())?,
            )
            .form(&*form)
            .send()
            .await
            .map_err(|_| network_error())?;
        let token_status = token_response.status();
        let token_body = bounded_body(token_response, MAX_IDENTITY_BYTES).await?;
        if !token_status.is_success() {
            let challenge: Value =
                serde_json::from_slice(&token_body).map_err(|_| invalid_response())?;
            let providers = challenge
                .get("TwoFactorProviders2")
                .or_else(|| challenge.get("twoFactorProviders2"))
                .and_then(Value::as_object);
            if let Some(providers) = providers {
                if providers.contains_key("0") {
                    return Err(VaultError {
                        code: "two_factor_required",
                        message: "Enter the code from your authenticator app.",
                        providers: vec![0],
                    });
                }
                return Err(VaultError::new(
                    "unsupported_two_factor",
                    "This account requires a second-factor method not supported by Veyra yet.",
                ));
            }
            return Err(VaultError::new(
                "authentication_failed",
                "Sign-in failed. Check your email, master password and authenticator code.",
            ));
        }
        let mut token: TokenResponse =
            serde_json::from_slice(&token_body).map_err(|_| invalid_response())?;
        if token.access_token.is_empty()
            || token.expires_in == 0
            || token.expires_in > 31_536_000
            || token.force_password_reset
            || token.reset_master_password
        {
            token.access_token.zeroize();
            return Err(VaultError::new(
                "unsupported_account",
                "This account requires a password change or an unsupported authentication flow.",
            ));
        }
        let wrapped_key = EncString::parse_strict(&token.key).map_err(|_| decryption_error())?;
        let private_key =
            EncString::parse_strict(&token.private_key).map_err(|_| decryption_error())?;
        if !matches!(private_key, EncString::Aes256Cbc_HmacSha256_B64 { .. })
            || !matches!(wrapped_key, EncString::Aes256Cbc_HmacSha256_B64 { .. })
            || token.account_keys.as_ref().is_some_and(|keys| {
                keys.signature_key_pair.is_some() || keys.security_state.is_some()
            })
        {
            return Err(VaultError::new(
                "unsupported_account_crypto",
                "This account uses an encryption format outside Veyra's tested compatibility boundary.",
            ));
        }
        let client = Client::new(Some(ClientSettings {
            identity_url: server
                .join("identity")
                .map_err(|_| invalid_response())?
                .to_string(),
            api_url: server
                .join("api")
                .map_err(|_| invalid_response())?
                .to_string(),
            user_agent: concat!("Veyra/", env!("CARGO_PKG_VERSION")).into(),
            device_type: DeviceType::LinuxDesktop,
            ..Default::default()
        }));
        // No SDK authentication/network operation is used: its default handler has no
        // tokens. All network traffic goes through the bounded transport above/below.
        client
            .flags()
            .load(HashMap::from([(
                "pm-34500-strict-cipher-decryption".into(),
                true,
            )]))
            .await;
        // The upstream setter logs and swallows state errors. Verify the value
        // rather than ever falling back to its lenient decryption default.
        if !client.flags().get().await.strict_cipher_decryption {
            return Err(VaultError::new(
                "crypto_unavailable",
                "Strict vault verification could not be initialized. Sign-in was cancelled.",
            ));
        }
        let crypto_client = client.clone();
        tokio::task::spawn_blocking(move || {
            // Use the official SDK derivation/unwrapping primitives directly so
            // the master password stays in a zeroizing buffer rather than being
            // copied into the SDK's ordinary String request model.
            let master_key =
                MasterKey::derive(&password, &email, &kdf).map_err(|_| decryption_error())?;
            let user_key = master_key
                .decrypt_user_key(wrapped_key)
                .map_err(|_| decryption_error())?;
            // This personal-vault boundary has no organization/sharing operation.
            // The encrypted privateKey envelope was parsed above for the account
            // format check; do not decrypt its contents or import/retain the unused
            // RSA private key. Initialize only the AES key with official SDK APIs,
            // preserving the V1 initializer's key-type and fresh-store requirements.
            if !matches!(user_key, SymmetricCryptoKey::Aes256CbcHmacKey(_)) {
                return Err(decryption_error());
            }
            let mut context = crypto_client.internal.get_key_store().context_mut();
            if context.has_symmetric_key(SymmetricKeySlotId::User)
                || context.has_private_key(PrivateKeySlotId::UserPrivateKey)
                || context.has_signing_key(SigningKeySlotId::UserSigningKey)
            {
                return Err(decryption_error());
            }
            let local_key = context.add_local_symmetric_key(user_key);
            context
                .persist_symmetric_key(local_key, SymmetricKeySlotId::User)
                .map_err(|_| decryption_error())
        })
        .await
        .map_err(|_| decryption_error())??;
        let mut vault = Self {
            client,
            http,
            server,
            access_token: Zeroizing::new(std::mem::take(&mut token.access_token)),
            expires_at: token_requested_at + Duration::from_secs(token.expires_in),
            encrypted: vec![],
            items: vec![],
            skipped_items: 0,
        };
        vault.sync().await?;
        Ok(vault)
    }

    pub fn metadata(&self) -> Vec<ItemMetadata> {
        self.items.clone()
    }

    pub fn expires_in(&self) -> Duration {
        self.expires_at
            .saturating_duration_since(std::time::Instant::now())
    }

    pub async fn sync(&mut self) -> Result<(), VaultError> {
        if self.access_token.is_empty() || std::time::Instant::now() >= self.expires_at {
            return Err(VaultError::new(
                "session_expired",
                "Your session has expired. Sign in again.",
            ));
        }
        let response = self
            .http
            .get(
                self.server
                    .join("api/sync")
                    .map_err(|_| invalid_response())?,
            )
            .bearer_auth(&*self.access_token)
            .send()
            .await
            .map_err(|_| network_error())?;
        if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
            return Err(VaultError::new(
                "session_expired",
                "Your session has expired. Sign in again.",
            ));
        }
        if !response.status().is_success() {
            return Err(network_error());
        }
        let body = bounded_body(response, MAX_SYNC_BYTES).await?;
        let sync: SyncResponseModel =
            serde_json::from_slice(&body).map_err(|_| invalid_response())?;
        let ciphers = sync.ciphers.ok_or_else(invalid_response)?;
        if ciphers.len() > MAX_ITEMS {
            return Err(VaultError::new(
                "vault_too_large",
                "This vault exceeds the supported item limit.",
            ));
        }
        let mut folder_names = HashMap::new();
        for folder in sync.folders.unwrap_or_default() {
            let id = folder.id.ok_or_else(invalid_response)?;
            let name =
                EncString::parse_strict(folder.name.as_deref().ok_or_else(invalid_response)?)
                    .map_err(|_| decryption_error())?;
            let mut context = self.client.internal.get_key_store().context();
            let name: String = name
                .decrypt(&mut context, SymmetricKeySlotId::User)
                .map_err(|_| decryption_error())?;
            folder_names.insert(id, name);
        }
        let mut next_encrypted = Vec::with_capacity(ciphers.len());
        let mut next_items = Vec::with_capacity(ciphers.len());
        let mut skipped_items = 0;
        for model in ciphers {
            // Unknown future item types and organization items are deliberately
            // excluded before SDK conversion; their count is visible to the UI.
            if model.organization_id.is_some()
                || matches!(
                    model.r#type,
                    Some(
                        ApiCipherType::SSHKey
                            | ApiCipherType::BankAccount
                            | ApiCipherType::DriversLicense
                            | ApiCipherType::Passport
                            | ApiCipherType::__Unknown(_)
                    )
                )
            {
                skipped_items += 1;
                continue;
            }
            let cipher = Cipher::try_from(model).map_err(|_| decryption_error())?;
            if cipher.deleted_date.is_some() {
                continue;
            }
            let kind = match cipher.r#type {
                CipherType::Login => "login",
                CipherType::SecureNote => "note",
                CipherType::Card => "card",
                CipherType::Identity => "identity",
                _ => {
                    skipped_items += 1;
                    continue;
                }
            };
            let mut view = self
                .client
                .vault()
                .ciphers()
                .decrypt(cipher.clone())
                .await
                .map_err(|_| decryption_error())?;
            let result = project_metadata(&view, kind);
            erase_transient_secrets(&mut view);
            let mut metadata = result?;
            if let Some(id) = cipher.folder_id {
                metadata.folder = folder_names
                    .get(&id.into())
                    .cloned()
                    .unwrap_or_else(|| "Personal vault".into());
            }
            next_items.push(metadata);
            next_encrypted.push(cipher);
        }
        self.require_active()?;
        // Commit only after the complete supported personal vault has parsed/decrypted.
        self.encrypted = next_encrypted;
        self.items = next_items;
        self.skipped_items = skipped_items;
        Ok(())
    }

    pub async fn field(&self, id: &str, field: Field) -> Result<Zeroizing<String>, VaultError> {
        self.require_active()?;
        let cipher = self
            .encrypted
            .iter()
            .find(|cipher| {
                cipher
                    .id
                    .as_ref()
                    .is_some_and(|value| value.to_string() == id)
            })
            .ok_or_else(|| VaultError::new("item_not_found", "This item is unavailable."))?;
        let mut view = self
            .client
            .vault()
            .ciphers()
            .decrypt(cipher.clone())
            .await
            .map_err(|_| decryption_error())?;
        let result = extract_field(&mut view, field);
        erase_transient_secrets(&mut view);
        self.require_active()?;
        result
    }

    pub fn url(&self, id: &str) -> Result<String, VaultError> {
        self.require_active()?;
        self.items
            .iter()
            .find(|item| item.id == id && !item.url.is_empty())
            .map(|item| item.url.clone())
            .ok_or_else(|| {
                VaultError::new(
                    "invalid_website",
                    "This item has no supported HTTPS website.",
                )
            })
    }

    fn require_active(&self) -> Result<(), VaultError> {
        if self.access_token.is_empty() {
            return Err(VaultError::new("locked", "Unlock your vault first."));
        }
        if std::time::Instant::now() >= self.expires_at {
            return Err(VaultError::new(
                "session_expired",
                "Your session has expired. Sign in again.",
            ));
        }
        Ok(())
    }

    pub fn lock(&mut self) {
        self.client.internal.get_key_store().clear();
        self.access_token.zeroize();
        self.items.clear();
        self.encrypted.clear();
    }
}

impl Drop for Vaultwarden {
    fn drop(&mut self) {
        self.lock();
    }
}

fn project_metadata(view: &CipherView, kind: &'static str) -> Result<ItemMetadata, VaultError> {
    let id = view.id.ok_or_else(invalid_response)?.to_string();
    let login = view.login.as_ref();
    let url = login
        .and_then(|login| login.uris.as_ref())
        .and_then(|uris| {
            uris.iter().find_map(|entry| {
                let parsed = Url::parse(entry.uri.as_deref()?).ok()?;
                (parsed.scheme() == "https"
                    && parsed.host_str().is_some()
                    && parsed.username().is_empty()
                    && parsed.password().is_none())
                .then(|| parsed.to_string())
            })
        })
        .unwrap_or_default();
    let domain = Url::parse(&url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_default();
    let permitted = view.view_password && matches!(view.reprompt, CipherRepromptType::None);
    Ok(ItemMetadata {
        id,
        name: view.name.clone(),
        username: login.and_then(|v| v.username.clone()).unwrap_or_default(),
        domain,
        url,
        kind,
        favorite: view.favorite,
        folder: "Personal vault".into(),
        color: "violet",
        initials: view
            .name
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(2)
            .collect::<String>()
            .to_uppercase(),
        updated: view.revision_date.format("%Y-%m-%d").to_string(),
        tags: vec![],
        has_password: permitted && login.is_some_and(|v| v.password.is_some()),
        has_totp: permitted && login.is_some_and(|v| v.totp.is_some()),
        read_only: true,
    })
}

fn extract_field(view: &mut CipherView, field: Field) -> Result<Zeroizing<String>, VaultError> {
    if !matches!(field, Field::Username)
        && (!view.view_password || !matches!(view.reprompt, CipherRepromptType::None))
    {
        return Err(VaultError::new(
            "reprompt_required",
            "This field requires an additional unlock method that Veyra does not support yet.",
        ));
    }
    let login = view.login.as_mut().ok_or_else(|| {
        VaultError::new(
            "field_unavailable",
            "This item does not contain that login field.",
        )
    })?;
    let value = match field {
        Field::Username => login.username.take(),
        Field::Password => login.password.take(),
        Field::Totp => {
            let key = Zeroizing::new(login.totp.take().ok_or_else(|| {
                VaultError::new(
                    "field_unavailable",
                    "This item has no authenticator secret.",
                )
            })?);
            Some(
                bitwarden_vault::generate_totp(key.to_string(), None)
                    .map_err(|_| {
                        VaultError::new(
                            "invalid_totp",
                            "This item's authenticator configuration is unsupported.",
                        )
                    })?
                    .code,
            )
        }
    };
    value.map(Zeroizing::new).ok_or_else(|| {
        VaultError::new(
            "field_unavailable",
            "This item does not contain that login field.",
        )
    })
}

/// SDK models contain ordinary strings. Reduce common plaintext lifetimes; this
/// cannot promise erasure of IPC/allocator/SDK-internal copies or third-party data.
fn erase_transient_secrets(view: &mut CipherView) {
    if let Some(login) = view.login.as_mut() {
        login.password.zeroize();
        login.totp.zeroize();
        if let Some(credentials) = login.fido2_credentials.as_mut() {
            for credential in credentials {
                credential.key_value.zeroize();
            }
        }
    }
    if let Some(card) = view.card.as_mut() {
        card.cardholder_name.zeroize();
        card.number.zeroize();
        card.code.zeroize();
        card.exp_month.zeroize();
        card.exp_year.zeroize();
        card.brand.zeroize();
    }
    if let Some(identity) = view.identity.as_mut() {
        identity.title.zeroize();
        identity.first_name.zeroize();
        identity.middle_name.zeroize();
        identity.last_name.zeroize();
        identity.address1.zeroize();
        identity.address2.zeroize();
        identity.address3.zeroize();
        identity.city.zeroize();
        identity.state.zeroize();
        identity.postal_code.zeroize();
        identity.country.zeroize();
        identity.company.zeroize();
        identity.email.zeroize();
        identity.phone.zeroize();
        identity.ssn.zeroize();
        identity.username.zeroize();
        identity.passport_number.zeroize();
        identity.license_number.zeroize();
    }
    if let Some(ssh_key) = view.ssh_key.as_mut() {
        ssh_key.private_key.zeroize();
    }
    view.notes.zeroize();
    if let Some(fields) = view.fields.as_mut() {
        for field in fields {
            field.value.zeroize();
        }
    }
    if let Some(history) = view.password_history.as_mut() {
        for item in history {
            item.password.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kdf_bounds_reject_negative_zero_unknown_and_excessive_work() {
        for json in [
            r#"{"kdf":0,"kdfIterations":0}"#,
            r#"{"kdf":0,"kdfIterations":-1}"#,
            r#"{"kdf":0,"kdfIterations":2000001}"#,
            r#"{"kdf":1,"kdfIterations":3,"kdfMemory":257,"kdfParallelism":4}"#,
            r#"{"kdf":1,"kdfIterations":11,"kdfMemory":64,"kdfParallelism":4}"#,
            r#"{"kdf":1,"kdfIterations":3,"kdfMemory":64,"kdfParallelism":9}"#,
            r#"{"kdf":1,"kdfIterations":3,"kdfMemory":-64,"kdfParallelism":4}"#,
            r#"{"kdf":2,"kdfIterations":600000}"#,
        ] {
            assert!(serde_json::from_str::<Prelogin>(json)
                .unwrap()
                .checked()
                .is_err());
        }
        for json in [
            r#"{"kdf":0,"kdfIterations":600000}"#,
            r#"{"kdf":1,"kdfIterations":3,"kdfMemory":64,"kdfParallelism":4}"#,
        ] {
            assert!(serde_json::from_str::<Prelogin>(json)
                .unwrap()
                .checked()
                .is_ok());
        }
    }

    #[test]
    fn release_server_policy_requires_https_and_preserves_subdirectory() {
        for url in [
            "http://vault.example",
            "http://127.0.0.1:8222",
            "https://user:pass@vault.example",
            "https://vault.example?x=1",
            "https://vault.example/#x",
            "https://vault.example\n",
        ] {
            assert!(server_url(url, false).is_err());
        }
        assert!(server_url("http://localhost:8222", true).is_err());
        assert!(server_url("http://127.0.0.1:8222", true).is_ok());
        let base = server_url("https://vault.example/subdir", false).unwrap();
        assert_eq!(
            base.join("api/sync").unwrap().as_str(),
            "https://vault.example/subdir/api/sync"
        );
    }

    #[test]
    fn additional_server_fields_do_not_break_supported_models() {
        let response: Prelogin = serde_json::from_str(
            r#"{"kdf":0,"kdfIterations":600000,"futureServerSetting":{"version":99}}"#,
        )
        .unwrap();
        assert!(response.checked().is_ok());
        let sync: SyncResponseModel =
            serde_json::from_str(r#"{"ciphers":[],"futureSyncField":true}"#).unwrap();
        assert!(sync.ciphers.unwrap().is_empty());
    }

    fn single_response_server(response: impl AsRef<[u8]>) -> Url {
        use std::io::{Read, Write};
        let response = response.as_ref().to_vec();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request);
            stream.write_all(&response).unwrap();
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }

    async fn http_response(response: impl AsRef<[u8]>) -> Response {
        HttpClient::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(single_response_server(response))
            .send()
            .await
            .unwrap()
    }

    fn assert_personal_key_store(vault: &Vaultwarden) {
        let context = vault.client.internal.get_key_store().context();
        assert!(
            context.has_symmetric_key(SymmetricKeySlotId::User),
            "the personal AES key must be available"
        );
        assert!(
            !context.has_private_key(PrivateKeySlotId::UserPrivateKey),
            "personal-only sessions must not retain the unused account RSA key"
        );
        assert!(
            !context.has_signing_key(SigningKeySlotId::UserSigningKey),
            "personal-only sessions must not initialize unsupported signing keys"
        );
    }

    #[tokio::test]
    async fn responses_are_bounded_for_declared_and_chunked_bodies() {
        let declared = http_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789",
        )
        .await;
        assert!(
            matches!(bounded_body(declared, 8).await, Err(error) if error.code == "response_too_large")
        );
        let chunked = http_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n9\r\n123456789\r\n0\r\n\r\n").await;
        assert!(
            matches!(bounded_body(chunked, 8).await, Err(error) if error.code == "response_too_large")
        );
    }

    #[tokio::test]
    #[ignore = "requires dedicated local Vaultwarden fixtures; set VEYRA_TEST_FIXTURE_FILE"]
    async fn actual_vaultwarden_pbkdf2_and_argon2_login_sync_and_decrypt() {
        let path =
            std::env::var("VEYRA_TEST_FIXTURE_FILE").expect("set path to synthetic fixture file");
        let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let server = fixture["serverUrl"].as_str().unwrap();
        let accounts = fixture["accounts"].as_array().unwrap();
        let accounts: Vec<_> = accounts
            .iter()
            .filter(|account| account.get("twoFactorSeed").is_none())
            .collect();
        assert!(
            accounts.iter().any(|account| account["kdf"]["kdf"] == 0),
            "PBKDF2 account required"
        );
        assert!(
            accounts.iter().any(|account| account["kdf"]["kdf"] == 1),
            "Argon2id account required"
        );
        for account in accounts {
            let email = account["email"].as_str().unwrap();
            let password = account["password"].as_str().unwrap();
            let mut vault = Vaultwarden::connect_inner(
                server.into(),
                email.into(),
                Zeroizing::new(password.into()),
                None,
                true,
            )
            .await
            .expect("real server login and complete supported sync");
            assert_personal_key_store(&vault);
            assert!(
                vault.client.flags().get().await.strict_cipher_decryption,
                "strict authenticated decryption must be active"
            );
            let expected_items = account["items"]
                .as_array()
                .expect("both user-key and per-cipher-key fixtures required");
            assert!(expected_items.len() >= 2);
            assert_eq!(vault.metadata().len(), expected_items.len());
            for expected in expected_items {
                let id = expected["cipherId"].as_str().unwrap();
                let item = vault
                    .metadata()
                    .into_iter()
                    .find(|item| item.id == id)
                    .expect("actual item found");
                assert!(
                    item.name == expected["name"].as_str().unwrap(),
                    "item name differs"
                );
                assert!(
                    item.username == expected["username"].as_str().unwrap(),
                    "username differs"
                );
                let actual = vault
                    .field(id, Field::Password)
                    .await
                    .expect("actual password decrypted");
                assert!(
                    *actual == expected["password"].as_str().unwrap(),
                    "password differs"
                );
                let totp = vault
                    .field(id, Field::Totp)
                    .await
                    .expect("actual TOTP generated");
                assert!(totp.len() == 6 && totp.bytes().all(|byte| byte.is_ascii_digit()));
            }
            // A complete authenticated sync is transactional: if the server
            // supplies a corrupt supported cipher, retain the previous snapshot.
            let original_server = vault.server.clone();
            let response = vault
                .http
                .get(original_server.join("api/sync").unwrap())
                .bearer_auth(&*vault.access_token)
                .send()
                .await
                .unwrap();
            let body = bounded_body(response, MAX_SYNC_BYTES).await.unwrap();
            let valid_sync: Value = serde_json::from_slice(&body).unwrap();
            let before = serde_json::to_value(vault.metadata()).unwrap();
            // A hostile response must not switch symmetric credential/key
            // decryption to an RSA envelope (types 3..6), or mutate the snapshot.
            for (field, value) in [
                ("name", "2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
                ("name", "3.AA=="),
                ("key", "4.AA=="),
                ("name", "5.AA==|AA=="),
                ("key", "6.AA==|AA=="),
            ] {
                let mut malformed = valid_sync.clone();
                malformed["ciphers"][1][field] = Value::String(value.into());
                let body = serde_json::to_vec(&malformed).unwrap();
                let mut wire = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
                wire.extend_from_slice(&body);
                vault.server = single_response_server(wire);
                assert!(matches!(vault.sync().await, Err(error) if error.code == "decryption_failed"));
                assert!(
                    before == serde_json::to_value(vault.metadata()).unwrap(),
                    "failed refresh must preserve the complete metadata snapshot"
                );
                assert_personal_key_store(&vault);
                let preserved_password = vault
                    .field(
                        expected_items[0]["cipherId"].as_str().unwrap(),
                        Field::Password,
                    )
                    .await
                    .expect("the original stored cipher must remain decryptable");
                assert!(
                    *preserved_password == expected_items[0]["password"].as_str().unwrap(),
                    "failed refresh must preserve the original stored password"
                );
            }
            vault.server = original_server;
            // Manual refresh performs another actual authenticated request.
            vault.sync().await.expect("second synchronization succeeds");
            assert_personal_key_store(&vault);
            let id = expected_items[0]["cipherId"].as_str().unwrap();
            vault.expires_at = std::time::Instant::now() - Duration::from_secs(1);
            assert!(
                matches!(vault.field(id, Field::Password).await, Err(error) if error.code == "session_expired")
            );
            vault.lock();
            assert!(
                !vault
                    .client
                    .internal
                    .get_key_store()
                    .context()
                    .has_symmetric_key(SymmetricKeySlotId::User),
                "explicit locking must remove the personal AES key"
            );
            assert!(vault.metadata().is_empty());
            assert!(vault.field(id, Field::Password).await.is_err());
            let wrong = Vaultwarden::connect_inner(
                server.into(),
                email.into(),
                Zeroizing::new("incorrect-fixture-password".into()),
                None,
                true,
            )
            .await;
            assert!(matches!(wrong, Err(error) if error.code == "authentication_failed"));
        }
    }

    #[tokio::test]
    #[ignore = "requires dedicated Vaultwarden TOTP fixtures; set VEYRA_TEST_FIXTURE_FILE"]
    async fn actual_vaultwarden_totp_two_factor_challenge_rejection_and_login() {
        let path =
            std::env::var("VEYRA_TEST_FIXTURE_FILE").expect("set path to synthetic fixture file");
        let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let account = fixture["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|account| account.get("twoFactorSeed").is_some())
            .expect("enrolled TOTP fixture account required");
        let server = fixture["serverUrl"].as_str().unwrap();
        let email = account["email"].as_str().unwrap();
        let password = account["password"].as_str().unwrap();
        let challenge = Vaultwarden::connect_inner(
            server.into(),
            email.into(),
            Zeroizing::new(password.into()),
            None,
            true,
        )
        .await;
        assert!(
            matches!(challenge, Err(error) if error.code == "two_factor_required" && error.providers == vec![0])
        );

        // Seven digits passes our syntactic validation, but cannot equal this
        // six-digit authenticator fixture. This exercises the actual server.
        let wrong = Vaultwarden::connect_inner(
            server.into(),
            email.into(),
            Zeroizing::new(password.into()),
            Some(TwoFactorInput {
                provider: 0,
                token: "9999999".into(),
            }),
            true,
        )
        .await;
        assert!(matches!(wrong, Err(error) if error.code == "authentication_failed"));

        // Vaultwarden prevents replay of a successful authenticator step. Wait
        // for a fresh step because fixture enrollment or a preceding test run
        // may already have consumed the current one. This is bounded to 31s.
        let remaining = 30 - chrono::Utc::now().timestamp().rem_euclid(30) + 1;
        tokio::time::sleep(Duration::from_secs(remaining as u64)).await;
        let code = bitwarden_vault::generate_totp(
            account["twoFactorSeed"].as_str().unwrap().to_owned(),
            None,
        )
        .expect("fixture authenticator code generated")
        .code;
        let mut vault = Vaultwarden::connect_inner(
            server.into(),
            email.into(),
            Zeroizing::new(password.into()),
            Some(TwoFactorInput {
                provider: 0,
                token: code,
            }),
            true,
        )
        .await
        .expect("actual authenticator login succeeds");
        assert_personal_key_store(&vault);
        let expected = &account["expected"];
        let id = expected["cipherId"].as_str().unwrap();
        assert!(vault.metadata().iter().any(|item| item.id == id));
        let actual = vault
            .field(id, Field::Password)
            .await
            .expect("actual protected field decrypted after 2FA");
        assert!(
            *actual == expected["password"].as_str().unwrap(),
            "password differs"
        );
        vault.lock();
        assert!(
            !vault
                .client
                .internal
                .get_key_store()
                .context()
                .has_symmetric_key(SymmetricKeySlotId::User),
            "explicit locking must remove the personal AES key"
        );
        assert!(vault.field(id, Field::Password).await.is_err());
    }
}
