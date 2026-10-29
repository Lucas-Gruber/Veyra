# Desktop behavior and limits

The native main and Quick Access WebViews share one Rust session and metadata snapshot. Tauri capabilities allow only scoped local commands and session event listeners; there is no shell/filesystem/generic clipboard/opener IPC exposed. Website opening is a native operation restricted to a known item's HTTPS URL and an active session.

`Ctrl Alt Space` works while Veyra has focus. The floating window uses supported Tauri show/focus/position calls; a compositor may reject focus or placement. There is no unsafe global interception. System-wide shortcuts need the XDG GlobalShortcuts portal and/or explicit compositor setup, not a claim inferred from an in-app shortcut. See https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html .

The clipboard worker keeps its owner alive, clears after 30 seconds only if unchanged, and also requests conditional clearing on lock. Read/clear is not atomic. Wayland data-control and XWayland availability differ by compositor; lack of access is an honest error. Clipboard managers can retain history beyond Veyra's control.

Locking cancels pending work and signs out. Both windows discard metadata and show sign-in. Explicit reveals expire after ten seconds and guard against late responses. Server token expiry locks the shared session. No offline, biometric or cached master-key unlock is implemented.

The browser UI is a sample-data preview only. Its clipboard cannot be cleared reliably. Connected operations require Rust, and a real connection failure never switches to sample data.

The native application was exercised under a private cloud X11 display, including real server login/copy/sync/Quick Access/lock. This does not establish GNOME/KDE/Wayland, multiple-monitor, accessibility or Flatpak portal compatibility. No tray, startup integration, Secret Service, system-wide shortcut or browser Native Messaging feature is claimed yet.

## KDE Wayland with NVIDIA: startup protocol error

A startup failure ending with `Error 71 (Protocol error) dispatching to Wayland display` can match an unresolved WebKitGTK/graphics-driver interaction. This has been reported on KDE/Wayland with NVIDIA, including current WebKitGTK versions: [WebKit #280210](https://bugs.webkit.org/show_bug.cgi?id=280210), [Tauri #10702](https://github.com/tauri-apps/tauri/issues/10702). The message alone does not identify the exact protocol violation.

Try the following per-launch renderer workaround first:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm desktop:dev
# Installed Flatpak:
flatpak run --env=WEBKIT_DISABLE_DMABUF_RENDERER=1 io.github.LucasGruber.Veyra
```

This disables WebKit's DMA-BUF renderer. It can reduce animation performance or introduce rendering artifacts on some driver/runtime combinations. The setting applies only to that launch.

If startup still fails or the window becomes blank/transparent, test the NVIDIA-specific explicit-sync workaround separately:

```sh
__NV_DISABLE_EXPLICIT_SYNC=1 pnpm desktop:dev
# Installed Flatpak:
flatpak run --env=__NV_DISABLE_EXPLICIT_SYNC=1 io.github.LucasGruber.Veyra
```

Test one option at a time with other graphics overrides removed. The second option keeps the DMA-BUF rendering path, but reports also describe flicker on some configurations. Both leave WebKit's security sandbox enabled. Neither is a general application-code fix or a guarantee for every NVIDIA/Wayland setup.

The DMA-BUF option passed Veyra's main/Quick Access, search, clipboard and lock smoke on X11/Xvfb. On 2026-10-29, the affected user also confirmed that `WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm desktop:dev` starts the app on CachyOS/KDE/Wayland/NVIDIA. This confirms startup on that setup; focus, clipboard behavior and other Wayland features still require separate verification there.
