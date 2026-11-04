# Veyra

**Your vault. One shortcut away.**

A Linux-first, keyboard-first Vaultwarden client built with Tauri 2, Rust, Nuxt 4 and Vue 3.

## Connected integration preview

Veyra can sign in to a compatible Vaultwarden server, decrypt supported personal and organization vault items using official Bitwarden Rust SDK release `rust-v4.0.0`, synchronize them and copy login fields or generated TOTP codes. The SDK is pinned to the release's exact commit. Veyra includes a main vault window, a dedicated Quick Access window, local search, an organization/collection switcher, light/dark/system appearance, adjustable text size and Modern/Glass surfaces. The current checkout supports encrypted create/edit/favorite operations and reversible Trash/Restore for logins, secure notes, cards and identities.

This is an early integration preview, **not a production-security claim**. The current checkout adds nested folders, context actions, Shift-click selection, folder/collection assignment, duplication and verified vault moves. Shortcuts are configurable; Quick Access uses native X11 registration or the Wayland GlobalShortcuts portal and also accepts `veyra --quick-access`. Actual compositor activation/focus still needs manual verification. Optional Linux browser filling requires the development [browser companion](docs/browser-companion.md); it fills an exact saved HTTPS page after an explicit action and never submits the form. Offline unlock/cache, automatic inactivity locking, attachment editing and permanent deletion remain deferred. Locking signs out, clears both windows and requires another sign-in. Clipboard clearing is best effort after 30 seconds, only when unchanged; external history managers may retain values.

Quick Access remembers its search and selected entry for 30 seconds after closing,
in memory only; locking or changing sessions clears both. Its visible **Open
Veyra** action (`Ctrl+E` by default) restores the main client, including while
locked. Closing the main window keeps Veyra running. A native tray menu provides
open, Quick Access, lock and quit actions on Linux, Windows and macOS; current
runtime verification is limited to isolated Linux D-Bus tests. Settings → Security
reports tray availability. See [tray behavior and platform limits](docs/tray.md).

```sh
pnpm install --frozen-lockfile
pnpm tauri dev            # Native application; Linux prerequisites required
pnpm dev                  # Browser UI preview with clearly labeled sample data only
```

Connect using your HTTPS server address in the desktop application. Passwords are transient form inputs passed to Rust; keys, tokens and cryptographic operations stay in Rust. Explicit reveals, contents and editor fields exist briefly in component memory; they never enter frontend stores or browser storage. The browser preview cannot connect to a real vault. The optional synthetic demo never substitutes for a failed real connection.

Submitting sign-in remembers the server URL and account email locally through
Rust, including after an incorrect password. Both windows prefill them on the
next sign-in. **Forget saved sign-in details** removes them; passwords and 2FA
codes are never saved. This does not provide offline unlock.

## Program information and versions

Settings → About shows the actual native version, release channel, platform, architecture and license. Update checks contact the public GitHub release API when due at startup and daily while open; they can be disabled or triggered manually. Stable installations ignore prereleases. Browser preview performs no update requests. Checks do not install updates.

Use `pnpm release:version prerelease --dry-run`, `pnpm release:version prerelease` and `pnpm version:check` to keep npm, Cargo, Tauri and AppStream consistent; see [versioning](docs/versioning.md). The published preview is **[0.1.0-alpha.3](https://github.com/Lucas-Gruber/Veyra/releases/tag/v0.1.0-alpha.3)**, with a Linux x86_64 Flatpak, dependency source archive and checksums. The working version is **0.1.1**, an unreleased local candidate. A tag starts validation; publication remains gated.

## Compatibility and validation

Reference server: Vaultwarden **1.37.4**. Real tests cover PBKDF2, Argon2id, account/per-item keys, sync, integrity failures, wrong passwords, authenticator 2FA and locking. Daily compatibility CI also resolves the latest stable upstream version to an immutable image. This does not guarantee compatibility with arbitrary future versions. See [compatibility](docs/compatibility.md) and [test results](docs/validation.md).

The Alpha3 Linux build, 70 tests and installed Flatpak check passed on GitHub. Its release uses an explicitly documented owner exception for **RUSTSEC-2023-0071** in the SDK dependency graph; the RSA library remains unfixed. CI retains the complete audit and accepts that finding only for the reviewed Alpha3 build inputs. Different source/dependency versions, changed findings and audit errors still block validation. **The changed organization/write sources in this checkout are outside that exception and are not authorized for publication.** See the [security policy](docs/security.md). Flatpak sandbox execution is unavailable in this cloud container; installed-package verification runs on GitHub.

[Development](docs/development.md) · [issue implementation status](docs/issues.md) · [entry management](docs/entry-management.md) · [shortcuts](docs/shortcuts.md) · [architecture](docs/architecture.md) · [security](docs/security.md) · [desktop limitations](docs/desktop.md) · [integration tests](tests/integration/README.md) · [Flatpak](packaging/flatpak/README.md)

GPL-3.0-only. Veyra selects the GPL option of the pinned official Bitwarden components; no proprietary-only SDK modules are used. See [third-party notices](THIRD_PARTY_NOTICES.md). This project is independent of Bitwarden and Vaultwarden.
