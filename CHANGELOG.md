# Changelog

## 0.1.1 — unreleased

- Personal and organization vaults; encrypted create/edit, favorites and reversible trash/restore for logins, secure notes, cards and identities.
- Nested and empty folders, keyboard-accessible context menus, Shift-click multi-selection, folder/collection assignment, duplication and verified owner migration.
- Configurable shortcuts with the requested clipboard defaults; dedicated website-open button in Quick Access.
- Linux single-instance `veyra --quick-access` launch, native X11 and Wayland portal shortcut registration with explicit status/fallback.
- Optional development browser companion for explicit exact-page filling through native messaging; separate setup required, no automatic form submission.
- Readable text sizing, independently scrolling settings, dock icon inset and Linux startup defaults based on Noryn.
- Quick Access remembers its search and selected item ID for 30 seconds, clears both on lock/session changes, and exposes a visible configurable Open Veyra shortcut (Ctrl+E).
- Native tray open/Quick Access/lock/quit actions for Linux, Windows and macOS; Linux StatusNotifier availability in Settings and minimal Flatpak D-Bus permissions. Actual desktop panel and Windows/macOS runtime acceptance remain unverified.
- Settings constrain scrolling to their content, keep navigation reachable in short/narrow windows, block background interaction and restore focus when closed.

This candidate is not published. Current application/build inputs are outside the recorded Alpha3 RSA exception. Native compositor acceptance, the complete installed browser matrix and required release checks remain outstanding.

## 0.1.0-alpha.1 — 2026-10-27

Initial candidate with real read-only personal Vaultwarden integration.

- Linux Tauri application with main vault and keyboard-first Quick Access windows.
- PBKDF2/Argon2 authentication, authenticator 2FA, strict decryption, manual synchronization, password/username/TOTP access and shared session locking.
- Program information with actual native version, release channel, platform, architecture and license.
- Automatic daily/manual Veyra release checks, explicit failure/no-release states and a privacy setting; no automatic installation.
- Central SemVer tooling and manifest/tag checks; prerelease Flatpak branch separated from stable.
- Daily baseline/latest-stable Vaultwarden compatibility tests and guarded Flatpak release workflow.

This is an early integration preview without independent security review. Organizations, editing, offline unlock/cache, browser autofill and system-wide shortcuts remain unsupported. An unresolved RSA advisory and required packaging checks prevent publication until their gates pass. A source tag or draft is not a published installable release.
