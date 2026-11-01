//! Authenticated mutations. SDK crypto only; never retry a dispatched mutation.
use super::*;
use bitwarden_api_api::models::CipherRequestModel;
use bitwarden_vault::{
    CardView, IdentityView, LoginUriView, LoginView, PasswordHistoryView, SecureNoteType,
    SecureNoteView,
};
use chrono::Utc;
mod management;
pub(super) use management::management_unsupported_ids;
pub use management::{ManageItemsRequest, ManagedItemResult};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemDraft {
    pub id: Option<String>,
    pub revision: Option<String>,
    pub organization_id: Option<String>,
    #[serde(default)]
    pub collection_ids: Vec<String>,
    pub kind: String,
    pub name: String,
    pub favorite: bool,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub totp: String,
    #[serde(default)]
    pub uri: String,
    pub card: Option<CardView>,
    pub identity: Option<IdentityView>,
}

impl Drop for ItemDraft {
    fn drop(&mut self) {
        self.name.zeroize();
        self.notes.zeroize();
        self.username.zeroize();
        self.password.zeroize();
        self.totp.zeroize();
        self.uri.zeroize();
        if let Some(card) = self.card.as_mut() {
            card.cardholder_name.zeroize();
            card.number.zeroize();
            card.code.zeroize();
            card.exp_month.zeroize();
            card.exp_year.zeroize();
            card.brand.zeroize();
        }
        if let Some(identity) = self.identity.as_mut() {
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
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableItem {
    pub draft: ItemDraft,
    pub generation: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeItemRequest {
    pub id: String,
    pub revision: String,
    pub action: ChangeAction,
    pub favorite: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeAction {
    Favorite,
    Trash,
    Restore,
}

struct TransientView(CipherView);
impl Drop for TransientView {
    fn drop(&mut self) {
        erase_transient_secrets(&mut self.0);
    }
}

fn can_read_contents(view: &CipherView) -> bool {
    view.view_password && !view.partial && matches!(view.reprompt, CipherRepromptType::None)
}

pub(super) fn can_edit(view: &CipherView) -> bool {
    view.edit && can_read_contents(view)
}
fn permission_error() -> VaultError {
    VaultError::new(
        "write_not_allowed",
        "This item is restricted or requires an unlock method Veyra does not support.",
    )
}
fn conflict() -> VaultError {
    VaultError::new(
        "item_changed",
        "This item has changed. Synchronize and reopen it before saving.",
    )
}
fn check_id(id: &str) -> Result<(), VaultError> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| invalid_response())
}

impl ItemDraft {
    fn validate(&self) -> Result<CipherType, VaultError> {
        let kind = match self.kind.as_str() {
            "login" => CipherType::Login,
            "note" => CipherType::SecureNote,
            "card" => CipherType::Card,
            "identity" => CipherType::Identity,
            _ => return Err(invalid_response()),
        };
        if self.name.trim().is_empty()
            || self.name.len() > 1024
            || self.notes.len() > 64 * 1024
            || [&self.username, &self.password, &self.totp]
                .iter()
                .any(|s| s.len() > 4096)
            || self.uri.len() > 8192
            || self.collection_ids.len() > 100
        {
            return Err(VaultError::new(
                "invalid_item",
                "Enter an item name and keep field sizes within the supported limits.",
            ));
        }
        if let Some(id) = self.id.as_ref() {
            check_id(id)?;
        }
        if let Some(id) = self.organization_id.as_ref() {
            check_id(id)?;
        }
        for id in &self.collection_ids {
            check_id(id)?;
        }
        // Bound the fixed SDK card/identity field sets, without serializing plaintext.
        if let Some(c) = self.card.as_ref() {
            if [
                &c.cardholder_name,
                &c.number,
                &c.code,
                &c.brand,
                &c.exp_month,
                &c.exp_year,
            ]
            .iter()
            .any(|s| s.as_ref().is_some_and(|s| s.len() > 4096))
            {
                return Err(invalid_response());
            }
        }
        if let Some(i) = self.identity.as_ref() {
            if [
                &i.title,
                &i.first_name,
                &i.middle_name,
                &i.last_name,
                &i.address1,
                &i.address2,
                &i.address3,
                &i.city,
                &i.state,
                &i.postal_code,
                &i.country,
                &i.company,
                &i.email,
                &i.phone,
                &i.ssn,
                &i.username,
                &i.passport_number,
                &i.license_number,
            ]
            .iter()
            .any(|s| s.as_ref().is_some_and(|s| s.len() > 4096))
            {
                return Err(invalid_response());
            }
        }
        Ok(kind)
    }
}

impl Vaultwarden {
    async fn decrypted_view(&self, id: &str) -> Result<TransientView, VaultError> {
        self.require_active()?;
        check_id(id)?;
        let cipher = self
            .encrypted
            .iter()
            .find(|cipher| cipher.id.is_some_and(|value| value.to_string() == id))
            .ok_or_else(|| VaultError::new("item_not_found", "This item is unavailable."))?;
        let view = TransientView(
            self.client
                .vault()
                .ciphers()
                .decrypt(cipher.clone())
                .await
                .map_err(|_| decryption_error())?,
        );
        self.require_active()?;
        Ok(view)
    }

    async fn editable_view(
        &self,
        id: &str,
        allow_deleted: bool,
    ) -> Result<TransientView, VaultError> {
        let view = self.decrypted_view(id).await?;
        if !can_edit(&view.0) || (!allow_deleted && view.0.deleted_date.is_some()) {
            return Err(permission_error());
        }
        Ok(view)
    }

    pub async fn item_contents(
        &self,
        id: &str,
        generation: u64,
    ) -> Result<EditableItem, VaultError> {
        let view = self.decrypted_view(id).await?;
        if !can_read_contents(&view.0) || view.0.deleted_date.is_some() {
            return Err(VaultError::new(
                "contents_not_allowed",
                "This item's contents are restricted or require an unsupported unlock method.",
            ));
        }
        Self::draft_from_view(id, &view.0, generation, false)
    }

    pub async fn editable_item(
        &self,
        id: &str,
        generation: u64,
    ) -> Result<EditableItem, VaultError> {
        let view = self.editable_view(id, false).await?;
        Self::draft_from_view(id, &view.0, generation, true)
    }

    fn draft_from_view(
        id: &str,
        v: &CipherView,
        generation: u64,
        editable: bool,
    ) -> Result<EditableItem, VaultError> {
        let login = v.login.as_ref();
        let draft = ItemDraft {
            id: Some(id.into()),
            revision: Some(v.revision_date.to_rfc3339()),
            organization_id: v.organization_id.map(|id| id.to_string()),
            collection_ids: v.collection_ids.iter().map(ToString::to_string).collect(),
            kind: match v.r#type {
                CipherType::Login => "login",
                CipherType::SecureNote => "note",
                CipherType::Card => "card",
                CipherType::Identity => "identity",
                _ => return Err(invalid_response()),
            }
            .into(),
            name: v.name.clone(),
            favorite: v.favorite,
            notes: v.notes.clone().unwrap_or_default(),
            username: login.and_then(|l| l.username.clone()).unwrap_or_default(),
            // Reading notes must not also reveal login secrets. The existing
            // field commands remain the explicit reveal/copy path for these.
            password: if editable {
                login.and_then(|l| l.password.clone()).unwrap_or_default()
            } else {
                String::new()
            },
            totp: if editable {
                login.and_then(|l| l.totp.clone()).unwrap_or_default()
            } else {
                String::new()
            },
            uri: login
                .and_then(|l| l.uris.as_ref())
                .and_then(|uris| uris.first())
                .and_then(|uri| uri.uri.clone())
                .unwrap_or_default(),
            card: v.card.clone(),
            identity: v.identity.clone(),
        };
        Ok(EditableItem { draft, generation })
    }

    fn require_write_ready(&self) -> Result<(), VaultError> {
        self.require_active()?;
        if self.write_needs_sync {
            return Err(VaultError::new("sync_required", "Synchronize the vault before another write. The previous request may have reached the server."));
        }
        Ok(())
    }

    async fn dispatch_write(
        &mut self,
        request: reqwest::RequestBuilder,
        has_response: bool,
    ) -> Result<Option<Value>, VaultError> {
        self.require_write_ready()?;
        // Mark before dispatch: cancellation/network errors cannot prove that the
        // server did not commit. Never automatically retry the mutation.
        self.write_needs_sync = true;
        let response = request.bearer_auth(&*self.access_token).send().await.map_err(|_| VaultError::new("write_uncertain", "The write result is unknown. Synchronize and inspect the vault before trying again."))?;
        let status = response.status();
        if status.as_u16() == 401 {
            return Err(VaultError::new(
                "session_expired",
                "Your session has expired. Sign in again.",
            ));
        }
        if !status.is_success() {
            if status.is_server_error() {
                return Err(VaultError::new("write_uncertain", "The server reported an error. Synchronize and inspect the vault before trying again."));
            }
            self.write_needs_sync = false;
            return Err(if status.as_u16() == 403 {
                permission_error()
            } else {
                VaultError::new("write_rejected", "The server rejected the change. Synchronize and reopen the item; check your permissions and account policies.")
            });
        }
        let body = bounded_body(response, MAX_IDENTITY_BYTES).await.map_err(|_| VaultError::new("write_uncertain", "The server accepted the change, but its reply could not be verified. Synchronize before trying again."))?;
        if has_response {
            Ok(Some(serde_json::from_slice(&body).map_err(|_| VaultError::new("write_uncertain", "The change may be saved. Synchronize and inspect the vault before trying again."))?))
        } else {
            Ok(None)
        }
    }

    pub async fn save_item(&mut self, draft: ItemDraft) -> Result<(String, bool), VaultError> {
        self.require_write_ready()?;
        let kind = draft.validate()?;
        let mut view = if let Some(id) = draft.id.as_ref() {
            let view = self.editable_view(id, false).await?;
            if draft.revision.as_deref() != Some(view.0.revision_date.to_rfc3339().as_str()) {
                return Err(conflict());
            }
            if view.0.r#type != kind
                || view.0.organization_id.map(|id| id.to_string()) != draft.organization_id
                || view
                    .0
                    .collection_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    != draft.collection_ids
            {
                return Err(permission_error());
            }
            view
        } else {
            if let Some(org) = draft.organization_id.as_ref() {
                if !self.organizations.iter().any(|o| &o.id == org)
                    || draft.collection_ids.is_empty()
                    || draft.collection_ids.iter().any(|id| {
                        !self.collections.iter().any(|c| {
                            &c.id == id
                                && &c.organization_id == org
                                && !c.read_only
                                && !c.hide_passwords
                        })
                    })
                {
                    return Err(permission_error());
                }
            } else if !draft.collection_ids.is_empty() {
                return Err(permission_error());
            }
            let now = Utc::now();
            TransientView(CipherView {
                id: None,
                organization_id: draft
                    .organization_id
                    .as_ref()
                    .map(|id| id.parse())
                    .transpose()
                    .map_err(|_| invalid_response())?,
                folder_id: None,
                collection_ids: draft
                    .collection_ids
                    .iter()
                    .map(|id| id.parse())
                    .collect::<Result<_, _>>()
                    .map_err(|_| invalid_response())?,
                key: None,
                name: String::new(),
                notes: None,
                r#type: kind,
                login: None,
                card: None,
                identity: None,
                secure_note: None,
                ssh_key: None,
                bank_account: None,
                drivers_license: None,
                passport: None,
                favorite: false,
                reprompt: CipherRepromptType::None,
                organization_use_totp: true,
                edit: true,
                permissions: None,
                view_password: true,
                local_data: None,
                attachments: None,
                attachment_decryption_failures: None,
                fields: None,
                password_history: None,
                creation_date: now,
                revision_date: now,
                deleted_date: None,
                archived_date: None,
                partial: false,
            })
        };
        let v = &mut view.0;
        if draft.id.is_none() {
            v.upgrade_to_cipher_key_encryption(&mut self.client.internal.get_key_store().context())
                .map_err(|_| decryption_error())?;
        }
        v.name = draft.name.clone();
        v.favorite = draft.favorite;
        v.notes = Some(draft.notes.clone());
        if kind == CipherType::Login {
            let login = v.login.get_or_insert(LoginView {
                username: None,
                password: None,
                password_revision_date: None,
                uris: None,
                totp: None,
                autofill_on_page_load: None,
                fido2_credentials: None,
            });
            if login.password.as_deref().unwrap_or_default() != draft.password {
                if let Some(old) = login
                    .password
                    .as_ref()
                    .filter(|password| !password.is_empty())
                {
                    let history = v.password_history.get_or_insert_default();
                    history.insert(
                        0,
                        PasswordHistoryView {
                            password: old.clone(),
                            last_used_date: Utc::now(),
                        },
                    );
                    history.truncate(5);
                }
                login.password_revision_date = Some(Utc::now());
            }
            login.username = Some(draft.username.clone());
            login.password = Some(draft.password.clone());
            login.totp = Some(draft.totp.clone());
            let uris = login.uris.get_or_insert_default();
            if let Some(first) = uris.first_mut() {
                first.uri = Some(draft.uri.clone());
            } else if !draft.uri.is_empty() {
                uris.push(LoginUriView {
                    uri: Some(draft.uri.clone()),
                    r#match: None,
                    uri_checksum: None,
                });
            }
        } else if kind == CipherType::SecureNote {
            v.secure_note = Some(SecureNoteView {
                r#type: SecureNoteType::Generic,
            });
        } else if kind == CipherType::Card {
            v.card = Some(draft.card.clone().ok_or_else(invalid_response)?);
        } else if kind == CipherType::Identity {
            v.identity = Some(draft.identity.clone().ok_or_else(invalid_response)?);
        }
        // Preserve keys, custom fields, extra URIs, passkeys, attachments and history
        // by editing the complete original SDK view, never a metadata projection.
        let mut request: CipherRequestModel = self
            .client
            .vault()
            .ciphers()
            .encrypt(v.clone())
            .await
            .map_err(|_| decryption_error())?
            .into();
        request.last_known_revision_date = draft.id.as_ref().map(|_| v.revision_date.to_rfc3339());
        let response = if let Some(id) = draft.id.as_ref() {
            self.http
                .put(
                    self.server
                        .join(&format!("api/ciphers/{id}"))
                        .map_err(|_| invalid_response())?,
                )
                .json(&request)
        } else if draft.organization_id.is_some() {
            self.http.post(self.server.join("api/ciphers/create").map_err(|_| invalid_response())?).json(&serde_json::json!({ "cipher": request, "collectionIds": draft.collection_ids }))
        } else {
            self.http
                .post(
                    self.server
                        .join("api/ciphers")
                        .map_err(|_| invalid_response())?,
                )
                .json(&request)
        };
        let result = self
            .dispatch_write(response, true)
            .await?
            .ok_or_else(invalid_response)?;
        let id = result
            .get("id")
            .or_else(|| result.get("Id"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                VaultError::new(
                    "write_uncertain",
                    "The item may be saved. Synchronize and inspect the vault before trying again.",
                )
            })?
            .to_owned();
        check_id(&id)?;
        let refresh_pending = self.refresh_after_write().await?;
        Ok((id, refresh_pending))
    }

    pub async fn change_item(
        &mut self,
        request: ChangeItemRequest,
    ) -> Result<(String, bool), VaultError> {
        self.require_write_ready()?;
        if !matches!(request.action, ChangeAction::Favorite) {
            // The official soft-delete/restore endpoints have no revision CAS.
            // Refresh permissions and detect existing conflicts before dispatch;
            // an external write can still race this check on the server.
            self.sync().await?;
        }
        let mut view = self.editable_view(&request.id, true).await?;
        let v = &mut view.0;
        if request.revision != v.revision_date.to_rfc3339() {
            return Err(conflict());
        }
        let endpoint = self
            .server
            .join(&format!("api/ciphers/{}", request.id))
            .map_err(|_| invalid_response())?;
        match request.action {
            ChangeAction::Favorite if v.deleted_date.is_none() => {
                v.favorite = request.favorite.ok_or_else(invalid_response)?;
                let mut body: CipherRequestModel = self
                    .client
                    .vault()
                    .ciphers()
                    .encrypt(v.clone())
                    .await
                    .map_err(|_| decryption_error())?
                    .into();
                body.last_known_revision_date = Some(v.revision_date.to_rfc3339());
                self.dispatch_write(self.http.put(endpoint).json(&body), true)
                    .await?;
            }
            ChangeAction::Trash
                if v.deleted_date.is_none()
                    && v.permissions.as_ref().map_or(v.edit, |p| p.delete) =>
            {
                self.dispatch_write(
                    self.http.put(
                        self.server
                            .join(&format!("api/ciphers/{}/delete", request.id))
                            .map_err(|_| invalid_response())?,
                    ),
                    false,
                )
                .await?;
            }
            ChangeAction::Restore
                if v.deleted_date.is_some()
                    && v.permissions.as_ref().map_or(v.edit, |p| p.restore) =>
            {
                self.dispatch_write(
                    self.http.put(
                        self.server
                            .join(&format!("api/ciphers/{}/restore", request.id))
                            .map_err(|_| invalid_response())?,
                    ),
                    true,
                )
                .await?;
            }
            _ => return Err(permission_error()),
        }
        let refresh_pending = self.refresh_after_write().await?;
        Ok((request.id, refresh_pending))
    }

    async fn refresh_after_write(&mut self) -> Result<bool, VaultError> {
        match self.sync().await {
            Ok(()) => Ok(false),
            Err(failure) if failure.code == "session_expired" => Err(VaultError::new(
                "session_expired",
                "Your session expired after the write. The change may be saved; sign in again and inspect the vault.",
            )),
            Err(_) => Ok(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn draft(kind: &str) -> ItemDraft {
        serde_json::from_value(serde_json::json!({
            "kind": kind, "name": format!("Veyra synthetic {kind}"), "favorite": false,
            "notes": "Synthetic editor notes", "username": "editor@veyra.test", "password": "Only-synthetic-editor-data",
            "uri": "https://example.com/edit", "totp": "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
            "card": { "cardholderName": "Fixture", "number": "4111111111111111", "code": "123", "expMonth": "12", "expYear": "2030", "brand": "Visa" },
            "identity": { "firstName": "Synthetic", "lastName": "Fixture", "email": "fixture@veyra.test" }
        })).unwrap()
    }

    fn login_view() -> CipherView {
        serde_json::from_value(serde_json::json!({
            "id": "12345678-1234-1234-1234-123456789abc", "collectionIds": [],
            "name": "Synthetic", "notes": "Transient note", "type": 1,
            "login": { "username": "fixture@veyra.test", "password": "Synthetic-password", "totp": "Synthetic-totp" },
            "favorite": false, "reprompt": 0, "organizationUseTotp": true,
            "edit": false, "viewPassword": true,
            "creationDate": "2026-10-29T00:00:00Z", "revisionDate": "2026-10-29T00:00:00Z"
        })).unwrap()
    }

    #[test]
    fn contents_permission_is_independent_from_edit_permission_and_omits_login_secrets() {
        let mut view = TransientView(login_view());
        assert!(can_read_contents(&view.0));
        assert!(!can_edit(&view.0));
        let contents = Vaultwarden::draft_from_view("synthetic", &view.0, 42, false).unwrap();
        assert!(contents.draft.notes == "Transient note");
        assert!(contents.draft.password.is_empty() && contents.draft.totp.is_empty());
        assert_eq!(contents.generation, 42);
        view.0.edit = true;
        let edit = Vaultwarden::draft_from_view("synthetic", &view.0, 42, true).unwrap();
        assert!(edit.draft.password == "Synthetic-password" && edit.draft.totp == "Synthetic-totp");
        for restricted in ["hidden", "partial", "reprompt"] {
            let mut view = TransientView(login_view());
            view.0.edit = true;
            match restricted {
                "hidden" => view.0.view_password = false,
                "partial" => view.0.partial = true,
                _ => view.0.reprompt = CipherRepromptType::Password,
            }
            assert!(!can_read_contents(&view.0) && !can_edit(&view.0));
        }
    }

    #[test]
    fn trash_and_restore_metadata_allow_only_the_applicable_action() {
        let mut view = TransientView(login_view());
        view.0.edit = true;
        let active = project_metadata(&view.0, "login").unwrap();
        assert!(active.can_delete && !active.can_restore);
        view.0.deleted_date = Some(Utc::now());
        let trashed = project_metadata(&view.0, "login").unwrap();
        assert!(!trashed.can_delete && trashed.can_restore && trashed.read_only);
    }

    pub(super) async fn connected_fixture(organization: bool) -> Vaultwarden {
        let path = std::env::var("VEYRA_TEST_FIXTURE_FILE").expect("set synthetic fixture path");
        let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let account = if organization {
            &fixture["organizationAccount"]
        } else {
            &fixture["writeAccount"]
        };
        Vaultwarden::connect_inner(
            fixture["serverUrl"].as_str().unwrap().into(),
            account["email"].as_str().unwrap().into(),
            Zeroizing::new(account["password"].as_str().unwrap().into()),
            None,
            true,
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    #[ignore = "requires dedicated disposable Vaultwarden fixtures"]
    async fn actual_vaultwarden_create_edit_favorite_trash_restore_all_supported_types() {
        let mut vault = connected_fixture(false).await;
        for kind in ["login", "note", "card", "identity"] {
            let (id, pending) = vault.save_item(draft(kind)).await.unwrap();
            assert!(!pending);
            let mut editor = vault.editable_item(&id, 0).await.unwrap().draft;
            assert!(editor.notes == "Synthetic editor notes");
            if kind == "login" {
                assert!(editor.password == "Only-synthetic-editor-data");
            }
            // Extra fields and URIs remain intact across an editor roundtrip.
            if kind == "login" {
                let mut view = vault.editable_view(&id, false).await.unwrap();
                view.0
                    .login
                    .as_mut()
                    .unwrap()
                    .uris
                    .as_mut()
                    .unwrap()
                    .push(LoginUriView {
                        uri: Some("https://example.net/extra".into()),
                        r#match: None,
                        uri_checksum: None,
                    });
                view.0.fields = Some(vec![bitwarden_vault::FieldView {
                    name: Some("Preserved custom field".into()),
                    value: Some("Synthetic custom secret".into()),
                    r#type: bitwarden_vault::FieldType::Hidden,
                    linked_id: None,
                }]);
                let body: CipherRequestModel = vault
                    .client
                    .vault()
                    .ciphers()
                    .encrypt(view.0.clone())
                    .await
                    .unwrap()
                    .into();
                vault
                    .dispatch_write(
                        vault
                            .http
                            .put(vault.server.join(&format!("api/ciphers/{id}")).unwrap())
                            .json(&body),
                        true,
                    )
                    .await
                    .unwrap();
                vault.sync().await.unwrap();
                editor = vault.editable_item(&id, 0).await.unwrap().draft;
                editor.password = "Synthetic-new-password".into();
            }
            let stale = vault.editable_item(&id, 0).await.unwrap().draft;
            editor.name = format!("Edited {kind}");
            vault.save_item(editor).await.unwrap();
            assert!(matches!(vault.save_item(stale).await, Err(e) if e.code == "item_changed"));
            let view = vault.editable_view(&id, false).await.unwrap();
            assert!(view.0.notes.as_deref() == Some("Synthetic editor notes"));
            if kind == "login" {
                assert!(view.0.login.as_ref().unwrap().uris.as_ref().unwrap().len() == 2);
                assert!(
                    view.0.fields.as_ref().unwrap()[0].value.as_deref()
                        == Some("Synthetic custom secret")
                );
                assert!(view
                    .0
                    .password_history
                    .as_ref()
                    .is_some_and(|h| h.iter().any(|p| p.password == "Only-synthetic-editor-data")));
            }
            drop(view);
            let revision = vault
                .items
                .iter()
                .find(|item| item.id == id)
                .unwrap()
                .revision
                .clone();
            vault
                .change_item(ChangeItemRequest {
                    id: id.clone(),
                    revision,
                    action: ChangeAction::Favorite,
                    favorite: Some(true),
                })
                .await
                .unwrap();
            assert!(
                vault
                    .items
                    .iter()
                    .find(|item| item.id == id)
                    .unwrap()
                    .favorite
            );
            let revision = vault
                .items
                .iter()
                .find(|item| item.id == id)
                .unwrap()
                .revision
                .clone();
            vault
                .change_item(ChangeItemRequest {
                    id: id.clone(),
                    revision,
                    action: ChangeAction::Trash,
                    favorite: None,
                })
                .await
                .unwrap();
            assert!(
                vault
                    .items
                    .iter()
                    .find(|item| item.id == id)
                    .unwrap()
                    .deleted
            );
            assert!(vault.field(&id, Field::Password).await.is_err());
            assert!(vault.item_contents(&id, 0).await.is_err());
            assert!(vault.url(&id).is_err());
            let revision = vault
                .items
                .iter()
                .find(|item| item.id == id)
                .unwrap()
                .revision
                .clone();
            vault
                .change_item(ChangeItemRequest {
                    id: id.clone(),
                    revision,
                    action: ChangeAction::Restore,
                    favorite: None,
                })
                .await
                .unwrap();
            assert!(
                !vault
                    .items
                    .iter()
                    .find(|item| item.id == id)
                    .unwrap()
                    .deleted
            );
            // Leave only original fixtures active, so repeat runs stay deterministic.
            let revision = vault
                .items
                .iter()
                .find(|item| item.id == id)
                .unwrap()
                .revision
                .clone();
            vault
                .change_item(ChangeItemRequest {
                    id,
                    revision,
                    action: ChangeAction::Trash,
                    favorite: None,
                })
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "requires dedicated disposable Vaultwarden organization fixtures"]
    async fn actual_vaultwarden_organization_writes_and_restrictions() {
        let mut vault = connected_fixture(true).await;
        let org = vault.organizations[0].id.clone();
        let collection = vault.collections[0].id.clone();
        let mut new = draft("login");
        new.organization_id = Some(org.clone());
        new.collection_ids = vec![collection.clone()];
        let (id, pending) = vault.save_item(new).await.unwrap();
        assert!(!pending);
        let item = vault.items.iter().find(|i| i.id == id).unwrap();
        assert!(
            item.organization_id.as_deref() == Some(org.as_str())
                && item.collection_ids.contains(&collection)
        );
        let mut edit = vault.editable_item(&id, 0).await.unwrap().draft;
        edit.password = "Synthetic-shared-edit".into();
        vault.save_item(edit).await.unwrap();
        assert!(*vault.field(&id, Field::Password).await.unwrap() == "Synthetic-shared-edit");
        let index = vault
            .encrypted
            .iter()
            .position(|cipher| cipher.id.is_some_and(|i| i.to_string() == id))
            .unwrap();
        vault.encrypted[index].edit = false;
        let contents = vault.item_contents(&id, 41).await.unwrap();
        assert!(contents.draft.notes == "Synthetic editor notes");
        assert!(contents.draft.password.is_empty() && contents.draft.totp.is_empty());
        assert!(
            matches!(vault.editable_item(&id, 0).await, Err(e) if e.code == "write_not_allowed")
        );
        vault.encrypted[index].edit = true;
        vault.encrypted[index].view_password = false;
        assert!(
            matches!(vault.item_contents(&id, 0).await, Err(e) if e.code == "contents_not_allowed")
        );
        assert!(
            matches!(vault.editable_item(&id, 0).await, Err(e) if e.code == "write_not_allowed")
        );
        vault.sync().await.unwrap();
        vault.collections[0].read_only = true;
        let mut blocked = draft("login");
        blocked.organization_id = Some(org);
        blocked.collection_ids = vec![collection];
        assert!(matches!(vault.save_item(blocked).await, Err(e) if e.code == "write_not_allowed"));
        vault.sync().await.unwrap();
        let revision = vault
            .items
            .iter()
            .find(|i| i.id == id)
            .unwrap()
            .revision
            .clone();
        vault
            .change_item(ChangeItemRequest {
                id,
                revision,
                action: ChangeAction::Trash,
                favorite: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires dedicated disposable Vaultwarden fixtures"]
    async fn actual_vaultwarden_rejects_server_conflicts_and_never_retries_uncertain_writes() {
        let mut vault = connected_fixture(false).await;
        let (id, _) = vault.save_item(draft("login")).await.unwrap();
        let stale = vault.editable_item(&id, 0).await.unwrap().draft;
        let mut other = connected_fixture(false).await;
        let mut updated = other.editable_item(&id, 0).await.unwrap().draft;
        updated.name = "Updated elsewhere".into();
        tokio::time::sleep(Duration::from_secs(2)).await;
        other.save_item(updated).await.unwrap();
        assert!(matches!(vault.save_item(stale).await, Err(e) if e.code == "write_rejected"));
        let stale_revision = vault
            .items
            .iter()
            .find(|i| i.id == id)
            .unwrap()
            .revision
            .clone();
        assert!(matches!(vault.change_item(ChangeItemRequest {
            id: id.clone(), revision: stale_revision, action: ChangeAction::Trash, favorite: None,
        }).await, Err(e) if e.code == "item_changed"));
        assert!(!vault.items.iter().find(|i| i.id == id).unwrap().deleted);
        vault.sync().await.unwrap();
        assert!(vault.items.iter().find(|i| i.id == id).unwrap().name == "Updated elsewhere");
        let original = vault.server.clone();
        vault.server = super::super::tests::single_response_server(
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        assert!(
            matches!(vault.save_item(draft("note")).await, Err(e) if e.code == "write_uncertain")
        );
        assert!(
            matches!(vault.save_item(draft("note")).await, Err(e) if e.code == "sync_required")
        );
        vault.server = original;
        vault.sync().await.unwrap();
        let revision = vault
            .items
            .iter()
            .find(|i| i.id == id)
            .unwrap()
            .revision
            .clone();
        vault
            .change_item(ChangeItemRequest {
                id,
                revision,
                action: ChangeAction::Trash,
                favorite: None,
            })
            .await
            .unwrap();
    }
}
