# Tray and application lifetime

Veyra creates its tray icon and menu in Rust on Linux, Windows and macOS. The
menu opens the main window or Quick Access, locks the vault, and quits Veyra.
Opening the main window restores it from a minimized state and also works while
locked or when the vault has no entries. Windows and Linux tray activation opens
the main window; the context menu exposes all actions. macOS uses a monochrome
template icon that follows the menu bar appearance, opens the menu on click, and
restores the main window when the Dock requests reopening.

The main window's close button hides it. This intentionally preserves the
in-memory vault session and remains true when no tray host is available. Closing
Quick Access hides its existing window, including through the window manager's
close action. Neither action terminates Veyra or signs out. **Lock vault** uses
the normal Rust session invalidation, cancels pending vault work, removes visible
metadata and requests conditional clipboard clearing. **Quit Veyra** performs
that lock before exit and waits for a bounded best-effort clipboard-clear
acknowledgment. Once quitting starts, new sign-ins, demo unlocks, credential
reveals, clipboard commits and browser-fill deliveries are rejected; pending
operations also lose their session generation during locking. Clipboard history managers, forced process termination and
operating-system shutdown can prevent clearing outside Veyra's control.

Settings → Security reports tray initialization and, on Linux, current desktop
host availability. The launcher and configured Quick Access shortcut remain
ways to reopen a hidden window. The browser demonstration has no native tray.
No vault item, account name, server URL or credential is placed in tray labels,
tooltips or desktop status responses. Tray creation and lifetime control do not
expose generic frontend tray or process IPC.

## Linux runtime and packaging

The published Tauri `tray-icon` API is enabled. For Linux, Veyra selects the
published `tray-icon` 0.25.1 KSNI backend, which communicates with the desktop's
StatusNotifier host in Rust. Unlike the default AppIndicator backend, it does
not dynamically load `libappindicator3` or `libayatana-appindicator3` at runtime.
No extra AppIndicator shared libraries are required by the Debian or GNOME
Flatpak runtime. See the [upstream backend documentation](https://docs.rs/tray-icon/0.25.1/tray_icon/).

The desktop still needs a StatusNotifier tray host. KDE Plasma normally provides
one through its system tray; GNOME needs compatible panel/AppIndicator extension
support. Veyra cannot force a host to display an icon or override the user's panel
settings. The Linux status query reads `IsStatusNotifierHostRegistered` on
`org.kde.StatusNotifierWatcher` with a two-second timeout. A created icon and a
registered host do not prove that a particular panel currently displays it.

Flatpak grants `--talk-name=org.kde.StatusNotifierWatcher` and
`--own-name=org.kde.StatusNotifierItem-*` for this integration, in addition to
Veyra's existing launcher permissions. KSNI uses a process-specific name in this
standard tray-item namespace. These permissions do not
grant access to the whole session bus. The KSNI backend exports its icon pixels
and menu over D-Bus, so no host-visible temporary icon file is required.

## Validation and remaining limits

Host detection and real KSNI item registration/icon export are tested on an
isolated D-Bus with a synthetic StatusNotifier watcher and no display variables.
The clipboard worker's shutdown acknowledgment can be tested without a
display. Linux Rust compilation and tests check the native code and scoped IPC.
CI runs the isolated tray probe with
`env -u DISPLAY -u WAYLAND_DISPLAY dbus-run-session -- cargo test --locked --manifest-path src-tauri/Cargo.toml tray_host_status -- --ignored`.
These checks do not establish actual icon placement, menu delivery, compositor
focus behavior, sandbox integration, or Windows/macOS compatibility. Those still
need desktop smoke tests on each target platform. The macOS template source is
`src-tauri/icons/tray-template.svg`; regenerate its PNG with
`rsvg-convert src-tauri/icons/tray-template.svg -o src-tauri/icons/tray-template.png`.

Tauri's [tray guide](https://v2.tauri.app/learn/system-tray/) describes the native
API and platform event conventions. Veyra's Linux KSNI backend also forwards
StatusNotifier activation through the tray event API; the generic Tauri guide's
Linux click limitation applies to its default AppIndicator backend.
