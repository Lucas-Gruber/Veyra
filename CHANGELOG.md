# Changelog

## 0.1.0-alpha.1 — 2026-10-27

Initial candidate with real read-only personal Vaultwarden integration.

- Linux Tauri application with main vault and keyboard-first Quick Access windows.
- PBKDF2/Argon2 authentication, authenticator 2FA, strict decryption, manual synchronization, password/username/TOTP access and shared session locking.
- Program information with actual native version, release channel, platform, architecture and license.
- Automatic daily/manual Veyra release checks, explicit failure/no-release states and a privacy setting; no automatic installation.
- Central SemVer tooling and manifest/tag checks; prerelease Flatpak branch separated from stable.
- Daily baseline/latest-stable Vaultwarden compatibility tests and guarded Flatpak release workflow.

This is an early integration preview without independent security review. Organizations, editing, offline unlock/cache, browser autofill and system-wide shortcuts remain unsupported. An unresolved RSA advisory and required packaging checks prevent publication until their gates pass. A source tag or draft is not a published installable release.
