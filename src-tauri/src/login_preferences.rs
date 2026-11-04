//! Explicitly remembered sign-in identifiers. Never accept or store credentials.
use crate::vaultwarden::{server_url, VaultError};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tauri::{Emitter, Manager};

const FILE_NAME: &str = "login-preferences.json";
const MAX_FILE_BYTES: u64 = 4096;

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginPreferences {
    pub server_url: String,
    pub email: String,
}

impl LoginPreferences {
    fn normalized(self) -> Result<Self, VaultError> {
        let server_url = server_url(&self.server_url, false)?.to_string();
        let email = self.email.trim().to_lowercase();
        let valid_email = email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && !domain.is_empty() && !domain.contains('@')
        });
        if server_url.len() > 2048
            || self.email.len() > 320
            || email.len() > 320
            || !valid_email
            || email.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(VaultError::new(
                "invalid_login_preferences",
                "Enter a valid server address and account email within the supported size limits.",
            ));
        }
        Ok(Self { server_url, email })
    }
}

fn storage_error() -> VaultError {
    VaultError::new(
        "login_preferences_unavailable",
        "Saved sign-in details could not be accessed. Your vault credentials were not stored.",
    )
}

fn invalid_file() -> VaultError {
    VaultError::new(
        "invalid_login_preferences",
        "Saved sign-in details are invalid. Forget them and enter the server and email again.",
    )
}

/// Every operation is confined to one fixed file below Tauri's app config directory.
struct Store {
    directory: PathBuf,
}

impl Store {
    fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    fn directory(&self, create: bool) -> Result<bool, VaultError> {
        match fs::symlink_metadata(&self.directory) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err(storage_error()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
                return Ok(false);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                builder.recursive(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder
                    .create(&self.directory)
                    .map_err(|_| storage_error())?;
            }
            Err(_) => return Err(storage_error()),
        }
        // Recheck after creation, including concurrent initial saves.
        let metadata = fs::symlink_metadata(&self.directory).map_err(|_| storage_error())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(storage_error());
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
            let directory = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&self.directory)
                .map_err(|_| storage_error())?;
            // SAFETY: geteuid has no arguments or memory-safety preconditions.
            if directory.metadata().map_err(|_| storage_error())?.uid()
                != unsafe { libc::geteuid() }
            {
                return Err(storage_error());
            }
            directory
                .set_permissions(fs::Permissions::from_mode(0o700))
                .map_err(|_| storage_error())?;
        }
        #[cfg(all(unix, not(target_os = "linux")))]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))
                .map_err(|_| storage_error())?;
        }
        Ok(true)
    }

    fn path(&self) -> PathBuf {
        self.directory.join(FILE_NAME)
    }

    fn load(&self) -> Result<Option<LoginPreferences>, VaultError> {
        if !self.directory(false)? {
            return Ok(None);
        }
        let path = self.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(storage_error()),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(storage_error());
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let file = options.open(path).map_err(|_| storage_error())?;
        let metadata = file.metadata().map_err(|_| storage_error())?;
        if !metadata.is_file() {
            return Err(storage_error());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.nlink() != 1 {
                return Err(storage_error());
            }
            #[cfg(target_os = "linux")]
            // SAFETY: geteuid has no arguments or memory-safety preconditions.
            if metadata.uid() != unsafe { libc::geteuid() } {
                return Err(storage_error());
            }
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| storage_error())?;
        }
        if metadata.len() > MAX_FILE_BYTES {
            return Err(invalid_file());
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| storage_error())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(invalid_file());
        }
        let preferences: LoginPreferences =
            serde_json::from_slice(&bytes).map_err(|_| invalid_file())?;
        preferences
            .normalized()
            .map(Some)
            .map_err(|_| invalid_file())
    }

    fn save(&self, preferences: LoginPreferences) -> Result<LoginPreferences, VaultError> {
        // Validate before touching an existing file. The DTO rejects all extra
        // fields, so password/token/key fields can never be serialized here.
        let preferences = preferences.normalized()?;
        let bytes = serde_json::to_vec(&preferences).map_err(|_| storage_error())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(invalid_file());
        }
        self.directory(true)?;
        match fs::symlink_metadata(self.path()) {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                return Err(storage_error());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(storage_error()),
        }
        let pending = PendingFile(
            self.directory
                .join(format!(".login-preferences-{}.tmp", uuid::Uuid::new_v4())),
        );
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options.open(&pending.0).map_err(|_| storage_error())?;
        file.write_all(&bytes).map_err(|_| storage_error())?;
        file.sync_all().map_err(|_| storage_error())?;
        drop(file);
        fs::rename(&pending.0, self.path()).map_err(|_| storage_error())?;
        sync_directory(&self.directory);
        Ok(preferences)
    }

    fn clear(&self) -> Result<(), VaultError> {
        if !self.directory(false)? {
            return Ok(());
        }
        // Unlinking a symlink does not follow or alter its target. This also
        // provides recovery from a malformed/oversized or symlinked saved file.
        match fs::remove_file(self.path()) {
            Ok(()) => sync_directory(&self.directory),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(storage_error()),
        }
        Ok(())
    }
}

struct PendingFile(PathBuf);

impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn sync_directory(path: &Path) {
    // The file is flushed before the atomic replacement. Directory flushing is
    // best effort because some filesystems/platforms do not support it.
    if let Ok(directory) = File::open(path) {
        let _ = directory.sync_all();
    }
}

fn store(app: &tauri::AppHandle) -> Result<Store, VaultError> {
    app.path()
        .app_config_dir()
        .map(Store::new)
        .map_err(|_| storage_error())
}

#[tauri::command]
pub async fn load_login_preferences(
    app: tauri::AppHandle,
) -> Result<Option<LoginPreferences>, VaultError> {
    let store = store(&app)?;
    tauri::async_runtime::spawn_blocking(move || store.load())
        .await
        .map_err(|_| storage_error())?
}

#[tauri::command]
pub async fn save_login_preferences(
    app: tauri::AppHandle,
    preferences: LoginPreferences,
) -> Result<LoginPreferences, VaultError> {
    let store = store(&app)?;
    let preferences = tauri::async_runtime::spawn_blocking(move || store.save(preferences))
        .await
        .map_err(|_| storage_error())??;
    let _ = app.emit("login-preferences:changed", ());
    Ok(preferences)
}

#[tauri::command]
pub async fn clear_login_preferences(app: tauri::AppHandle) -> Result<(), VaultError> {
    let store = store(&app)?;
    tauri::async_runtime::spawn_blocking(move || store.clear())
        .await
        .map_err(|_| storage_error())??;
    let _ = app.emit("login-preferences:changed", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "veyra-login-preferences-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn store(&self) -> Store {
            Store::new(self.0.join("app-config"))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn preferences() -> LoginPreferences {
        LoginPreferences {
            server_url: "https://Vault.Example.test/nested/vault".into(),
            email: " Synthetic.User@Example.test ".into(),
        }
    }

    #[test]
    fn remembers_exactly_normalized_identifiers_replaces_and_forgets_them() {
        let fixture = Fixture::new();
        let store = fixture.store();
        assert!(store.load().unwrap().is_none());
        let saved = store.save(preferences()).unwrap();
        assert_eq!(saved.server_url, "https://vault.example.test/nested/vault/");
        assert_eq!(saved.email, "synthetic.user@example.test");
        assert!(store.load().unwrap().unwrap() == saved);
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        let keys: Vec<_> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, vec!["email", "serverUrl"]);
        let next = LoginPreferences {
            server_url: "https://other.example.test/".into(),
            email: "another@example.test".into(),
        };
        store.save(next.clone()).unwrap();
        assert!(store.load().unwrap().unwrap() == next);
        assert_eq!(fs::read_dir(&store.directory).unwrap().count(), 1);
        store.clear().unwrap();
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn rejects_extra_secret_fields_unsafe_urls_and_unbounded_or_corrupt_files() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let saved = store.save(preferences()).unwrap();
        assert!(serde_json::from_value::<LoginPreferences>(serde_json::json!({
            "serverUrl": "https://vault.example.test", "email": "synthetic@example.test", "masterPassword": "must never be stored"
        })).is_err());
        for server in [
            "http://127.0.0.1:8222",
            "http://vault.example.test",
            "https://user:secret@vault.example.test",
            "https://vault.example.test/?token=secret",
            "https://vault.example.test/#secret",
            "https://vault.example.test/\n",
        ] {
            assert!(store
                .save(LoginPreferences {
                    server_url: server.into(),
                    email: "synthetic@example.test".into()
                })
                .is_err());
        }
        for email in [
            "",
            "@example.test",
            "synthetic@",
            "synthetic@@example.test",
            "a b@example.test",
            "a\nb@example.test",
        ] {
            assert!(store
                .save(LoginPreferences {
                    server_url: saved.server_url.clone(),
                    email: email.into()
                })
                .is_err());
        }
        assert!(store
            .save(LoginPreferences {
                server_url: saved.server_url.clone(),
                email: "x".repeat(321)
            })
            .is_err());
        assert!(store.load().unwrap().unwrap() == saved);
        for body in [vec![b'x'; MAX_FILE_BYTES as usize + 1], b"{broken".to_vec(), br#"{"serverUrl":"https://vault.example.test/","email":"synthetic@example.test","password":"unexpected"}"#.to_vec()] {
            fs::write(store.path(), body).unwrap();
            assert!(store.load().is_err());
        }
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn uses_private_permissions_and_never_follows_preference_symlinks() {
        use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
        let fixture = Fixture::new();
        let store = fixture.store();
        store.save(preferences()).unwrap();
        assert_eq!(
            fs::metadata(&store.directory).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::metadata(store.path()).unwrap().mode() & 0o777, 0o600);
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644)).unwrap();
        store.load().unwrap();
        assert_eq!(fs::metadata(store.path()).unwrap().mode() & 0o777, 0o600);
        store.clear().unwrap();
        let target = fixture.0.join("unrelated-file");
        fs::write(&target, b"Unrelated data must remain untouched.").unwrap();
        symlink(&target, store.path()).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(preferences()).is_err());
        store.clear().unwrap();
        assert_eq!(
            fs::read(&target).unwrap(),
            b"Unrelated data must remain untouched."
        );
        let linked_directory = fixture.0.join("linked-config");
        symlink(&store.directory, &linked_directory).unwrap();
        let linked_store = Store::new(linked_directory);
        assert!(linked_store.save(preferences()).is_err());
        assert!(linked_store.load().is_err());
        assert!(linked_store.clear().is_err());
    }
}
