//! Browser intents bind a credential revision to its exact saved website.
use super::*;

impl Vaultwarden {
    pub fn browser_target(&self, id: &str) -> Result<(String, String), VaultError> {
        self.require_active()?;
        self.items
            .iter()
            .find(|item| {
                item.id == id && item.kind == "login" && !item.deleted && !item.url.is_empty()
            })
            .ok_or_else(|| {
                VaultError::new(
                    "invalid_item",
                    "This login is unavailable for browser filling.",
                )
            })
            .and_then(|item| {
                let mut target = crate::urls::validate_website(&item.url).map_err(|_| {
                    VaultError::new("invalid_website", "This login needs a valid HTTPS website.")
                })?;
                if target.fragment() == Some("") {
                    target.set_fragment(None);
                }
                Ok((target.to_string(), item.revision.clone()))
            })
    }

    /// The caller retains the vault mutex through credential delivery. Decrypt
    /// once so username/password cannot originate from different local revisions.
    pub async fn browser_credentials(
        &self,
        id: &str,
        target: &str,
        revision: &str,
    ) -> Result<(Zeroizing<String>, Zeroizing<String>), VaultError> {
        let current = self.browser_target(id)?;
        if current.0 != target || current.1 != revision {
            return Err(VaultError::new(
                "item_changed",
                "The login changed. Request browser filling again.",
            ));
        }
        let cipher = self
            .encrypted
            .iter()
            .find(|cipher| {
                cipher
                    .id
                    .as_ref()
                    .is_some_and(|value| value.to_string() == id)
                    && cipher.deleted_date.is_none()
            })
            .ok_or_else(|| VaultError::new("item_not_found", "This login is unavailable."))?;
        let mut view = self
            .client
            .vault()
            .ciphers()
            .decrypt(cipher.clone())
            .await
            .map_err(|_| decryption_error())?;
        let result = (|| {
            let username = extract_field(&mut view, Field::Username)?;
            let password = extract_field(&mut view, Field::Password)?;
            Ok((username, password))
        })();
        erase_transient_secrets(&mut view);
        self.require_active()?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires dedicated local Vaultwarden fixtures; set VEYRA_TEST_FIXTURE_FILE"]
    async fn actual_vaultwarden_browser_credentials_bind_target_revision_and_lock() {
        let fixture: Value = serde_json::from_slice(
            &std::fs::read(std::env::var("VEYRA_TEST_FIXTURE_FILE").unwrap()).unwrap(),
        )
        .unwrap();
        for account in fixture["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|account| account.get("twoFactorSeed").is_none())
            .take(2)
        {
            let mut vault = Vaultwarden::connect_inner(
                fixture["serverUrl"].as_str().unwrap().into(),
                account["email"].as_str().unwrap().into(),
                Zeroizing::new(account["password"].as_str().unwrap().into()),
                None,
                true,
            )
            .await
            .unwrap();
            let expected = &account["items"][0];
            let id = expected["cipherId"].as_str().unwrap();
            let (target, revision) = vault.browser_target(id).unwrap();
            let (username, password) = vault
                .browser_credentials(id, &target, &revision)
                .await
                .unwrap();
            assert!(
                *username == expected["username"].as_str().unwrap(),
                "synthetic username must match"
            );
            assert!(
                *password == expected["password"].as_str().unwrap(),
                "synthetic password must match"
            );
            assert!(
                matches!(vault.browser_credentials(id, "https://other.test/", &revision).await, Err(error) if error.code == "item_changed")
            );
            assert!(
                matches!(vault.browser_credentials(id, &target, "stale-revision").await, Err(error) if error.code == "item_changed")
            );
            // A concurrent edit/sync can replace local metadata without changing
            // the session generation. Its new URL/revision must revoke the intent.
            let item = vault.items.iter_mut().find(|item| item.id == id).unwrap();
            let original = item.url.clone();
            item.url = "https://other.test/".into();
            assert!(
                matches!(vault.browser_credentials(id, &target, &revision).await, Err(error) if error.code == "item_changed")
            );
            let item = vault.items.iter_mut().find(|item| item.id == id).unwrap();
            item.url = original;
            item.revision = "new-local-revision".into();
            assert!(
                matches!(vault.browser_credentials(id, &target, &revision).await, Err(error) if error.code == "item_changed")
            );
            vault.lock();
            assert!(
                matches!(vault.browser_credentials(id, &target, &revision).await, Err(error) if error.code == "locked")
            );
        }
    }
}
