//! Backend-owned registration for the single system-wide Quick Access action.
//! The frontend can choose an accelerator, but cannot register arbitrary handlers.
use crate::session::error;
use crate::vaultwarden::VaultError;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::Mutex as AsyncMutex;

const DEFAULT: &str = "Ctrl+Alt+Space";
const COMMAND: &str = "veyra --quick-access";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Backend {
    Portal,
    Native,
    Manual,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShortcutStatus {
    backend: Backend,
    state: &'static str,
    shortcut: String,
    trigger: Option<String>,
    message: String,
    command: &'static str,
}

impl ShortcutStatus {
    fn initial(backend: Backend) -> Self {
        Self {
            backend,
            state: "unavailable",
            shortcut: DEFAULT.into(),
            trigger: None,
            message: "Use the desktop shortcut settings to bind veyra --quick-access.".into(),
            command: COMMAND,
        }
    }
}

#[derive(Default)]
struct Registration {
    shortcut: Option<String>,
    #[cfg(target_os = "linux")]
    portal: Option<portal::Binding>,
}

pub(crate) struct ShortcutService {
    status: Mutex<ShortcutStatus>,
    registration: AsyncMutex<Registration>,
}

fn desktop_backend(wayland: bool, x11: bool) -> Backend {
    if cfg!(target_os = "linux") {
        if wayland {
            Backend::Portal
        } else if x11 {
            Backend::Native
        } else {
            Backend::Manual
        }
    } else {
        Backend::Native
    }
}

impl Default for ShortcutService {
    fn default() -> Self {
        // An XWayland rendering workaround does not make X11 key grabs work
        // globally in a Wayland session. Ask its compositor via the portal.
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v == "wayland");
        let x11 = std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty());
        Self {
            status: Mutex::new(ShortcutStatus::initial(desktop_backend(wayland, x11))),
            registration: AsyncMutex::new(Registration::default()),
        }
    }
}

impl ShortcutService {
    fn status(&self) -> Result<ShortcutStatus, VaultError> {
        self.status
            .lock()
            .map(|v| v.clone())
            .map_err(|_| unavailable())
    }

    fn update(&self, update: impl FnOnce(&mut ShortcutStatus)) -> Result<(), VaultError> {
        let mut status = self.status.lock().map_err(|_| unavailable())?;
        update(&mut status);
        Ok(())
    }

    fn failed(
        &self,
        requested: &str,
        state: &'static str,
        message: &str,
        keep_previous: bool,
    ) -> Result<ShortcutStatus, VaultError> {
        self.update(|status| {
            // A failed rebind must not advertise the rejected accelerator as
            // registered, nor discard a still-working previous registration.
            if !keep_previous {
                status.state = state;
                status.shortcut = requested.into();
                status.trigger = None;
            } else {
                status.state = "registered";
            }
            status.message = message.into();
        })?;
        self.status()
    }
}

fn unavailable() -> VaultError {
    error(
        "shortcut_unavailable",
        "The desktop shortcut service is unavailable.",
    )
}

pub(crate) fn initialize(app: &AppHandle) {
    let service = app.state::<ShortcutService>();
    if service
        .status()
        .is_ok_and(|status| status.backend == Backend::Native)
    {
        // Install the plugin only on a supported backend. In particular, a
        // Wayland-only process must never initialize its X11 global-hotkey path.
        if app
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            .is_err()
        {
            let _ = service.update(|status| {
                status.backend = Backend::Manual;
                status.message = "The native shortcut backend is unavailable. Bind veyra --quick-access in your desktop settings.".into();
            });
        }
    }
}

#[tauri::command]
pub(crate) async fn quick_access_shortcut_status(
    service: State<'_, ShortcutService>,
) -> Result<ShortcutStatus, VaultError> {
    service.status()
}

#[tauri::command]
pub(crate) async fn configure_quick_access_shortcut(
    app: AppHandle,
    shortcut: String,
) -> Result<ShortcutStatus, VaultError> {
    let parsed = ParsedShortcut::parse(&shortcut)?;
    let service = app.state::<ShortcutService>();
    let mut registration = service.registration.lock().await;
    let previous = service.status()?;
    if registration.shortcut.as_ref() == Some(&parsed.canonical) && previous.state == "registered" {
        return Ok(previous);
    }
    let keep_previous = previous.state == "registered";
    service.update(|status| {
        if !keep_previous {
            status.state = "pending";
            status.shortcut = parsed.canonical.clone();
        }
        status.message =
            "Waiting for the desktop to register Quick Access. A permission dialog may appear."
                .into();
    })?;
    match previous.backend {
        Backend::Native => {
            let next = parsed.native()?;
            let handle = app.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                handle.global_shortcut().on_shortcut(next, |app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        crate::quick_launch::show(app, crate::quick_launch::Launch::QuickAccess);
                    }
                })
            })
            .await
            .map_err(|_| unavailable())?;
            if result.is_err() {
                return service.failed(&parsed.canonical, "denied", "The desktop rejected this shortcut; it may already be in use. Any previous shortcut remains active. You can also bind veyra --quick-access in desktop settings.", keep_previous);
            }
            if let Some(old) = registration.shortcut.as_deref() {
                if let Ok(old) = ParsedShortcut::parse(old).and_then(|s| s.native()) {
                    let handle = app.clone();
                    let removed = tauri::async_runtime::spawn_blocking(move || {
                        handle.global_shortcut().unregister(old)
                    })
                    .await
                    .map_err(|_| unavailable())?;
                    if removed.is_err() {
                        let handle = app.clone();
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            handle.global_shortcut().unregister(next)
                        })
                        .await;
                        return service.failed(&parsed.canonical, "unavailable", "The previous shortcut could not be released. The change was not applied.", keep_previous);
                    }
                }
            }
            service.update(|status| {
                status.state = "registered";
                status.shortcut = parsed.canonical.clone();
                status.trigger = Some(parsed.canonical.clone());
                status.message = "Quick Access is registered with the desktop. Window focus remains subject to compositor policy.".into();
            })?;
        }
        #[cfg(target_os = "linux")]
        Backend::Portal => {
            let binding = match portal::bind(&parsed).await {
                Ok(binding) => binding,
                Err(failure) => {
                    return service.failed(
                        &parsed.canonical,
                        failure.state,
                        failure.message,
                        keep_previous,
                    )
                }
            };
            if let Some(old) = registration.portal.take() {
                old.close().await;
            }
            service.update(|status| {
                status.state = "registered";
                status.shortcut = parsed.canonical.clone();
                status.trigger = Some(binding.trigger.clone());
                status.message = "The compositor registered Quick Access. Its assigned trigger is shown; it may differ from the requested shortcut.".into();
            })?;
            binding.listen(&app);
            registration.portal = Some(binding);
        }
        _ => {
            return service.failed(&parsed.canonical, "unavailable", "This session has no supported global shortcut backend. Bind veyra --quick-access in your desktop or compositor settings.", false);
        }
    }
    registration.shortcut = Some(parsed.canonical);
    service.status()
}

struct ParsedShortcut {
    canonical: String,
    modifiers: Vec<&'static str>,
    key: String,
}

impl ParsedShortcut {
    fn parse(value: &str) -> Result<Self, VaultError> {
        let invalid = || {
            error(
                "invalid_shortcut",
                "Choose a valid shortcut using Ctrl, Alt, Shift or Meta and one supported key.",
            )
        };
        if value.len() > 80 {
            return Err(invalid());
        }
        let mut parts: Vec<_> = value.split('+').collect();
        let key = parts.pop().ok_or_else(invalid)?;
        let is_function = key
            .strip_prefix('F')
            .and_then(|v| v.parse::<u8>().ok())
            .is_some_and(|v| (1..=24).contains(&v) && format!("F{v}") == key);
        let is_character = key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric()
            || key.len() == 1 && ",.;/[]\\'`=-".contains(key);
        if !is_function
            && !is_character
            && ![
                "Space",
                "Enter",
                "Escape",
                "Tab",
                "ArrowDown",
                "ArrowUp",
                "ArrowLeft",
                "ArrowRight",
                "Home",
                "End",
                "Delete",
                "Backspace",
                "Insert",
                "PageUp",
                "PageDown",
            ]
            .contains(&key)
        {
            return Err(invalid());
        }
        let order = ["Ctrl", "Alt", "Shift", "Meta"];
        if parts.iter().any(|part| !order.contains(part))
            || parts
                .iter()
                .enumerate()
                .any(|(i, part)| parts[..i].contains(part))
        {
            return Err(invalid());
        }
        let modifiers: Vec<_> = order
            .into_iter()
            .filter(|modifier| parts.contains(modifier))
            .collect();
        let key = if key.len() == 1 {
            key.to_ascii_uppercase()
        } else {
            key.into()
        };
        let canonical = modifiers
            .iter()
            .copied()
            .chain(std::iter::once(key.as_str()))
            .collect::<Vec<_>>()
            .join("+");
        Ok(Self {
            canonical,
            modifiers,
            key,
        })
    }

    fn native(&self) -> Result<Shortcut, VaultError> {
        use tauri_plugin_global_shortcut::{Code, Modifiers};
        let code = match self.key.as_str() {
            "," => Code::Comma,
            "." => Code::Period,
            ";" => Code::Semicolon,
            "/" => Code::Slash,
            "[" => Code::BracketLeft,
            "]" => Code::BracketRight,
            "\\" => Code::Backslash,
            "'" => Code::Quote,
            "`" => Code::Backquote,
            "=" => Code::Equal,
            "-" => Code::Minus,
            "Backspace" => Code::Backspace,
            key if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() => {
                format!("Key{key}").parse().map_err(|_| unavailable())?
            }
            key if key.len() == 1 && key.as_bytes()[0].is_ascii_digit() => {
                format!("Digit{key}").parse().map_err(|_| unavailable())?
            }
            key => key.parse().map_err(|_| {
                error(
                    "invalid_shortcut",
                    "This key is not supported by the native shortcut backend.",
                )
            })?,
        };
        let mut modifiers = Modifiers::empty();
        for modifier in &self.modifiers {
            modifiers |= match *modifier {
                "Ctrl" => Modifiers::CONTROL,
                "Alt" => Modifiers::ALT,
                "Shift" => Modifiers::SHIFT,
                "Meta" => Modifiers::SUPER,
                _ => unreachable!(),
            };
        }
        Ok(Shortcut::new(Some(modifiers), code))
    }

    #[cfg(target_os = "linux")]
    fn portal_trigger(&self) -> String {
        let key = match self.key.as_str() {
            "Space" => "space",
            "Enter" => "Return",
            "ArrowDown" => "Down",
            "ArrowUp" => "Up",
            "ArrowLeft" => "Left",
            "ArrowRight" => "Right",
            "Backspace" => "BackSpace",
            "PageUp" => "Page_Up",
            "PageDown" => "Page_Down",
            "," => "comma",
            "." => "period",
            ";" => "semicolon",
            "/" => "slash",
            "[" => "bracketleft",
            "]" => "bracketright",
            "\\" => "backslash",
            "'" => "apostrophe",
            "`" => "grave",
            "=" => "equal",
            "-" => "minus",
            key => key,
        };
        let key = if key.len() == 1 {
            key.to_ascii_lowercase()
        } else {
            key.into()
        };
        self.modifiers
            .iter()
            .map(|modifier| match *modifier {
                "Ctrl" => "CTRL",
                "Alt" => "ALT",
                "Shift" => "SHIFT",
                "Meta" => "LOGO",
                _ => unreachable!(),
            })
            .chain(std::iter::once(key.as_str()))
            .collect::<Vec<_>>()
            .join("+")
    }
}

#[cfg(target_os = "linux")]
mod portal {
    use super::*;
    use ashpd::desktop::{
        global_shortcuts::{GlobalShortcuts, NewShortcut},
        Session,
    };
    use futures_util::StreamExt;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    const ACTION: &str = "veyra-quick-access";

    pub(super) struct Failure {
        pub state: &'static str,
        pub message: &'static str,
    }

    impl From<ashpd::Error> for Failure {
        fn from(error: ashpd::Error) -> Self {
            let denied = matches!(
                error,
                ashpd::Error::Response(_)
                    | ashpd::Error::Portal(ashpd::PortalError::NotAllowed(_))
                    | ashpd::Error::Portal(ashpd::PortalError::Cancelled(_))
            );
            Self {
                state: if denied { "denied" } else { "unavailable" },
                message: if denied {
                    "The shortcut portal request was declined or cancelled. Any previous shortcut remains active. Bind veyra --quick-access manually if preferred."
                } else {
                    "The compositor's GlobalShortcuts portal is unavailable. Any previous shortcut remains active. Bind veyra --quick-access in desktop settings."
                },
            }
        }
    }

    pub(super) struct Binding {
        portal: Arc<GlobalShortcuts>,
        session: Arc<Session<GlobalShortcuts>>,
        cancel: CancellationToken,
        pub trigger: String,
    }

    pub(super) async fn bind(requested: &ParsedShortcut) -> Result<Binding, Failure> {
        let portal = Arc::new(GlobalShortcuts::new().await?);
        let session = Arc::new(portal.create_session(Default::default()).await?);
        let trigger = requested.portal_trigger();
        let shortcut =
            NewShortcut::new(ACTION, "Open Veyra Quick Access").preferred_trigger(trigger.as_str());
        let result = async {
            let response = portal.bind_shortcuts(&session, &[shortcut], None, Default::default()).await?.response()?;
            response.shortcuts().iter().find(|s| s.id() == ACTION)
                .filter(|s| !s.trigger_description().is_empty())
                .map(|s| s.trigger_description().to_string())
                .ok_or(Failure { state: "denied", message: "The compositor did not assign a shortcut. Any previous shortcut remains active. Bind veyra --quick-access manually if preferred." })
        }.await;
        let trigger = match result {
            Ok(trigger) => trigger,
            Err(failure) => {
                let _ = session.close().await;
                return Err(failure);
            }
        };
        Ok(Binding {
            portal,
            session,
            cancel: CancellationToken::new(),
            trigger,
        })
    }

    impl Binding {
        pub async fn close(self) {
            self.cancel.cancel();
            let _ = self.session.close().await;
        }

        pub fn listen(&self, app: &AppHandle) {
            let app = app.clone();
            let portal = self.portal.clone();
            let session = self.session.clone();
            let cancel = self.cancel.clone();
            tauri::async_runtime::spawn(async move {
                // ashpd intentionally hides Session::path; its public Serialize
                // contract exposes the D-Bus object path for signal correlation.
                let path = serde_json::to_value(&session)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned));
                let Some(path) = path else {
                    ended(&app, &cancel);
                    return;
                };
                let streams = tokio::try_join!(
                    portal.receive_activated(),
                    portal.receive_shortcuts_changed(),
                    session.receive_closed()
                );
                let Ok((activated, changed, closed)) = streams else {
                    ended(&app, &cancel);
                    return;
                };
                tokio::pin!(activated, changed, closed);
                loop {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return,
                        event = activated.next() => {
                            let Some(event) = event else { break; };
                            if !cancel.is_cancelled() && event.session_handle().as_str() == path && event.shortcut_id() == ACTION {
                                crate::quick_launch::show(&app, crate::quick_launch::Launch::QuickAccess);
                            }
                        }
                        event = changed.next() => {
                            let Some(event) = event else { break; };
                            if event.session_handle().as_str() == path {
                                let shortcut = event.shortcuts().iter().find(|s| s.id() == ACTION);
                                let _ = app.state::<ShortcutService>().update(|status| {
                                    if cancel.is_cancelled() { return; }
                                    status.trigger = shortcut.map(|s| s.trigger_description().to_owned());
                                    status.state = if shortcut.is_some() { "registered" } else { "denied" };
                                    status.message = "The compositor changed the Quick Access shortcut assignment.".into();
                                });
                            }
                        }
                        _ = closed.next() => break,
                    }
                }
                ended(&app, &cancel);
            });
        }
    }

    fn ended(app: &AppHandle, cancel: &CancellationToken) {
        let _ = app.state::<ShortcutService>().update(|status| {
                    if cancel.is_cancelled() { return; }
                    status.state = "unavailable";
                    status.trigger = None;
                    status.message = "The shortcut portal session ended. Configure Quick Access again or bind veyra --quick-access manually.".into();
        });
    }

    impl Drop for Binding {
        fn drop(&mut self) {
            self.cancel.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_parser_matches_frontend_canonical_order() {
        for (raw, canonical) in [
            ("Alt+Ctrl+Space", "Ctrl+Alt+Space"),
            ("Meta+Shift+a", "Shift+Meta+A"),
            ("Enter", "Enter"),
            ("Ctrl+,", "Ctrl+,"),
        ] {
            let shortcut = ParsedShortcut::parse(raw).unwrap();
            assert_eq!(shortcut.canonical, canonical);
            assert!(shortcut.native().is_ok());
        }
        for raw in [
            "Ctrl+Ctrl+A",
            "Super+A",
            "Ctrl+",
            "A+B",
            "F25",
            "F01",
            "https://example.org",
        ] {
            assert!(ParsedShortcut::parse(raw).is_err(), "accepted {raw}");
        }
    }

    #[test]
    fn all_frontend_key_variants_have_native_codes() {
        for key in [
            "Space",
            "Enter",
            "Escape",
            "Tab",
            "ArrowDown",
            "ArrowUp",
            "ArrowLeft",
            "ArrowRight",
            "Home",
            "End",
            "Delete",
            "Backspace",
            "Insert",
            "PageUp",
            "PageDown",
            "F1",
            "F24",
            "A",
            "0",
            ",",
            ".",
            ";",
            "/",
            "[",
            "]",
            "\\",
            "'",
            "`",
            "=",
            "-",
        ] {
            assert!(
                ParsedShortcut::parse(&format!("Ctrl+{key}"))
                    .unwrap()
                    .native()
                    .is_ok(),
                "missing {key}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn portal_uses_xdg_keysym_specification_not_native_accelerator_syntax() {
        for (raw, expected) in [
            ("Ctrl+Alt+Space", "CTRL+ALT+space"),
            ("Ctrl+Shift+A", "CTRL+SHIFT+a"),
            ("Meta+Enter", "LOGO+Return"),
            ("Ctrl+PageDown", "CTRL+Page_Down"),
            ("Ctrl+,", "CTRL+comma"),
        ] {
            assert_eq!(
                ParsedShortcut::parse(raw).unwrap().portal_trigger(),
                expected
            );
        }
        assert_eq!(desktop_backend(true, true), Backend::Portal);
        assert_eq!(desktop_backend(false, true), Backend::Native);
        assert_eq!(desktop_backend(false, false), Backend::Manual);
    }

    #[test]
    fn failed_rebind_keeps_the_previous_registered_accelerator() {
        let service = ShortcutService::default();
        service
            .update(|status| {
                status.state = "registered";
                status.shortcut = "Ctrl+Alt+Space".into();
                status.trigger = Some("Ctrl+Alt+Space".into());
            })
            .unwrap();
        let status = service
            .failed("Ctrl+X", "denied", "Already in use", true)
            .unwrap();
        assert_eq!(status.state, "registered");
        assert_eq!(status.shortcut, "Ctrl+Alt+Space");
        assert_eq!(status.trigger.as_deref(), Some("Ctrl+Alt+Space"));
    }
}
