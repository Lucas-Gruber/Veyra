# Flatpak distribution gate

The application ID is `io.github.LucasGruber.Veyra`. Flatpak is the intended primary release format. This milestone builds the native Tauri binary; **no installable Flatpak or release pipeline is claimed yet**.

Before adding a release workflow, validate a complete offline `flatpak-builder` build:

1. Select a maintained GNOME SDK/runtime with WebKitGTK 4.1 and pin the runtime branch and Rust/Node SDK extensions.
2. Vendor the exact pnpm and Cargo lockfile dependency graphs as checksummed Flatpak sources. Build the static Nuxt assets and native binary with network disabled. A manifest that runs `pnpm install` or `cargo build` against the Internet in its final sandbox is insufficient.
3. Install the binary, scalable application icon, desktop file and AppStream metadata with the same application ID. Verify `flatpak run` without developer tools or workspace mounts.
4. Start with Wayland and fallback X11 sockets plus the minimum rendering permissions. Add network only when real Vaultwarden connectivity exists. Do not grant host filesystem, all devices or unrestricted D-Bus access.
5. Verify opening HTTPS URLs through the portal and test clipboard ownership on GNOME/KDE Wayland. Global shortcut registration requires a portal or explicit compositor configuration; the current shortcut is in-app only.
6. Design authenticated browser Native Messaging pairing and Flatpak host integration separately. Do not use an unauthenticated local server or grant unrestricted host execution.
7. Build and smoke-test the bundle in CI, validate matching version metadata, and only then add semantic-tag releases, checksums and least-privilege release permissions.

Reference: https://docs.flatpak.org/en/latest/first-build.html and https://v2.tauri.app/distribute/flatpak/ . Packaging remains an explicit later milestone rather than a nonfunctional manifest.

Build output goes to `artifacts/`, separate from Nuxt’s generated `dist` symlink. Prerelease versions are packaged on the `beta` app branch; stable versions use `stable`.
