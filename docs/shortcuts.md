# Quick Access from the desktop

An installed native Veyra binary accepts:

```sh
veyra --quick-access
veyra --help
veyra --version
# Installed Flatpak:
flatpak run io.github.LucasGruber.Veyra --quick-access
```

On Linux, a launch contacts the running Veyra session over the user's session
D-Bus. Quick Access shares that process's existing vault session. A cold
`--quick-access` launch builds the main window hidden and shows only Quick Access.
A launch without arguments shows the main window. Help and version work without
GTK, a display, or a D-Bus session; unsupported or combined arguments fail with
exit code 2. Arguments are not included in errors.

The launch service claims `io.github.LucasGruber.Veyra.Launcher` before GTK
initializes, preserving `enableGTKAppId` and dock icon association. The service
accepts only the two window targets, bounds pending requests, and never accepts
URLs, file paths, credentials or vault operations. The Flatpak manifest grants
only this exact D-Bus name. The service is released when the process exits.
Linux startup requires a working desktop session bus; it fails explicitly when
that bus is unavailable. Single-instance forwarding outside Linux is deferred.

Settings → Quick Access changes the Quick Access accelerator and its in-app
counterpart. The main window asks Rust to register the saved accelerator at
startup. The default is `Ctrl+Alt+Space`. Only the main local window can invoke
the two scoped shortcut commands; generic global-shortcut plugin IPC is not
granted. Shortcut preferences contain no vault data.

- In a Wayland session, Rust uses the XDG GlobalShortcuts portal. This includes
  sessions where Veyra renders through XWayland for graphics compatibility. The
  compositor can show a permission dialog, reject the request, or assign another
  trigger; Settings reports the actual compositor trigger. Portal assignment
  changes and session closure update the backend status.
- In an X11 session, Rust uses Tauri's native global shortcut backend. A conflict
  or rejected registration is reported. A failed replacement keeps the previous
  registration active.
- If no supported backend is available, assign `veyra --quick-access` (or the
  Flatpak command above) to a shortcut in the desktop/compositor settings.

Registration does not guarantee that a compositor allows raising or focusing
the Quick Access window. Veyra does not modify compositor configuration or
intercept all keyboard input. Runtime initialization installs the backend;
registration occurs when the main frontend requests its saved preference.
Neither a settings change nor a version bump authorizes publication.

Quick Access always shows **Open Veyra** and its configurable shortcut (default
`Ctrl+E`) in the footer. It opens the selected entry in the main client, or simply
restores the main window when locked or when no result is available. The command
also restores a minimized main window. The same binding is listed in Settings →
Quick Access and works from the sign-in form.

The implementation uses pinned `tauri-plugin-global-shortcut 2.4.1`,
`ashpd 0.13.13`, and `zbus 5.19.0`. Portal trigger strings follow the official
[XDG shortcut specification](https://specifications.freedesktop.org/shortcuts/latest/)
and the [GlobalShortcuts portal contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html).

Parser, native-key conversion, portal-key conversion and rejected-rebind state
have unit coverage. The real launch service can be tested without a GUI or
access to the real desktop bus:

```sh
dbus-run-session -- cargo test --locked --manifest-path src-tauri/Cargo.toml \
  forwards_launches_before_any_gtk_runtime_and_releases_the_name -- --ignored
```

Actual KDE/GNOME portal permission dialogs, global key activation, native focus,
multi-monitor behavior and installed Flatpak forwarding require their own manual
acceptance checks. Unit tests and an isolated D-Bus test do not establish
compositor compatibility.
