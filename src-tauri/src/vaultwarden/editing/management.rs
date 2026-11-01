//! Sequential, revision-checked management. A batch is never an atomic server transaction.
use super::*;
use std::collections::HashSet;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedItemReference {
    pub id: String,
    pub revision: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagementOperation {
    AssignFolder {
        #[serde(rename = "folderId")]
        folder_id: Option<String>,
    },
    AssignCollections {
        #[serde(rename = "collectionIds")]
        collection_ids: Vec<String>,
    },
    Copy {
        #[serde(rename = "organizationId")]
        organization_id: Option<String>,
        #[serde(rename = "collectionIds")]
        collection_ids: Vec<String>,
        #[serde(rename = "folderId")]
        folder_id: Option<String>,
    },
    Move {
        #[serde(rename = "organizationId")]
        organization_id: Option<String>,
        #[serde(rename = "collectionIds")]
        collection_ids: Vec<String>,
        #[serde(rename = "folderId")]
        folder_id: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManageItemsRequest {
    pub items: Vec<ManagedItemReference>,
    pub operation: ManagementOperation,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagementStatus {
    Succeeded,
    Failed,
    Uncertain,
    NotAttempted,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedItemResult {
    pub source_id: String,
    pub item_id: Option<String>,
    pub status: ManagementStatus,
    pub error: Option<VaultError>,
}

// Preserve future encrypted fields by using the partial endpoints for folder and
// collection assignment. Copy/re-key operations must reject any unknown payload,
// including unknown nested login/custom-field data, rather than silently drop it.
pub(in crate::vaultwarden) fn management_unsupported_ids(
    raw: &Value,
) -> Result<HashSet<String>, VaultError> {
    let mut unsupported = HashSet::new();
    for value in raw
        .get("ciphers")
        .or_else(|| raw.get("Ciphers"))
        .and_then(Value::as_array)
        .ok_or_else(invalid_response)?
    {
        let known: bitwarden_api_api::models::CipherDetailsResponseModel =
            serde_json::from_value(value.clone()).map_err(|_| invalid_response())?;
        let id = known.id.ok_or_else(invalid_response)?.to_string();
        let known = serde_json::to_value(known).map_err(|_| invalid_response())?;
        let login = value.get("login").or_else(|| value.get("Login"));
        let unsupported_legacy_uri = login.is_some_and(|login| {
            login
                .get("uri")
                .or_else(|| login.get("Uri"))
                .is_some_and(|v| !v.is_null())
                && login
                    .get("uris")
                    .or_else(|| login.get("Uris"))
                    .is_none_or(Value::is_null)
        });
        if unsupported_legacy_uri
            || unknown_fields(value, &known)
            || value
                .get("data")
                .or_else(|| value.get("Data"))
                .is_some_and(|v| !v.is_null())
        {
            unsupported.insert(id);
        }
    }
    Ok(unsupported)
}

fn unknown_fields(raw: &Value, known: &Value) -> bool {
    match (raw, known) {
        (Value::Object(raw), Value::Object(known)) => raw.iter().any(|(key, value)| {
            if value.is_null() {
                return false;
            }
            let mut canonical = key.clone();
            if let Some(first) = canonical.get_mut(0..1) {
                first.make_ascii_lowercase();
            }
            known
                .get(&canonical)
                .is_none_or(|known| unknown_fields(value, known))
        }),
        (Value::Array(raw), Value::Array(known)) => {
            raw.len() != known.len()
                || raw
                    .iter()
                    .zip(known)
                    .any(|(raw, known)| unknown_fields(raw, known))
        }
        _ => false,
    }
}

impl ManageItemsRequest {
    fn validate(&self) -> Result<(), VaultError> {
        if self.items.is_empty() || self.items.len() > 100 {
            return Err(VaultError::new(
                "invalid_selection",
                "Select between 1 and 100 entries.",
            ));
        }
        let mut ids = HashSet::new();
        for item in &self.items {
            check_id(&item.id)?;
            if !ids.insert(&item.id)
                || item.revision.len() > 64
                || chrono::DateTime::parse_from_rfc3339(&item.revision).is_err()
            {
                return Err(invalid_response());
            }
        }
        let (folder, organization, collections) = match &self.operation {
            ManagementOperation::AssignFolder { folder_id } => (Some(folder_id), None, None),
            ManagementOperation::AssignCollections { collection_ids } => {
                (None, None, Some(collection_ids))
            }
            ManagementOperation::Copy {
                organization_id,
                collection_ids,
                folder_id,
            }
            | ManagementOperation::Move {
                organization_id,
                collection_ids,
                folder_id,
            } => (Some(folder_id), Some(organization_id), Some(collection_ids)),
        };
        for id in folder.into_iter().chain(organization).flatten() {
            check_id(id)?;
        }
        if let Some(collections) = collections {
            let mut ids = HashSet::new();
            if collections.len() > 100 {
                return Err(invalid_response());
            }
            for id in collections {
                check_id(id)?;
                if !ids.insert(id) {
                    return Err(invalid_response());
                }
            }
        }
        Ok(())
    }
}

fn same_payload(a: &CipherView, b: &CipherView) -> bool {
    a.name == b.name
        && a.favorite == b.favorite
        && a.notes == b.notes
        && a.r#type == b.r#type
        && a.login == b.login
        && a.card == b.card
        && a.identity == b.identity
        && a.secure_note == b.secure_note
        && a.fields == b.fields
        && a.password_history == b.password_history
        && a.reprompt == b.reprompt
        && a.archived_date == b.archived_date
}

fn retained_copy(uncertain: bool) -> VaultError {
    if uncertain {
        VaultError::new("copy_created_source_uncertain", "The destination copy was verified. The source removal result is unknown; synchronize and inspect both entries before retrying.")
    } else {
        VaultError::new("copy_created_source_retained", "The destination copy was verified, but the source was retained. Inspect both entries before trying to move again.")
    }
}

impl Vaultwarden {
    fn validate_folder(&self, folder: &Option<String>) -> Result<(), VaultError> {
        if folder
            .as_ref()
            .is_some_and(|id| !self.folders.iter().any(|f| &f.id == id))
        {
            return Err(VaultError::new(
                "folder_unavailable",
                "The selected folder is unavailable. Synchronize and select it again.",
            ));
        }
        Ok(())
    }

    fn validate_destination(
        &self,
        organization: &Option<String>,
        collections: &[String],
    ) -> Result<(), VaultError> {
        if let Some(org) = organization {
            if !self.organizations.iter().any(|o| &o.id == org)
                || collections.is_empty()
                || collections.iter().any(|id| {
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
        } else if !collections.is_empty() {
            return Err(permission_error());
        }
        Ok(())
    }

    async fn management_dispatch(
        &mut self,
        request: reqwest::RequestBuilder,
    ) -> Result<Value, VaultError> {
        // Some server management endpoints can partially commit before a 4xx.
        // Once dispatched, every failure gates later writes until an explicit sync.
        match self.dispatch_write(request, true).await {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(invalid_response()),
            Err(error) => {
                self.write_needs_sync = true;
                Err(error)
            }
        }
    }

    async fn assign_folder(
        &mut self,
        id: &str,
        folder: &Option<String>,
        favorite: bool,
    ) -> Result<bool, VaultError> {
        self.validate_folder(folder)?;
        let url = self
            .server
            .join(&format!("api/ciphers/{id}/partial"))
            .map_err(|_| invalid_response())?;
        self.management_dispatch(
            self.http
                .put(url)
                .json(&serde_json::json!({"folderId": folder, "favorite": favorite})),
        )
        .await?;
        self.refresh_after_write().await
    }

    async fn assign_collections(
        &mut self,
        id: &str,
        org: &Option<String>,
        collections: &[String],
    ) -> Result<bool, VaultError> {
        if org.is_none() {
            return Err(permission_error());
        }
        self.validate_destination(org, collections)?;
        // Removing a collection also requires write permission to that collection.
        let source = self
            .items
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(invalid_response)?;
        if source.collection_ids.iter().any(|id| {
            !collections.contains(id)
                && !self
                    .collections
                    .iter()
                    .any(|c| &c.id == id && !c.read_only && !c.hide_passwords)
        }) {
            return Err(permission_error());
        }
        let url = self
            .server
            .join(&format!("api/ciphers/{id}/collections"))
            .map_err(|_| invalid_response())?;
        self.management_dispatch(
            self.http
                .put(url)
                .json(&serde_json::json!({"collectionIds": collections})),
        )
        .await?;
        self.refresh_after_write().await
    }

    fn require_copyable(&self, id: &str, view: &CipherView) -> Result<(), VaultError> {
        if self.management_unsupported.contains(id)
            || view.attachments.as_ref().is_some_and(|a| !a.is_empty())
            || view
                .attachment_decryption_failures
                .as_ref()
                .is_some_and(|a| !a.is_empty())
            || view
                .login
                .as_ref()
                .and_then(|login| login.fido2_credentials.as_ref())
                .is_some_and(|credentials| !credentials.is_empty())
        {
            return Err(VaultError::new("migration_not_supported", "This entry contains attachments, passkeys or unsupported fields. Use an official client to copy it or change its owner without losing data."));
        }
        Ok(())
    }

    async fn manage_one(
        &mut self,
        item: &ManagedItemReference,
        operation: &ManagementOperation,
        destination_id: &mut Option<String>,
    ) -> Result<bool, VaultError> {
        let mut source = self.editable_view(&item.id, false).await?;
        if source.0.revision_date.to_rfc3339() != item.revision {
            return Err(conflict());
        }
        let source_org = source.0.organization_id.map(|id| id.to_string());
        match operation {
            ManagementOperation::AssignFolder { folder_id } => {
                self.assign_folder(&item.id, folder_id, source.0.favorite)
                    .await
            }
            ManagementOperation::AssignCollections { collection_ids } => {
                self.assign_collections(&item.id, &source_org, collection_ids)
                    .await
            }
            ManagementOperation::Copy {
                organization_id,
                collection_ids,
                folder_id,
            }
            | ManagementOperation::Move {
                organization_id,
                collection_ids,
                folder_id,
            } => {
                self.validate_folder(folder_id)?;
                self.validate_destination(organization_id, collection_ids)?;
                let moving = matches!(operation, ManagementOperation::Move { .. });
                if moving && source_org == *organization_id {
                    *destination_id = Some(item.id.clone());
                    if organization_id.is_some()
                        && self
                            .assign_collections(&item.id, organization_id, collection_ids)
                            .await?
                    {
                        return Err(VaultError::new("move_incomplete", "The collection change was accepted but could not be refreshed. Folder assignment was not attempted; synchronize and inspect the item."));
                    }
                    // Collection assignment refreshed the vault. Preserve any
                    // favorite change observed in that new snapshot and recheck
                    // current permissions before the second management write.
                    let current = self.editable_view(&item.id, false).await.map_err(|error| {
                        if error.code == "session_expired" { error }
                        else { VaultError::new("move_incomplete", "Collection assignment completed, but the entry can no longer be changed. Folder assignment was not attempted.") }
                    })?;
                    return self
                        .assign_folder(&item.id, folder_id, current.0.favorite)
                        .await;
                }
                self.require_copyable(&item.id, &source.0)?;
                if moving
                    && !source
                        .0
                        .permissions
                        .as_ref()
                        .map_or(source.0.edit, |p| p.delete)
                {
                    return Err(permission_error());
                }
                // The SDK encrypts the full view under the destination owner key.
                // Per-cipher keys for duplicates are freshly generated by the SDK.
                let now = Utc::now();
                let mut target = TransientView(source.0.clone());
                target.0.folder_id = folder_id
                    .as_ref()
                    .map(|id| id.parse())
                    .transpose()
                    .map_err(|_| invalid_response())?;
                target.0.collection_ids = collection_ids
                    .iter()
                    .map(|id| id.parse())
                    .collect::<Result<_, _>>()
                    .map_err(|_| invalid_response())?;
                let direct_share = moving && source_org.is_none() && organization_id.is_some();
                if direct_share {
                    target
                        .0
                        .move_to_organization(
                            organization_id
                                .as_ref()
                                .unwrap()
                                .parse()
                                .map_err(|_| invalid_response())?,
                        )
                        .map_err(|_| decryption_error())?;
                } else {
                    target.0.id = None;
                    target.0.organization_id = organization_id
                        .as_ref()
                        .map(|id| id.parse())
                        .transpose()
                        .map_err(|_| invalid_response())?;
                    target.0.key = None;
                    target.0.local_data = None;
                    target.0.permissions = None;
                    target.0.creation_date = now;
                    target.0.revision_date = now;
                    target
                        .0
                        .upgrade_to_cipher_key_encryption(
                            &mut self.client.internal.get_key_store().context(),
                        )
                        .map_err(|_| decryption_error())?;
                }
                let mut body: CipherRequestModel = self
                    .client
                    .vault()
                    .ciphers()
                    .encrypt(target.0.clone())
                    .await
                    .map_err(|_| decryption_error())?
                    .into();
                body.last_known_revision_date = direct_share.then(|| item.revision.clone());
                let url = self
                    .server
                    .join(&if direct_share {
                        format!("api/ciphers/{}/share", item.id)
                    } else {
                        "api/ciphers/create".into()
                    })
                    .map_err(|_| invalid_response())?;
                let request = if direct_share {
                    self.http.put(url)
                } else {
                    self.http.post(url)
                };
                let response = self
                    .management_dispatch(request.json(
                        &serde_json::json!({"cipher": body, "collectionIds": collection_ids}),
                    ))
                    .await?;
                let new_id = response
                    .get("id")
                    .or_else(|| response.get("Id"))
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_response)?
                    .to_owned();
                check_id(&new_id)?;
                if (direct_share && new_id != item.id) || (!direct_share && new_id == item.id) {
                    return Err(VaultError::new("write_uncertain", "The server returned an unexpected entry ID. Synchronize and inspect the vault before trying again."));
                }
                *destination_id = Some(new_id.clone());
                if self.refresh_after_write().await? {
                    if moving && !direct_share {
                        return Err(VaultError::new("copy_created_refresh_pending", "Destination creation was accepted but could not be verified. The source was retained; synchronize and inspect both entries before retrying."));
                    }
                    return Ok(true);
                }
                let copied = match self.decrypted_view(&new_id).await {
                    Ok(copied) => copied,
                    Err(error) => {
                        self.write_needs_sync = true;
                        if error.code == "session_expired" {
                            return Err(error);
                        }
                        return Err(VaultError::new("copy_verification_failed", "The server accepted the change, but the destination could not be verified. Synchronize and inspect the vault before another change."));
                    }
                };
                if !same_payload(&target.0, &copied.0)
                    || copied.0.organization_id.map(|id| id.to_string()) != *organization_id
                    || copied.0.folder_id.map(|id| id.to_string()) != *folder_id
                    || copied
                        .0
                        .collection_ids
                        .iter()
                        .map(ToString::to_string)
                        .collect::<HashSet<_>>()
                        != collection_ids.iter().cloned().collect::<HashSet<_>>()
                {
                    self.write_needs_sync = true;
                    if direct_share {
                        return Err(VaultError::new("move_verification_failed", "The server accepted the owner change, but the destination did not match. Synchronize and inspect the item before another change."));
                    }
                    return Err(VaultError::new("copy_verification_failed", "The destination could not be fully verified. The source was retained; inspect both entries before retrying."));
                }
                if moving && !direct_share {
                    // Organization-owned ciphers cannot be reassigned to a private
                    // vault/other organization. Verify the destination, then soft
                    // delete the unchanged source. Never hard-delete it.
                    source = self.editable_view(&item.id, false).await.map_err(|error| {
                        if error.code == "session_expired" {
                            error
                        } else {
                            retained_copy(false)
                        }
                    })?;
                    if source.0.revision_date.to_rfc3339() != item.revision {
                        return Err(retained_copy(false));
                    }
                    match self
                        .change_item(ChangeItemRequest {
                            id: item.id.clone(),
                            revision: item.revision.clone(),
                            action: ChangeAction::Trash,
                            favorite: None,
                        })
                        .await
                    {
                        Ok((_, pending)) => return Ok(pending),
                        Err(error) if error.code == "session_expired" => return Err(error),
                        Err(_) => return Err(retained_copy(self.write_needs_sync)),
                    }
                }
                Ok(false)
            }
        }
    }

    pub async fn manage_items(
        &mut self,
        request: ManageItemsRequest,
    ) -> Result<(Vec<ManagedItemResult>, bool), VaultError> {
        self.require_write_ready()?;
        request.validate()?;
        // Folder/collection/delete routes have no atomic revision compare-and-set.
        // Refresh immediately before this sequential batch and recheck each source.
        self.sync().await?;
        let mut results = Vec::with_capacity(request.items.len());
        let mut stop = false;
        let mut pending = false;
        for item in &request.items {
            if stop {
                results.push(ManagedItemResult { source_id: item.id.clone(), item_id: None, status: ManagementStatus::NotAttempted, error: Some(VaultError::new("batch_stopped", "This entry was not attempted. Inspect completed changes and synchronize before trying again.")) });
                continue;
            }
            let mut destination = None;
            let result = self
                .manage_one(item, &request.operation, &mut destination)
                .await;
            match result {
                Ok(refresh_pending) => {
                    pending |= refresh_pending;
                    stop = refresh_pending;
                    results.push(ManagedItemResult {
                        source_id: item.id.clone(),
                        item_id: destination.or_else(|| Some(item.id.clone())),
                        status: ManagementStatus::Succeeded,
                        error: None,
                    });
                }
                Err(error) if error.code == "session_expired" => return Err(error),
                Err(error) => {
                    let uncertain = self.write_needs_sync;
                    stop = uncertain;
                    pending |= uncertain;
                    results.push(ManagedItemResult {
                        source_id: item.id.clone(),
                        item_id: destination,
                        status: if uncertain {
                            ManagementStatus::Uncertain
                        } else {
                            ManagementStatus::Failed
                        },
                        error: Some(error),
                    });
                }
            }
        }
        Ok((results, pending))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitwarden_crypto::PrimitiveEncryptable;

    #[test]
    fn future_fields_are_rejected_even_inside_encrypted_payload_models() {
        let supported = serde_json::json!({ "name": "encrypted", "login": { "username": "encrypted" }, "notes": null });
        assert!(!unknown_fields(&supported, &supported));
        let future = serde_json::json!({ "name": "encrypted", "login": { "username": "encrypted", "futureSecret": "encrypted" } });
        assert!(unknown_fields(&future, &supported));
        assert!(unknown_fields(
            &serde_json::json!({"futurePayload": []}),
            &serde_json::json!({})
        ));
        assert!(!unknown_fields(
            &serde_json::json!({"Name": "encrypted", "Login": {"Username": "encrypted"}}),
            &supported
        ));
    }

    #[test]
    fn batch_selection_and_destination_inputs_are_bounded() {
        let item = serde_json::json!({ "id": "12345678-1234-1234-1234-123456789abc", "revision": "2026-10-29T00:00:00Z" });
        let make = |items: Value| {
            serde_json::from_value::<ManageItemsRequest>(serde_json::json!({"items": items, "operation": {"type": "assign_folder", "folderId": null}})).unwrap()
        };
        assert!(make(serde_json::json!([])).validate().is_err());
        assert!(make(serde_json::json!([item, item])).validate().is_err());
        assert!(make(serde_json::json!([item])).validate().is_ok());
        assert!(serde_json::from_value::<ManageItemsRequest>(serde_json::json!({"items": [item], "operation": {"type":"assign_folder", "folderId":null,"password":"unexpected"}})).is_err());
    }

    async fn fixture() -> Vaultwarden {
        super::super::tests::connected_fixture(true).await
    }

    fn reference(vault: &Vaultwarden, id: &str) -> Value {
        serde_json::json!({"id": id, "revision": vault.items.iter().find(|i| i.id == id).unwrap().revision})
    }

    async fn manage(
        vault: &mut Vaultwarden,
        items: Vec<Value>,
        operation: Value,
    ) -> (Vec<ManagedItemResult>, bool) {
        vault
            .manage_items(
                serde_json::from_value(serde_json::json!({"items":items,"operation":operation}))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    fn response_sequence(bodies: Vec<Vec<u8>>) -> (Url, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let handle = std::thread::spawn(move || {
            use std::io::{Read, Write};
            for body in bodies {
                let deadline = std::time::Instant::now() + Duration::from_secs(15);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && std::time::Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
                        Err(error) => panic!("Synthetic response sequence did not receive the expected request: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 4096];
                let header_end = loop {
                    let length = stream.read(&mut chunk).unwrap();
                    assert!(length > 0 && bytes.len() + length <= 1024 * 1024);
                    bytes.extend_from_slice(&chunk[..length]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        break end + 4;
                    }
                };
                let header = std::str::from_utf8(&bytes[..header_end]).unwrap();
                let content_length = header
                    .lines()
                    .find_map(|line| {
                        line.split_once(':')
                            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            .map(|(_, length)| length.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                assert!(content_length <= 1024 * 1024);
                while bytes.len() < header_end + content_length {
                    let length = stream.read(&mut chunk).unwrap();
                    assert!(length > 0);
                    bytes.extend_from_slice(&chunk[..length]);
                }
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        (format!("http://{address}/").parse().unwrap(), handle)
    }

    #[tokio::test]
    #[ignore = "requires dedicated disposable Vaultwarden fixtures"]
    async fn actual_vaultwarden_copy_verification_failures_gate_remaining_writes() {
        let mut vault = fixture().await;
        for missing in [true, false] {
            let ids: Vec<_> = vault
                .items
                .iter()
                .filter(|i| {
                    !i.deleted
                        && i.organization_id.is_none()
                        && !vault.management_unsupported.contains(&i.id)
                })
                .take(2)
                .map(|i| i.id.clone())
                .collect();
            assert_eq!(ids.len(), 2);
            let before: Value = vault
                .http
                .get(vault.server.join("api/sync").unwrap())
                .bearer_auth(&*vault.access_token)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            let new_id = uuid::Uuid::new_v4().to_string();
            let mut after = before.clone();
            if !missing {
                let mut view = vault.decrypted_view(&ids[0]).await.unwrap();
                view.0.name.push_str(" (synthetic mismatch)");
                let encrypted: CipherRequestModel = vault
                    .client
                    .vault()
                    .ciphers()
                    .encrypt(view.0.clone())
                    .await
                    .unwrap()
                    .into();
                let encrypted = serde_json::to_value(encrypted).unwrap();
                let mut copied = before["ciphers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["id"].as_str() == Some(ids[0].as_str()))
                    .unwrap()
                    .clone();
                copied["id"] = Value::String(new_id.clone());
                copied["name"] = encrypted["name"].clone();
                copied["folderId"] = Value::Null;
                copied["collectionIds"] = serde_json::json!([]);
                after["ciphers"].as_array_mut().unwrap().push(copied);
            }
            let references = ids.iter().map(|id| reference(&vault, id)).collect();
            let (mock_url, mock) = response_sequence(vec![
                serde_json::to_vec(&before).unwrap(),
                serde_json::to_vec(&serde_json::json!({"id":new_id})).unwrap(),
                serde_json::to_vec(&after).unwrap(),
            ]);
            let original_server = std::mem::replace(&mut vault.server, mock_url);
            let (results, pending) = manage(&mut vault, references, serde_json::json!({"type":"copy","organizationId":null,"collectionIds":[],"folderId":null})).await;
            mock.join().unwrap();
            assert!(pending && vault.needs_sync());
            assert!(matches!(results[0].status, ManagementStatus::Uncertain));
            assert_eq!(results[0].item_id.as_deref(), Some(new_id.as_str()));
            assert_eq!(
                results[0].error.as_ref().unwrap().code,
                "copy_verification_failed"
            );
            assert!(matches!(results[1].status, ManagementStatus::NotAttempted));
            vault.server = original_server;
            vault.sync().await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "requires dedicated disposable Vaultwarden fixtures"]
    async fn actual_vaultwarden_batch_folder_copy_owner_moves_and_partial_results() {
        let mut vault = fixture().await;
        let org = vault.organizations[0].id.clone();
        let collection = vault
            .collections
            .iter()
            .find(|c| c.organization_id == org)
            .unwrap()
            .id
            .clone();
        let collection_name: EncString = "Management destination collection"
            .to_owned()
            .encrypt(
                &mut vault.client.internal.get_key_store().context(),
                SymmetricKeySlotId::Organization(org.parse().unwrap()),
            )
            .unwrap();
        let response = vault.management_dispatch(vault.http.post(vault.server.join(&format!("api/organizations/{org}/collections")).unwrap()).json(&serde_json::json!({"name": collection_name.to_string(), "groups": [], "users": [], "externalId": null}))).await.unwrap();
        let second_collection = response["id"].as_str().unwrap().to_string();
        assert!(!vault.refresh_after_write().await.unwrap());
        let name: EncString = "Nested/Management fixture"
            .to_owned()
            .encrypt(
                &mut vault.client.internal.get_key_store().context(),
                SymmetricKeySlotId::User,
            )
            .unwrap();
        let result = vault
            .management_dispatch(
                vault
                    .http
                    .post(vault.server.join("api/folders").unwrap())
                    .json(&serde_json::json!({"name": name.to_string()})),
            )
            .await
            .unwrap();
        let folder = result["id"].as_str().unwrap().to_string();
        assert!(!vault.refresh_after_write().await.unwrap());
        let mut draft = super::super::tests::draft("login");
        draft.favorite = true;
        let (id, _) = vault.save_item(draft).await.unwrap();
        let mut full = vault.editable_view(&id, false).await.unwrap();
        full.0.fields = Some(vec![bitwarden_vault::FieldView {
            name: Some("Preserved field".into()),
            value: Some("Synthetic field value".into()),
            r#type: bitwarden_vault::FieldType::Text,
            linked_id: None,
        }]);
        full.0
            .login
            .as_mut()
            .unwrap()
            .uris
            .as_mut()
            .unwrap()
            .push(bitwarden_vault::LoginUriView {
                uri: Some("https://example.net/extra".into()),
                r#match: None,
                uri_checksum: None,
            });
        let body: CipherRequestModel = vault
            .client
            .vault()
            .ciphers()
            .encrypt(full.0.clone())
            .await
            .unwrap()
            .into();
        vault
            .management_dispatch(
                vault
                    .http
                    .put(vault.server.join(&format!("api/ciphers/{id}")).unwrap())
                    .json(&body),
            )
            .await
            .unwrap();
        assert!(!vault.refresh_after_write().await.unwrap());
        drop(full);
        let first = reference(&vault, &id);
        let (results, pending) = manage(
            &mut vault,
            vec![first],
            serde_json::json!({"type":"assign_folder","folderId":folder}),
        )
        .await;
        assert!(!pending && matches!(results[0].status, ManagementStatus::Succeeded));
        assert_eq!(
            vault
                .items
                .iter()
                .find(|i| i.id == id)
                .unwrap()
                .folder_id
                .as_ref(),
            Some(&folder)
        );
        let first = reference(&vault, &id);
        let (results, pending) = manage(&mut vault, vec![first], serde_json::json!({"type":"copy","organizationId":null,"collectionIds":[],"folderId":null})).await;
        assert!(!pending && matches!(results[0].status, ManagementStatus::Succeeded));
        let private_copy = results[0].item_id.clone().unwrap();
        assert_ne!(private_copy, id);
        let original = vault.decrypted_view(&id).await.unwrap();
        let duplicate = vault.decrypted_view(&private_copy).await.unwrap();
        assert!(same_payload(&original.0, &duplicate.0));
        assert!(original.0.key != duplicate.0.key);
        drop((original, duplicate));
        let first = reference(&vault, &id);
        let (results, pending) = manage(&mut vault, vec![first], serde_json::json!({"type":"move","organizationId":org,"collectionIds":[collection],"folderId":folder})).await;
        assert!(!pending && matches!(results[0].status, ManagementStatus::Succeeded));
        assert_eq!(results[0].item_id.as_deref(), Some(id.as_str()));
        assert_eq!(
            vault
                .items
                .iter()
                .find(|i| i.id == id)
                .unwrap()
                .organization_id
                .as_ref(),
            Some(&org)
        );
        let first = reference(&vault, &id);
        let (results, pending) = manage(
            &mut vault,
            vec![first],
            serde_json::json!({"type":"assign_collections","collectionIds":[second_collection]}),
        )
        .await;
        assert!(!pending && matches!(results[0].status, ManagementStatus::Succeeded));
        assert_eq!(
            vault
                .items
                .iter()
                .find(|i| i.id == id)
                .unwrap()
                .collection_ids,
            vec![second_collection]
        );
        let first = reference(&vault, &id);
        let (results, pending) = manage(&mut vault, vec![first], serde_json::json!({"type":"move","organizationId":org,"collectionIds":[collection],"folderId":null})).await;
        assert!(!pending && matches!(results[0].status, ManagementStatus::Succeeded));
        let same_owner = vault.items.iter().find(|i| i.id == id).unwrap();
        assert!(same_owner.favorite && same_owner.folder_id.is_none());
        assert_eq!(same_owner.collection_ids, vec![collection]);
        let first = reference(&vault, &id);
        let (results, pending) = manage(&mut vault, vec![first], serde_json::json!({"type":"move","organizationId":null,"collectionIds":[],"folderId":null})).await;
        assert!(!pending && matches!(results[0].status, ManagementStatus::Succeeded));
        let moved = results[0].item_id.clone().unwrap();
        assert_ne!(moved, id);
        assert!(vault.items.iter().find(|i| i.id == id).unwrap().deleted);
        assert!(!vault.items.iter().find(|i| i.id == moved).unwrap().deleted);
        assert!(vault
            .items
            .iter()
            .find(|i| i.id == moved)
            .unwrap()
            .organization_id
            .is_none());
        let mut stale = reference(&vault, &private_copy);
        stale["revision"] = Value::String("2000-01-01T00:00:00Z".into());
        let valid = reference(&vault, &moved);
        let (results, pending) = manage(
            &mut vault,
            vec![stale, valid],
            serde_json::json!({"type":"assign_folder","folderId":folder}),
        )
        .await;
        assert!(!pending);
        assert!(matches!(results[0].status, ManagementStatus::Failed));
        assert_eq!(results[0].error.as_ref().unwrap().code, "item_changed");
        assert!(matches!(results[1].status, ManagementStatus::Succeeded));

        // Sync is real; only the dispatched management response is replaced.
        // A server failure must stop the batch and prevent another mutation.
        let sync_body = vault
            .http
            .get(vault.server.join("api/sync").unwrap())
            .bearer_auth(&*vault.access_token)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mock = std::thread::spawn(move || {
            use std::io::{Read, Write};
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 16384];
                let length = stream.read(&mut request).unwrap();
                let header = std::str::from_utf8(&request[..length]).unwrap();
                if index == 0 {
                    assert!(header.starts_with("GET /api/sync "));
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", sync_body.len()).unwrap();
                    stream.write_all(&sync_body).unwrap();
                } else {
                    assert!(
                        header.starts_with("PUT /api/ciphers/")
                            && header.contains("/partial HTTP/")
                    );
                    stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                }
            }
        });
        let original_server = vault.server.clone();
        vault.server = format!("http://{address}/").parse().unwrap();
        let first = reference(&vault, &private_copy);
        let second = reference(&vault, &moved);
        let (results, pending) = manage(
            &mut vault,
            vec![first, second],
            serde_json::json!({"type":"assign_folder","folderId":null}),
        )
        .await;
        mock.join().unwrap();
        assert!(pending && vault.needs_sync());
        assert!(matches!(results[0].status, ManagementStatus::Uncertain));
        assert!(matches!(results[1].status, ManagementStatus::NotAttempted));
        let first = reference(&vault, &private_copy);
        let request = serde_json::from_value(serde_json::json!({"items":[first],"operation":{"type":"assign_folder","folderId":null}})).unwrap();
        assert!(
            matches!(vault.manage_items(request).await, Err(error) if error.code == "sync_required")
        );
        vault.server = original_server;
        vault.sync().await.unwrap();
    }
}
