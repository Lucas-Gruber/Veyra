# Flatpak distribution gate

The application ID is `io.github.LucasGruber.Veyra`. Flatpak is the primary release format configured by `.github/workflows/ci.yml` and `release.yml`. The pipeline builds the static Nuxt UI and native Tauri binary, packages that tested binary offline, installs the bundle and exercises the real UI before permitting publication. A tag or release draft alone is not proof that these gates passed.

The packaging manifest uses the signed GNOME 51 Platform and SDK, including GTK 3 and WebKitGTK 4.1. To reproduce the gate on an x86_64 Linux host that supports the Flatpak sandbox:

```sh
pnpm install --frozen-lockfile
pnpm desktop:build
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user --noninteractive flathub org.gnome.Platform//51 org.gnome.Sdk//51
bash scripts/build-flatpak.sh src-tauri/target/release/veyra
```

Install the pinned Node, pnpm and Rust toolchains and the native build prerequisites described in `docs/development.md` first. Packaging additionally requires `flatpak`, `flatpak-builder`, `appstreamcli`, `desktop-file-validate`, Python 3, `dbus-run-session`, Xvfb, `xdotool`, `xclip` and `timeout`. ImageMagick supplies optional failure screenshots. The script refuses to replace an existing user installation or overwrite an existing bundle; use a clean packaging account.

The script stages local sources with SHA-256 checksums, validates the binary architecture and matching version metadata, and invokes `flatpak-builder` with downloads disabled and sandboxing enforced. `--disable-rofiles-fuse` selects a copying cache for hosts without FUSE; it does not disable the sandbox. This packages an already-built binary; it does not claim that compilation of the Nuxt/Rust dependencies took place offline. Tagged releases also prepare a source archive containing the repository and vendored Rust dependencies; JavaScript dependencies are specified by the committed pnpm lockfile.

The installed-bundle test runs under a private D-Bus/Xvfb session. It checks a separate native Quick Access window, synthetic-vault search, a real native clipboard copy and explicit-lock clearing before the clipboard timer. Real Vaultwarden authentication is verified separately by the compatibility workflow with disposable accounts. Failure prevents publication. The current cloud container cannot run the Flatpak sandbox because user-namespace uid-map writes are blocked; metadata or native-host tests do not substitute for the installed CI gate. The unresolved Cargo audit documented in `docs/security.md` also blocks publication.

Runtime permissions are limited to network, Wayland/fallback X11, shared IPC and rendering devices. There is no host-filesystem or unrestricted D-Bus grant. Real GNOME/KDE Wayland clipboard ownership, HTTPS portal opening and compositor focus remain separate manual checks. The shortcut is currently in-app; global registration and browser Native Messaging pairing require future authenticated integration.

Build output goes to `artifacts/`, separate from Nuxt’s generated `dist` symlink. Prerelease versions are packaged on the `beta` app branch; stable versions use `stable`.

References: https://docs.flatpak.org/en/latest/first-build.html and https://v2.tauri.app/distribute/flatpak/ .
