# Desktop behavior and limits

The GTK application ID matches the packaged desktop entry and icon ID, including
Quick Access, so Linux docks can associate both windows with Veyra. Native
packages install their launcher/icon. For an unpackaged local build, register it
once with `pnpm desktop:integrate /absolute/path/to/veyra`; rerun after moving the
binary. A debug binary still needs the Nuxt development server running. This
installs only the Veyra launcher and icon under the user's XDG data directory.
For development, run `pnpm desktop:integrate --dev` once from the configured
development shell. Its launcher starts `pnpm desktop:dev` in this checkout,
including the development server, using the resolved pnpm and Node paths without
a shell. Rerun registration after moving the checkout or changing those runtime
paths. This uses the same launcher ID as a native install, so the last explicit
registration determines what launching Veyra from the dock starts.
Paths containing `%`, `=` or control characters are rejected before writing a
launcher because desktop launchers cannot reliably execute them. Existing
symlink targets are also rejected. The launcher helper can be verified without
opening Veyra or changing the user's launcher with
`python3 -m unittest discover -s tests/desktop -v`; its launch test uses a harmless
stub and a temporary XDG data directory.

Debian bundles explicitly install the same application-ID launcher and SVG icon
as Flatpak and the local helper. Tauri's generated `Veyra.desktop` is hidden to
avoid a second launcher: its default filename does not match the GTK application
ID used by Wayland docks.

The native main and Quick Access WebViews share one Rust session and metadata snapshot. Tauri capabilities allow only scoped local commands and session event listeners; there is no shell/filesystem/generic clipboard/opener IPC exposed. Website opening is a native operation restricted to a known item's HTTPS URL and an active session.

Native Quick Access fills its undecorated window with one surface and hides on
native focus loss (including clicking another window). Search, action-menu state
and unsubmitted sign-in fields are cleared on focus loss or explicit closing.
The browser overlay closes on a backdrop
click. Compositor-specific focus delivery still requires desktop verification.

`Ctrl Alt Space` works while Veyra has focus. The floating window uses supported Tauri show/focus/position calls; a compositor may reject focus or placement. There is no unsafe global interception. System-wide shortcuts need the XDG GlobalShortcuts portal and/or explicit compositor setup, not a claim inferred from an in-app shortcut. See https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html .

The clipboard worker keeps its owner alive, clears after 30 seconds only if unchanged, and also requests conditional clearing on lock. Read/clear is not atomic. Wayland data-control and XWayland availability differ by compositor; lack of access is an honest error. Clipboard managers can retain history beyond Veyra's control.

Locking cancels pending work and signs out. Both windows discard metadata and show sign-in. Explicit reveals expire after ten seconds and guard against late responses. Server token expiry locks the shared session. No offline, biometric or cached master-key unlock is implemented.

The browser UI is a sample-data preview only. Its clipboard cannot be cleared reliably. Connected operations require Rust, and a real connection failure never switches to sample data.

The native application was exercised under a private cloud X11 display, including real server login/copy/sync/Quick Access/lock. This does not establish GNOME/KDE/Wayland, multiple-monitor, accessibility or Flatpak portal compatibility. No tray, startup integration, Secret Service, system-wide shortcut or browser Native Messaging feature is claimed yet.

## KDE Wayland with NVIDIA: startup protocol error

A startup failure ending with `Error 71 (Protocol error) dispatching to Wayland display` can match an unresolved WebKitGTK/graphics-driver interaction. This has been reported on KDE/Wayland with NVIDIA, including current WebKitGTK versions: [WebKit #280210](https://bugs.webkit.org/show_bug.cgi?id=280210), [Tauri #10702](https://github.com/tauri-apps/tauri/issues/10702). The message alone does not identify the exact protocol violation.

Veyra now applies Noryn's Linux startup defaults before GTK initializes: use
X11/XWayland when `DISPLAY` is available, and disable WebKit's DMA-BUF renderer.
Explicit `GDK_BACKEND` and `WEBKIT_DISABLE_DMABUF_RENDERER` values are preserved.
This avoids requiring an environment prefix on every normal development or
installed launch. The WebKit security sandbox stays enabled. The GTK assertion
alone does not prove the cause of the fatal protocol error; see also the upstream
[explicit-sync fix](https://bugs.webkit.org/show_bug.cgi?id=324551).

The equivalent explicit launch is:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm desktop:dev
# Installed Flatpak:
flatpak run --env=WEBKIT_DISABLE_DMABUF_RENDERER=1 io.github.LucasGruber.Veyra
```

Disabling WebKit's DMA-BUF renderer can reduce animation performance or introduce
rendering artifacts on some driver/runtime combinations. To test native Wayland
with the normal renderer, explicitly set `GDK_BACKEND=wayland` and
`WEBKIT_DISABLE_DMABUF_RENDERER=0` for that launch.

If startup still fails or the window becomes blank/transparent, test the NVIDIA-specific explicit-sync workaround separately:

```sh
GDK_BACKEND=wayland WEBKIT_DISABLE_DMABUF_RENDERER=0 __NV_DISABLE_EXPLICIT_SYNC=1 pnpm desktop:dev
# Installed Flatpak:
flatpak run --env=GDK_BACKEND=wayland --env=WEBKIT_DISABLE_DMABUF_RENDERER=0 --env=__NV_DISABLE_EXPLICIT_SYNC=1 io.github.LucasGruber.Veyra
```

Test one option at a time with other graphics overrides removed. The second option keeps the DMA-BUF rendering path, but reports also describe flicker on some configurations. Both leave WebKit's security sandbox enabled. Neither is a general application-code fix or a guarantee for every NVIDIA/Wayland setup.

The DMA-BUF option passed Veyra's main/Quick Access, search, clipboard and lock smoke on X11/Xvfb. On 2026-10-29, the affected user also confirmed that `WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm desktop:dev` starts the app on CachyOS/KDE/Wayland/NVIDIA. This confirms startup on that setup; focus, clipboard behavior and other Wayland features still require separate verification there.
