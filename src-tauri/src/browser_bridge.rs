//! Explicit, single-use browser filling. Credentials never enter the Veyra UI.
//!
//! The browser is a trusted credential recipient after an explicit desktop action.
//! Linux peer credentials protect the transport from other users; they do not
//! protect against arbitrary programs already running as the signed-in user.

use crate::{session::error, vaultwarden::VaultError};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::oneshot;
use uuid::Uuid;
use zeroize::Zeroizing;

const LIFETIME: Duration = Duration::from_secs(30);
const MAX_REQUEST: usize = 16 * 1024;
const MAX_RESPONSE: usize = 256 * 1024;
pub const FIREFOX_ID: &str = "companion@veyra.app";

#[derive(Default)]
pub struct BrowserBridge(Mutex<BridgeInner>);

#[derive(Default)]
struct BridgeInner {
    available: bool,
    companion_seen: bool,
    pending: Option<Pending>,
}

struct Pending {
    id: Uuid,
    item_id: String,
    target: String,
    revision: String,
    generation: u64,
    expires: Instant,
    delivered: bool,
    completion: Option<oneshot::Sender<bool>>,
}

impl Pending {
    fn matches(&self, id: &str, target: &str, generation: u64) -> bool {
        self.id.to_string() == id
            && self.target == target
            && self.generation == generation
            && Instant::now() < self.expires
    }

    fn claim(&mut self, id: &str, target: &str, generation: u64) -> bool {
        if self.delivered || !self.matches(id, target, generation) {
            return false;
        }
        self.delivered = true;
        true
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionStatus {
    supported: bool,
    connected: bool,
    message: &'static str,
}

#[tauri::command]
pub fn browser_companion_status(bridge: State<'_, BrowserBridge>) -> CompanionStatus {
    let Ok(inner) = bridge.0.lock() else {
        return CompanionStatus {
            supported: false,
            connected: false,
            message: "Browser filling is unavailable in this desktop session.",
        };
    };
    CompanionStatus {
        supported: inner.available,
        connected: inner.companion_seen,
        message: if !inner.available {
            "Browser filling requires a native Linux installation; the Flatpak bridge is unavailable."
        } else if inner.companion_seen {
            "The browser companion was detected in this desktop session."
        } else {
            "Install the Veyra browser companion and native messaging host, then visit an HTTPS page."
        },
    }
}

fn marked_url(target: &url::Url, id: Uuid) -> String {
    let mut marked = target.clone();
    let fragment = target.fragment().unwrap_or("");
    let separator = if fragment.is_empty() { "" } else { "&" };
    marked.set_fragment(Some(&format!("{fragment}{separator}__veyra_fill={id}")));
    marked.to_string()
}

#[tauri::command]
pub async fn begin_browser_fill(
    app: tauri::AppHandle,
    item_id: String,
    expected_generation: u64,
) -> Result<(), VaultError> {
    let (generation, vault, cancellation) = crate::connected(&app.state::<crate::Session>())?;
    if generation != expected_generation {
        return Err(error(
            "cancelled",
            "The vault session changed. Select the item again.",
        ));
    }
    let (mut target, revision) = {
        let vault = vault.lock().await;
        let (target, revision) = vault.browser_target(&item_id)?;
        (
            crate::urls::validate_website(&target)
                .map_err(|_| error("invalid_website", "This login needs a valid HTTPS website."))?,
            revision,
        )
    };
    if target.fragment() == Some("") {
        target.set_fragment(None);
    }
    let id = Uuid::new_v4();
    let (completion, receiver) = oneshot::channel();
    let opened = {
        let session = app.state::<crate::Session>();
        let state = session.0.lock().map_err(|_| crate::unavailable())?;
        state.require_generation(generation)?;
        state.status.require_unlocked()?;
        if !state
            .metadata
            .iter()
            .any(|item| item.id == item_id && item.kind == "login" && !item.deleted)
        {
            return Err(error("invalid_item", "Choose an available login to fill."));
        }
        {
            let bridge = app.state::<BrowserBridge>();
            let mut inner = bridge.0.lock().map_err(|_| crate::unavailable())?;
            if !inner.available || !inner.companion_seen {
                return Err(error(
                    "companion_required",
                    "Connect the Veyra browser companion before filling a website.",
                ));
            }
            if inner
                .pending
                .as_ref()
                .is_some_and(|pending| pending.expires > Instant::now())
            {
                return Err(error(
                    "fill_in_progress",
                    "A browser fill is already in progress.",
                ));
            }
            inner.pending = Some(Pending {
                id,
                item_id,
                target: target.to_string(),
                revision,
                generation,
                expires: Instant::now() + LIFETIME,
                delivered: false,
                completion: Some(completion),
            });
        }
        app.opener().open_url(marked_url(&target, id), None::<&str>)
    };
    if opened.is_err() {
        cancel(&app, id);
        return Err(error(
            "browser_unavailable",
            "The default browser could not be opened.",
        ));
    }
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(error("cancelled", "Browser filling was cancelled because the vault locked or changed.")),
        result = tokio::time::timeout(LIFETIME, receiver) => match result {
            Ok(Ok(true)) => Ok(()),
            Ok(Ok(false)) => Err(error("fill_rejected", "The companion could not safely fill a visible login form on the exact target page.")),
            _ => Err(error("fill_timeout", "No browser fill was confirmed. Check the companion, page address and login form.")),
        },
    };
    cancel(&app, id);
    result
}

fn cancel(app: &tauri::AppHandle, id: Uuid) {
    if let Ok(mut inner) = app.state::<BrowserBridge>().0.lock() {
        if inner
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id == id)
        {
            inner.pending = None;
        }
    }
}

pub fn invalidate(app: &tauri::AppHandle) {
    if let Ok(mut inner) = app.state::<BrowserBridge>().0.lock() {
        inner.pending = None;
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Hello {},
    Take {
        intent_id: String,
        target_url: String,
    },
    Ack {
        intent_id: String,
        target_url: String,
        filled: bool,
    },
}

fn read_frame(reader: &mut impl Read, maximum: usize) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut header = [0; 4];
    reader.read_exact(&mut header)?;
    let length = u32::from_ne_bytes(header) as usize;
    if length == 0 || length > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid native message length",
        ));
    }
    let mut bytes = Zeroizing::new(vec![0; length]);
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn write_frame(writer: &mut impl Write, value: &[u8]) -> io::Result<()> {
    if value.is_empty() || value.len() > MAX_RESPONSE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid native response length",
        ));
    }
    writer.write_all(&(value.len() as u32).to_ne_bytes())?;
    writer.write_all(value)?;
    writer.flush()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;
    use std::{
        fs,
        os::{
            fd::AsRawFd,
            unix::{
                fs::{FileTypeExt, MetadataExt, PermissionsExt},
                net::{UnixListener, UnixStream},
            },
        },
        path::{Path, PathBuf},
    };

    fn user_id() -> u32 {
        // SAFETY: geteuid has no preconditions and returns the current process ID.
        unsafe { libc::geteuid() }
    }

    fn private_directory(path: &Path, create: bool) -> io::Result<()> {
        if create {
            match fs::DirBuilder::new().mode(0o700).create(path) {
                Ok(()) => {}
                Err(failure) if failure.kind() == io::ErrorKind::AlreadyExists => {}
                Err(failure) => return Err(failure),
            }
        }
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.uid() != user_id() || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Private runtime directory required",
            ));
        }
        Ok(())
    }

    pub(super) fn socket_path() -> io::Result<PathBuf> {
        let parent = if let Some(path) = std::env::var_os("XDG_RUNTIME_DIR") {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            private_directory(&path, false)?;
            path
        } else {
            let path = std::env::temp_dir().join(format!("veyra-runtime-{}", user_id()));
            private_directory(&path, true)?;
            path
        };
        let directory = parent.join("veyra-browser");
        private_directory(&directory, true)?;
        Ok(directory.join("bridge.sock"))
    }

    fn require_peer_user(stream: &UnixStream) -> io::Result<()> {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: the correctly sized, initialized output buffer is valid for this call.
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut credentials as *mut _ as *mut libc::c_void,
                &mut size,
            )
        };
        if result != 0 || credentials.uid != user_id() {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        Ok(())
    }

    pub(super) fn start(app: &tauri::AppHandle) -> io::Result<()> {
        let path = socket_path()?;
        if path.exists() {
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.file_type().is_socket()
                || metadata.uid() != user_id()
                || UnixStream::connect(&path).is_ok()
            {
                return Err(io::Error::from(io::ErrorKind::AddrInUse));
            }
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        let app = app.clone();
        std::thread::Builder::new()
            .name("veyra-browser".into())
            .spawn(move || {
                // Process one short, bounded request at a time, avoiding unbounded
                // client threads. No request waits for user interaction or networking.
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { break };
                    if require_peer_user(&stream).is_err() {
                        continue;
                    }
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                    let result = handle(&app, &mut stream);
                    if result.is_err() {
                        let _ = write_frame(&mut stream, br#"{"status":"rejected"}"#);
                    }
                }
            })?;
        Ok(())
    }

    fn handle(app: &tauri::AppHandle, stream: &mut UnixStream) -> io::Result<()> {
        let bytes = read_frame(stream, MAX_REQUEST)?;
        let request: Request = serde_json::from_slice(&bytes)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        match request {
            Request::Hello {} => {
                app.state::<BrowserBridge>()
                    .0
                    .lock()
                    .map_err(|_| io::Error::other("Unavailable"))?
                    .companion_seen = true;
                write_frame(stream, br#"{"status":"ready"}"#)
            }
            Request::Take {
                intent_id,
                target_url,
            } => {
                let (item_id, generation, revision) = {
                    let session = app.state::<crate::Session>();
                    let state = session
                        .0
                        .lock()
                        .map_err(|_| io::Error::other("Unavailable"))?;
                    state
                        .status
                        .require_unlocked()
                        .map_err(|_| io::Error::from(io::ErrorKind::PermissionDenied))?;
                    let bridge = app.state::<BrowserBridge>();
                    let mut inner = bridge
                        .0
                        .lock()
                        .map_err(|_| io::Error::other("Unavailable"))?;
                    let pending = inner
                        .pending
                        .as_mut()
                        .ok_or_else(|| io::Error::from(io::ErrorKind::PermissionDenied))?;
                    // Consume before decrypting; a replay can never retrieve a second copy.
                    if !pending.claim(&intent_id, &target_url, state.status.generation) {
                        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                    }
                    (
                        pending.item_id.clone(),
                        pending.generation,
                        pending.revision.clone(),
                    )
                };
                let (credentials, _vault_guard) = tauri::async_runtime::block_on(async {
                    let (current_generation, vault, cancellation) = crate::connected(&app.state::<crate::Session>())?;
                    if current_generation != generation {
                        return Err(error("cancelled", "The vault session changed."));
                    }
                    tokio::select! {
                        biased;
                        _ = cancellation.cancelled() => Err(error("cancelled", "The vault session changed.")),
                        result = async {
                            let vault = vault.lock_owned().await;
                            let credentials = vault.browser_credentials(&item_id, &target_url, &revision).await?;
                            Ok::<_, VaultError>((credentials, vault))
                        } => result,
                    }
                })
                .map_err(|_| io::Error::from(io::ErrorKind::PermissionDenied))?;
                #[derive(Serialize)]
                struct FilledCredentials<'a> {
                    status: &'static str,
                    username: &'a str,
                    password: &'a str,
                }
                let mut response = Zeroizing::new(Vec::new());
                serde_json::to_writer(
                    &mut *response,
                    &FilledCredentials {
                        status: "credentials",
                        username: &credentials.0,
                        password: &credentials.1,
                    },
                )
                .map_err(|_| io::Error::other("Serialization failed"))?;
                let session = app.state::<crate::Session>();
                let state = session
                    .0
                    .lock()
                    .map_err(|_| io::Error::other("Unavailable"))?;
                state
                    .require_generation(generation)
                    .and_then(|()| state.status.require_unlocked())
                    .map_err(|_| io::Error::from(io::ErrorKind::PermissionDenied))?;
                let bridge = app.state::<BrowserBridge>();
                let inner = bridge
                    .0
                    .lock()
                    .map_err(|_| io::Error::other("Unavailable"))?;
                if !inner
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.matches(&intent_id, &target_url, generation))
                {
                    return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                }
                // Like clipboard writes, serialize the bounded commit with lock.
                write_frame(stream, &response)
            }
            Request::Ack {
                intent_id,
                target_url,
                filled,
            } => {
                let session = app.state::<crate::Session>();
                let state = session
                    .0
                    .lock()
                    .map_err(|_| io::Error::other("Unavailable"))?;
                state
                    .status
                    .require_unlocked()
                    .map_err(|_| io::Error::from(io::ErrorKind::PermissionDenied))?;
                let bridge = app.state::<BrowserBridge>();
                let mut inner = bridge
                    .0
                    .lock()
                    .map_err(|_| io::Error::other("Unavailable"))?;
                if !inner.pending.as_ref().is_some_and(|pending| {
                    (pending.delivered || !filled)
                        && pending.matches(&intent_id, &target_url, state.status.generation)
                }) {
                    return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                }
                if let Some(mut pending) = inner.pending.take() {
                    if let Some(completion) = pending.completion.take() {
                        let _ = completion.send(filled);
                    }
                }
                write_frame(stream, br#"{"status":"done"}"#)
            }
        }
    }

    fn chromium_allowlist() -> io::Result<Vec<String>> {
        let root = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|path| PathBuf::from(path).join(".config")))
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let directory = root.join("veyra");
        private_directory(&directory, false)?;
        let path = directory.join("browser-companion.json");
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.uid() != user_id()
            || metadata.mode() & 0o077 != 0
            || metadata.len() > MAX_REQUEST as u64
        {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Allowlist {
            chromium_extension_ids: Vec<String>,
        }
        let config: Allowlist = serde_json::from_slice(&fs::read(path)?)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        Ok(config.chromium_extension_ids)
    }

    fn caller_allowed(arguments: &[String], chromium: &[String]) -> bool {
        if arguments.len() == 2
            && arguments[1] == FIREFOX_ID
            && Path::new(&arguments[0]).is_absolute()
        {
            return true;
        }
        if arguments.len() == 1 {
            return chromium.iter().any(|id| {
                id.len() == 32
                    && id.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
                    && (arguments[0] == format!("chrome-extension://{id}")
                        || arguments[0] == format!("chrome-extension://{id}/"))
            });
        }
        false
    }

    pub(super) fn native_host() -> io::Result<()> {
        let arguments = std::env::args().skip(1).collect::<Vec<_>>();
        if !caller_allowed(&arguments, &chromium_allowlist().unwrap_or_default()) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let request = read_frame(&mut io::stdin().lock(), MAX_REQUEST)?;
        // Reject malformed or extra fields before forwarding to the desktop.
        serde_json::from_slice::<Request>(&request)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        let mut stream = UnixStream::connect(socket_path()?)?;
        require_peer_user(&stream)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        write_frame(&mut stream, &request)?;
        let response = read_frame(&mut stream, MAX_RESPONSE)?;
        write_frame(&mut io::stdout().lock(), &response)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn caller_allowlist_is_exact_and_browser_specific() {
            let id = "abcdefghijklmnopabcdefghijklmnop".to_owned();
            assert!(caller_allowed(
                &[format!("chrome-extension://{id}/")],
                std::slice::from_ref(&id)
            ));
            assert!(!caller_allowed(
                &[format!("https://{id}/")],
                std::slice::from_ref(&id)
            ));
            assert!(!caller_allowed(
                &[format!("chrome-extension://{id}.evil/")],
                &[id]
            ));
            assert!(caller_allowed(
                &["/tmp/host.json".into(), FIREFOX_ID.into()],
                &[]
            ));
            assert!(!caller_allowed(
                &["/tmp/host.json".into(), "other@extension".into()],
                &[]
            ));
            assert!(!caller_allowed(&[], &[]));
        }

        #[test]
        fn peer_credentials_accept_our_uid() {
            let (left, right) = UnixStream::pair().unwrap();
            assert!(require_peer_user(&left).is_ok());
            assert!(require_peer_user(&right).is_ok());
        }
    }
}

pub fn initialize(app: &tauri::AppHandle) {
    #[cfg(target_os = "linux")]
    if !std::path::Path::new("/.flatpak-info").exists() && linux::start(app).is_ok() {
        if let Ok(mut inner) = app.state::<BrowserBridge>().0.lock() {
            inner.available = true;
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
}

/// Dedicated stdin/stdout native host entry point; never initializes GTK or logs messages.
pub fn native_host() {
    #[cfg(target_os = "linux")]
    let result = linux::native_host();
    #[cfg(not(target_os = "linux"))]
    let result = Err::<(), _>(io::Error::from(io::ErrorKind::Unsupported));
    if result.is_err() {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn native_framing_is_bounded_and_rejects_truncated_or_unknown_messages() {
        let mut frame = Vec::new();
        write_frame(&mut frame, br#"{"type":"hello"}"#).unwrap();
        let body = read_frame(&mut Cursor::new(frame), MAX_REQUEST).unwrap();
        assert!(matches!(
            serde_json::from_slice::<Request>(&body).unwrap(),
            Request::Hello {}
        ));
        assert!(read_frame(&mut Cursor::new(u32::MAX.to_ne_bytes()), MAX_REQUEST).is_err());
        assert!(read_frame(&mut Cursor::new([2, 0, 0, 0, b'{']), MAX_REQUEST).is_err());
        assert!(
            serde_json::from_str::<Request>(r#"{"type":"hello","url":"https://evil.test"}"#)
                .is_err()
        );
    }

    #[test]
    fn intent_matches_full_target_generation_and_lifetime() {
        let pending = Pending {
            id: Uuid::new_v4(),
            item_id: "item".into(),
            target: "https://example.test/login?q=1#route".into(),
            revision: "revision".into(),
            generation: 4,
            expires: Instant::now() + LIFETIME,
            delivered: false,
            completion: None,
        };
        let id = pending.id.to_string();
        assert!(pending.matches(&id, &pending.target, 4));
        for target in [
            "https://example.test/other?q=1#route",
            "https://example.test/login?q=2#route",
            "https://example.test.evil/login?q=1#route",
            "http://example.test/login?q=1#route",
            "https://example.test/login?q=1#other",
        ] {
            assert!(!pending.matches(&id, target, 4));
        }
        assert!(!pending.matches(&Uuid::new_v4().to_string(), &pending.target, 4));
        assert!(!pending.matches(&id, &pending.target, 5));
        let expired = Pending {
            expires: Instant::now() - Duration::from_secs(1),
            ..pending
        };
        assert!(!expired.matches(&id, &expired.target, 4));
    }

    #[test]
    fn a_claim_is_single_use_and_a_lock_invalidates_its_generation() {
        let session = crate::Session::default();
        let mut state = session.0.lock().unwrap();
        state.status.locked = false;
        let generation = state.status.generation;
        let mut pending = Pending {
            id: Uuid::new_v4(),
            item_id: "item".into(),
            target: "https://example.test/login".into(),
            revision: "revision".into(),
            generation,
            expires: Instant::now() + LIFETIME,
            delivered: false,
            completion: None,
        };
        let id = pending.id.to_string();
        let target = pending.target.clone();
        assert!(!pending.claim(&id, "https://example.test/other", generation));
        assert!(pending.claim(&id, &target, generation));
        assert!(!pending.claim(&id, &target, generation));
        state.invalidate();
        assert!(state.status.require_unlocked().is_err());
        assert!(!pending.matches(&id, &target, state.status.generation));
    }

    #[test]
    fn request_marker_preserves_existing_fragment_and_query() {
        let id = Uuid::new_v4();
        let target = url::Url::parse("https://example.test/login?q=1#route").unwrap();
        assert_eq!(
            marked_url(&target, id),
            format!("https://example.test/login?q=1#route&__veyra_fill={id}")
        );
    }
}
