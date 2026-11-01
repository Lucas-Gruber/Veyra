//! Rust-owned desktop lifetime and tray actions. No vault data reaches the tray.

use serde::Serialize;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
#[cfg(not(target_os = "macos"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

const TRAY_ID: &str = "veyra-tray";
const OPEN: &str = "veyra-tray-open";
const QUICK: &str = "veyra-tray-quick";
const LOCK: &str = "veyra-tray-lock";
const QUIT: &str = "veyra-tray-quit";

#[derive(Default)]
pub struct DesktopService {
    tray_created: AtomicBool,
    quitting: AtomicBool,
    exit_ready: AtomicBool,
}

pub fn require_running(app: &tauri::AppHandle) -> Result<(), super::VaultError> {
    if app
        .state::<DesktopService>()
        .quitting
        .load(Ordering::Acquire)
    {
        Err(super::session::error(
            "shutting_down",
            "Veyra is shutting down.",
        ))
    } else {
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayAvailability {
    Ready,
    HostUnavailable,
    Unavailable,
}

#[derive(Serialize)]
pub struct DesktopIntegrationStatus {
    tray: TrayAvailability,
    message: &'static str,
}

#[tauri::command]
pub async fn desktop_integration_status(app: tauri::AppHandle) -> DesktopIntegrationStatus {
    if !host_available().await {
        return DesktopIntegrationStatus { tray: TrayAvailability::HostUnavailable, message: "No StatusNotifier tray host is available. Enable your desktop's tray support (GNOME needs an AppIndicator extension) and restart Veyra. Veyra stays running when closed; reopen it using the launcher or Quick Access shortcut." };
    }
    if !app
        .state::<DesktopService>()
        .tray_created
        .load(Ordering::Acquire)
    {
        return DesktopIntegrationStatus { tray: TrayAvailability::Unavailable, message: "The tray could not be initialized. Restart Veyra after enabling desktop tray support. Closing the window keeps Veyra running; reopen it using the launcher or Quick Access shortcut." };
    }
    DesktopIntegrationStatus { tray: TrayAvailability::Ready, message: "The tray icon is created and your desktop reports tray support. Closing the main window keeps the vault session running. Use Lock vault to sign out, or Quit Veyra to exit." }
}

#[cfg(target_os = "linux")]
async fn host_available() -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        let connection = zbus::Connection::session().await.ok()?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
        )
        .await
        .ok()?;
        proxy
            .get_property::<bool>("IsStatusNotifierHostRegistered")
            .await
            .ok()
    })
    .await
    .ok()
    .flatten()
    .unwrap_or(false)
}

#[cfg(not(target_os = "linux"))]
async fn host_available() -> bool {
    true
}

pub fn initialize(app: &tauri::AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, OPEN, "Open Veyra", true, None::<&str>)?;
    let quick = MenuItem::with_id(app, QUICK, "Quick Access", true, None::<&str>)?;
    let lock = MenuItem::with_id(app, LOCK, "Lock vault", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT, "Quit Veyra", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quick, &lock, &separator, &quit])?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_image(app)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Veyra")
        .menu(&menu)
        // macOS menu-bar items conventionally open their menu on a left click.
        // Windows/Linux can restore the window directly; right click keeps the menu.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            OPEN => {
                let _ = super::show_main_window(app.clone());
            }
            QUICK => {
                super::quick_launch::show(app, super::quick_launch::Launch::QuickAccess);
            }
            LOCK => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = super::lock_vault(app).await;
                });
            }
            QUIT => request_quit(app, 0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            #[cfg(not(target_os = "macos"))]
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let _ = super::show_main_window(tray.app_handle().clone());
            }
            #[cfg(target_os = "macos")]
            let _ = (tray, event);
        })
        .build(app)?;
    // Tauri retains the registered icon in its resource manager.
    app.state::<DesktopService>()
        .tray_created
        .store(true, Ordering::Release);
    Ok(())
}

fn tray_image(app: &tauri::AppHandle) -> tauri::Result<Image<'static>> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        Ok(tauri::include_image!("icons/tray-template.png"))
    }
    #[cfg(not(target_os = "macos"))]
    app.default_window_icon()
        .cloned()
        .map(Image::to_owned)
        .ok_or_else(|| tauri::Error::AssetNotFound("Veyra application icon".into()))
}

pub fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if !matches!(window.label(), "main" | "quick-access") {
        return;
    }
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        // Both windows remain reusable. Hiding intentionally does not sign out.
        api.prevent_close();
        if window.label() == "quick-access" {
            let _ = window.emit("quick-access:blur", ());
        }
        let _ = window.hide();
    }
}

pub fn request_quit(app: &tauri::AppHandle, code: i32) {
    let service = app.state::<DesktopService>();
    if service.quitting.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Lock invalidates pending work before waiting for retired SDK state.
        // Bound shutdown even if a compositor/clipboard backend stops responding.
        let _ = tokio::time::timeout(Duration::from_secs(5), super::lock_vault(app.clone())).await;
        let clipboard_app = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            clipboard_app
                .state::<super::ClipboardService>()
                .clear_and_wait(Duration::from_secs(2))
        })
        .await;
        app.state::<DesktopService>()
            .exit_ready
            .store(true, Ordering::Release);
        app.exit(code);
    });
}

pub fn run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    match event {
        tauri::RunEvent::ExitRequested { code, api, .. }
            if !app
                .state::<DesktopService>()
                .exit_ready
                .load(Ordering::Acquire) =>
        {
            api.prevent_exit();
            request_quit(app, code.unwrap_or(0));
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => {
            let _ = super::show_main_window(app.clone());
        }
        _ => {}
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "tray_tests.rs"]
mod tests;
