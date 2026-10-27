mod clipboard;
mod demo;
mod session;
mod urls;
mod vaultwarden;

use clipboard::ClipboardService;
use serde::{Deserialize, Serialize};
use session::{error, ConnectedVault, MetadataSnapshot, Session, SessionStatus};
use std::sync::Arc;
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex as AsyncMutex;
use vaultwarden::{Field, TwoFactorInput, VaultError, Vaultwarden};
use zeroize::Zeroizing;

fn current(session: &Session) -> Result<SessionStatus, VaultError> {
    session
        .0
        .lock()
        .map(|state| state.status.clone())
        .map_err(|_| unavailable())
}
fn unavailable() -> VaultError {
    error(
        "session_unavailable",
        "The desktop session is unavailable. Please restart Veyra.",
    )
}
fn changed(app: &tauri::AppHandle, status: &SessionStatus) {
    // No credentials, item metadata or server responses are emitted globally.
    let _ = app.emit("vault:changed", status);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoginRequest {
    server_url: String,
    email: String,
    master_password: String,
    two_factor: Option<FactorRequest>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactorRequest {
    provider: String,
    code: String,
}
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum LoginOutcome {
    Authenticated {
        session: SessionStatus,
    },
    TwoFactorRequired {
        providers: Vec<&'static str>,
        session: SessionStatus,
    },
}

#[tauri::command]
async fn session_status(session: State<'_, Session>) -> Result<SessionStatus, VaultError> {
    current(&session)
}

#[tauri::command]
async fn login_vault(
    app: tauri::AppHandle,
    mut request: LoginRequest,
) -> Result<LoginOutcome, VaultError> {
    let password = Zeroizing::new(std::mem::take(&mut request.master_password));
    let factor = match request.two_factor.take() {
        Some(mut input) => {
            let token = Zeroizing::new(std::mem::take(&mut input.code));
            if input.provider != "totp" {
                return Err(error(
                    "unsupported_two_factor",
                    "Only authenticator codes are currently supported.",
                ));
            }
            Some(TwoFactorInput {
                provider: 0,
                token: token.to_string(),
            })
        }
        None => None,
    };
    let (generation, cancellation, retired, locked_status) = {
        let session = app.state::<Session>();
        let mut state = session.0.lock().map_err(|_| unavailable())?;
        let retired = state.invalidate();
        (
            state.status.generation,
            state.cancellation.clone(),
            retired,
            state.status.clone(),
        )
    };
    app.state::<ClipboardService>().clear();
    changed(&app, &locked_status);
    session::retire(retired).await;
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(error("cancelled", "Sign-in was cancelled because the session changed.")),
        result = Vaultwarden::connect(request.server_url, request.email, password, factor) => result,
    };
    let session = app.state::<Session>();
    let mut state = session.0.lock().map_err(|_| unavailable())?;
    state.require_generation(generation)?;
    match result {
        Ok(vault) => {
            let expires_in = vault.expires_in();
            state.metadata = vault.metadata();
            state.unsupported_items = vault.skipped_items;
            state.vault = Some(Arc::new(AsyncMutex::new(vault)));
            state.status.locked = false;
            state.status.mode = "connected";
            state.status.last_sync = Some(chrono::Utc::now().to_rfc3339());
            let status = state.status.clone();
            drop(state);
            changed(&app, &status);
            let expiry_app = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {},
                    _ = tokio::time::sleep(expires_in) => { let _ = expire_session(&expiry_app, generation); },
                }
            });
            Ok(LoginOutcome::Authenticated { session: status })
        }
        Err(failure) if failure.code == "two_factor_required" && failure.providers.contains(&0) => {
            Ok(LoginOutcome::TwoFactorRequired {
                providers: vec!["totp"],
                session: state.status.clone(),
            })
        }
        Err(failure) => Err(failure),
    }
}

#[tauri::command]
async fn unlock_demo(app: tauri::AppHandle) -> Result<SessionStatus, VaultError> {
    let (retired, status) = {
        let session = app.state::<Session>();
        let mut state = session.0.lock().map_err(|_| unavailable())?;
        let retired = state.invalidate();
        state.status.locked = false;
        state.status.mode = "demo";
        (retired, state.status.clone())
    };
    app.state::<ClipboardService>().clear();
    changed(&app, &status);
    session::retire(retired).await;
    Ok(status)
}

#[tauri::command]
async fn lock_vault(app: tauri::AppHandle) -> Result<SessionStatus, VaultError> {
    let (retired, status) = {
        let session = app.state::<Session>();
        let mut state = session.0.lock().map_err(|_| unavailable())?;
        let retired = state.invalidate();
        (retired, state.status.clone())
    };
    app.state::<ClipboardService>().clear();
    changed(&app, &status);
    session::retire(retired).await;
    Ok(status)
}

#[tauri::command]
async fn list_metadata(session: State<'_, Session>) -> Result<MetadataSnapshot, VaultError> {
    session
        .0
        .lock()
        .map(|state| state.snapshot())
        .map_err(|_| unavailable())
}

fn connected(
    session: &Session,
) -> Result<(u64, ConnectedVault, tokio_util::sync::CancellationToken), VaultError> {
    let state = session.0.lock().map_err(|_| unavailable())?;
    state.status.require_unlocked()?;
    let vault = state.vault.clone().ok_or_else(|| {
        error(
            "not_connected",
            "Connect a Vaultwarden account to use this action.",
        )
    })?;
    Ok((state.status.generation, vault, state.cancellation.clone()))
}

#[tauri::command]
async fn sync_vault(app: tauri::AppHandle) -> Result<MetadataSnapshot, VaultError> {
    let (generation, vault, cancellation) = connected(&app.state::<Session>())?;
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(error("cancelled", "Synchronization was cancelled because the session changed.")),
        result = async {
            let mut vault = vault.lock().await;
            vault.sync().await?;
            Ok::<_, VaultError>((vault.metadata(), vault.skipped_items))
        } => result,
    };
    let session = app.state::<Session>();
    let mut state = session.0.lock().map_err(|_| unavailable())?;
    state.require_generation(generation)?;
    state.status.require_unlocked()?;
    match result {
        Ok((items, unsupported)) => {
            state.metadata = items;
            state.unsupported_items = unsupported;
            state.status.last_sync = Some(chrono::Utc::now().to_rfc3339());
            let snapshot = state.snapshot();
            drop(state);
            changed(&app, &snapshot.session);
            Ok(snapshot)
        }
        Err(failure) if failure.code == "session_expired" => {
            let retired = state.invalidate();
            let status = state.status.clone();
            drop(state);
            app.state::<ClipboardService>().clear();
            changed(&app, &status);
            tauri::async_runtime::spawn(session::retire(retired));
            Err(failure)
        }
        Err(failure) => Err(failure),
    }
}

fn expire_session(app: &tauri::AppHandle, generation: u64) -> Result<(), VaultError> {
    let session = app.state::<Session>();
    let mut state = session.0.lock().map_err(|_| unavailable())?;
    state.require_generation(generation)?;
    let retired = state.invalidate();
    let status = state.status.clone();
    drop(state);
    app.state::<ClipboardService>().clear();
    changed(app, &status);
    tauri::async_runtime::spawn(session::retire(retired));
    Ok(())
}

async fn read_field(
    app: &tauri::AppHandle,
    item_id: &str,
    field: Field,
) -> Result<(u64, Zeroizing<String>), VaultError> {
    let (generation, mode, vault, cancellation) = {
        let session = app.state::<Session>();
        let state = session.0.lock().map_err(|_| unavailable())?;
        state.status.require_unlocked()?;
        (
            state.status.generation,
            state.status.mode,
            state.vault.clone(),
            state.cancellation.clone(),
        )
    };
    let value = if mode == "demo" {
        demo::field(item_id, field)
            .map_err(|_| error("item_not_found", "This demonstration item is unavailable."))?
    } else {
        let vault = vault.ok_or_else(|| {
            error(
                "not_connected",
                "Sign in to your Vaultwarden account first.",
            )
        })?;
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(error("cancelled", "Field access was cancelled because the session changed.")),
            result = async { vault.lock().await.field(item_id, field).await } => match result {
                Ok(value) => value,
                Err(failure) => {
                    if failure.code == "session_expired" { expire_session(app, generation)?; }
                    return Err(failure);
                },
            },
        }
    };
    let session = app.state::<Session>();
    let state = session.0.lock().map_err(|_| unavailable())?;
    state.require_generation(generation)?;
    state.status.require_unlocked()?;
    Ok((generation, value))
}

#[tauri::command]
async fn copy_field(
    app: tauri::AppHandle,
    item_id: String,
    field: Field,
) -> Result<(), VaultError> {
    let (generation, value) = read_field(&app, &item_id, field).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let session = app.state::<Session>();
        // Serialize committing the clipboard value with locking. A later lock queues
        // conditional clearing immediately after this bounded write completes.
        let state = session.0.lock().map_err(|_| unavailable())?;
        state.require_generation(generation)?;
        state.status.require_unlocked()?;
        app.state::<ClipboardService>().write(value).map_err(|_| {
            error(
                "clipboard_unavailable",
                "Clipboard access failed in this desktop session.",
            )
        })
    })
    .await
    .map_err(|_| {
        error(
            "clipboard_unavailable",
            "Clipboard operation did not complete.",
        )
    })?
}

#[derive(Serialize)]
struct RevealedField {
    value: String,
    generation: u64,
}
#[tauri::command]
async fn reveal_field(
    app: tauri::AppHandle,
    item_id: String,
    field: Field,
) -> Result<RevealedField, VaultError> {
    let (generation, value) = read_field(&app, &item_id, field).await?;
    Ok(RevealedField {
        value: value.to_string(),
        generation,
    })
}

#[tauri::command]
fn show_quick_access(app: tauri::AppHandle) -> Result<(), VaultError> {
    let window = if let Some(window) = app.get_webview_window("quick-access") {
        window
    } else {
        WebviewWindowBuilder::new(
            &app,
            "quick-access",
            WebviewUrl::App("index.html?window=quick".into()),
        )
        .title("Veyra Quick Access")
        .inner_size(720.0, 520.0)
        .min_inner_size(520.0, 380.0)
        .center()
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .visible(false)
        .build()
        .map_err(|_| {
            error(
                "window_unavailable",
                "Quick Access window could not be created.",
            )
        })?
    };
    window.show().map_err(|_| {
        error(
            "window_unavailable",
            "Quick Access window could not be shown.",
        )
    })?;
    window.set_focus().map_err(|_| {
        error(
            "window_focus_denied",
            "The compositor did not allow focusing Quick Access.",
        )
    })?;
    window.emit("quick-access:focus", ()).map_err(|_| {
        error(
            "window_unavailable",
            "Quick Access focus event could not be delivered.",
        )
    })?;
    Ok(())
}

#[tauri::command]
fn hide_quick_access(app: tauri::AppHandle) -> Result<(), VaultError> {
    if let Some(window) = app.get_webview_window("quick-access") {
        window.hide().map_err(|_| {
            error(
                "window_unavailable",
                "Quick Access window could not be hidden.",
            )
        })?;
    }
    Ok(())
}

#[tauri::command]
async fn open_main_item(app: tauri::AppHandle, item_id: String) -> Result<(), VaultError> {
    {
        let session = app.state::<Session>();
        let state = session.0.lock().map_err(|_| unavailable())?;
        state.status.require_unlocked()?;
        if state.status.mode == "demo" {
            demo::field(&item_id, Field::Username)
                .map_err(|_| error("item_not_found", "This demonstration item is unavailable."))?;
        } else if !state.metadata.iter().any(|item| item.id == item_id) {
            return Err(error("item_not_found", "This item is unavailable."));
        }
    }
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| error("window_unavailable", "The main window is unavailable."))?;
    main.show()
        .map_err(|_| error("window_unavailable", "The main window could not be shown."))?;
    hide_quick_access(app.clone())?;
    main.set_focus().map_err(|_| {
        error(
            "window_focus_denied",
            "The compositor did not allow focusing the main window.",
        )
    })?;
    main.emit("vault:open-item", item_id).map_err(|_| {
        error(
            "window_unavailable",
            "The selected item could not be opened.",
        )
    })?;
    Ok(())
}

#[tauri::command]
async fn open_website(app: tauri::AppHandle, url: String) -> Result<(), VaultError> {
    let validated = urls::validate_website(&url).map_err(|_| {
        error(
            "invalid_website",
            "Only a valid HTTPS website without embedded credentials may be opened.",
        )
    })?;
    let (generation, known_id, vault, cancellation) = {
        let session = app.state::<Session>();
        let state = session.0.lock().map_err(|_| unavailable())?;
        state.status.require_unlocked()?;
        let known_id = if state.status.mode == "demo" {
            None
        } else {
            Some(
                state
                    .metadata
                    .iter()
                    .find(|item| item.url == validated.as_str())
                    .map(|item| item.id.clone())
                    .ok_or_else(|| {
                        error(
                            "invalid_website",
                            "Open a website associated with an available vault item.",
                        )
                    })?,
            )
        };
        (
            state.status.generation,
            known_id,
            state.vault.clone(),
            state.cancellation.clone(),
        )
    };
    if let Some(item_id) = known_id {
        let vault = vault.ok_or_else(|| {
            error(
                "not_connected",
                "Sign in to your Vaultwarden account first.",
            )
        })?;
        let checked = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(error("cancelled", "Website opening was cancelled because the session changed.")),
            result = async { vault.lock().await.url(&item_id) } => result,
        };
        if let Err(failure) = checked {
            if failure.code == "session_expired" {
                expire_session(&app, generation)?;
            }
            return Err(failure);
        }
    }
    let session = app.state::<Session>();
    let state = session.0.lock().map_err(|_| unavailable())?;
    state.require_generation(generation)?;
    state.status.require_unlocked()?;
    app.opener()
        .open_url(validated.as_str(), None::<&str>)
        .map_err(|_| {
            error(
                "browser_unavailable",
                "The default browser could not be opened.",
            )
        })
}

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Session::default())
        .manage(ClipboardService::default())
        .invoke_handler(tauri::generate_handler![
            session_status,
            login_vault,
            unlock_demo,
            lock_vault,
            list_metadata,
            sync_vault,
            copy_field,
            reveal_field,
            show_quick_access,
            hide_quick_access,
            open_website,
            open_main_item
        ])
        .run(tauri::generate_context!());
    if result.is_err() {
        eprintln!("Veyra could not initialize the desktop runtime.");
        std::process::exit(1);
    }
}
