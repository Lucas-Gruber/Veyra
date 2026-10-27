# Veyra

**Your vault. One shortcut away.**

A Linux-first, keyboard-first Vaultwarden client built with Tauri 2, Rust, Nuxt 4 and Vue 3.

## Read-only integration preview

Veyra can sign in to a compatible Vaultwarden server, decrypt supported personal vault items using pinned official Bitwarden Rust components, synchronize them and copy login fields or generated TOTP codes. It includes a main vault window, a dedicated Quick Access window, local search, light/dark/system appearance and Modern/Glass surfaces.

This is an early integration preview, **not a production-security claim**. Organizations, vault editing, offline unlock/cache, browser autofill and system-wide shortcut registration are not implemented. Locking signs out, clears both windows and requires another sign-in. `Ctrl Alt Space` opens Quick Access while Veyra has focus. Clipboard clearing is best effort after 30 seconds, only when unchanged; external history managers may retain values.

```sh
pnpm install --frozen-lockfile
pnpm desktop:dev          # Native application; Linux prerequisites required
pnpm dev                  # Browser UI preview with clearly labeled sample data only
```

Connect using your HTTPS server address in the desktop application. Passwords are transient form inputs passed to Rust; keys, tokens and decrypted field operations stay in Rust, not Pinia/browser storage. The browser preview cannot connect to a real vault. The optional synthetic demo never substitutes for a failed real connection.

## Program information and versions

Settings → About shows the actual native version, release channel, platform, architecture and license. Update checks contact the public GitHub release API when due at startup and daily while open; they can be disabled or triggered manually. Stable installations ignore prereleases. Browser preview performs no update requests. Checks do not install updates.

Use `pnpm version:set VERSION` and `pnpm version:check` to keep npm, Cargo, Tauri and AppStream consistent; see [versioning](docs/versioning.md). The first candidate is **0.1.0-alpha.1**. A tag starts validation; publication remains gated.

## Compatibility and validation

Reference server: Vaultwarden **1.37.4**. Real tests cover PBKDF2, Argon2id, account/per-item keys, sync, integrity failures, wrong passwords, authenticator 2FA and locking. Daily compatibility CI also resolves the latest stable upstream version to an immutable image. This does not guarantee compatibility with arbitrary future versions. See [compatibility](docs/compatibility.md) and [test results](docs/validation.md).

The release workflow builds a Flatpak and source archive only after all required validation succeeds. **Releases remain blocked by an unresolved RSA advisory in the SDK dependency graph**; the check is not suppressed. Flatpak sandbox execution is also unavailable in this cloud container and must be validated by the CI runner. No downloadable release is claimed here.

[Development](docs/development.md) · [architecture](docs/architecture.md) · [security](docs/security.md) · [desktop limitations](docs/desktop.md) · [integration tests](tests/integration/README.md) · [Flatpak](packaging/flatpak/README.md)

GPL-3.0-only. Veyra selects the GPL option of the pinned official Bitwarden components; no proprietary-only SDK modules are used. See [third-party notices](THIRD_PARTY_NOTICES.md). This project is independent of Bitwarden and Vaultwarden.
