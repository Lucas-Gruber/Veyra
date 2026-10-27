# Desktop behavior and limits

The native main and Quick Access WebViews share one Rust session and metadata snapshot. Tauri capabilities allow only scoped local commands and session event listeners; there is no shell/filesystem/generic clipboard/opener IPC exposed. Website opening is a native operation restricted to a known item's HTTPS URL and an active session.

`Ctrl Alt Space` works while Veyra has focus. The floating window uses supported Tauri show/focus/position calls; a compositor may reject focus or placement. There is no unsafe global interception. System-wide shortcuts need the XDG GlobalShortcuts portal and/or explicit compositor setup, not a claim inferred from an in-app shortcut. See https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html .

The clipboard worker keeps its owner alive, clears after 30 seconds only if unchanged, and also requests conditional clearing on lock. Read/clear is not atomic. Wayland data-control and XWayland availability differ by compositor; lack of access is an honest error. Clipboard managers can retain history beyond Veyra's control.

Locking cancels pending work and signs out. Both windows discard metadata and show sign-in. Explicit reveals expire after ten seconds and guard against late responses. Server token expiry locks the shared session. No offline, biometric or cached master-key unlock is implemented.

The browser UI is a sample-data preview only. Its clipboard cannot be cleared reliably. Connected operations require Rust, and a real connection failure never switches to sample data.

The native application was exercised under a private cloud X11 display, including real server login/copy/sync/Quick Access/lock. This does not establish GNOME/KDE/Wayland, multiple-monitor, accessibility or Flatpak portal compatibility. No tray, startup integration, Secret Service, system-wide shortcut or browser Native Messaging feature is claimed yet.
