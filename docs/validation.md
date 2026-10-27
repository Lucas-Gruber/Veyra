# Current-instance validation — 2026-10-27

These are actual results from the Debian 13 x86_64 cloud instance. Remote CI, publication, a new task, Flatpak and desktop compositors are separate claims.

| Check | Outcome |
| --- | --- |
| Frozen pnpm installation | Passed with node-forge 1.4.1 patch; no high-severity advisory ignore |
| Strict TypeScript / ESLint | Passed |
| Search unit suite | 8 passed |
| Nuxt static generation | Passed |
| Browser E2E | 7 passed: five demo/keyboard/persistence cases plus two explicitly mocked desktop-IPC UI contract cases |
| Cargo formatting / Clippy all targets, warnings denied | Passed |
| Default Rust suite | 16 passed; 2 live integration tests explicitly excluded from default execution |
| Live Rust integration | Both tests executed and passed against actual Vaultwarden 1.37.4: PBKDF2/Argon2, account/per-item keys, fields/TOTP, sync, wrong password, expiry, lock, actual authenticator challenge/wrong code/correct code |
| Cipher integrity behavior | Corrupted MAC rejects refresh; previous complete snapshot remains intact |
| Final strict-mode guard | Real authentication/sync/decryption revalidated after adding explicit SDK strict-flag verification |
| Native embedded debug build | Passed |
| Native UI with actual server | Form login, real decrypted entries, exact clipboard field, separate Quick Access, manual sync and cross-window locking passed under Xvfb/DBus |
| Native clipboard | Own unchanged value clears after 30s; external replacement preserved; lock conditionally clears |
| Independent fixtures | Fresh pinned venv verified 3 accounts/2 KDFs/6 encrypted entries, actual 2FA and four output/origin safety rejections |
| GitHub Actions YAML | actionlint 1.7.12 passed; tool archive checksum verified |
| Frontend audit | Exit 0 at high threshold; one low Windows esbuild development-server advisory remains |
| Cargo audit | **Exit 1: RUSTSEC-2023-0071 rsa 0.10.0-rc.19**, no fixed upstream upgrade; five additional maintenance/unsoundness warnings |
| Flatpak metadata | Desktop/AppStream/manifest local validation passed |
| Flatpak actual sandbox | Blocked in cloud by read-only user-namespace uid-map setup; no sandbox bypass |
| GNOME / KDE / Wayland / multi-monitor / independent security review | Not verified |

Browser IPC fixtures validate UI behavior only; they are not evidence of real authentication. The live Rust tests and native server smoke provide that evidence. Real tests used only disposable synthetic credentials, never a production vault. The low-level test HTTP override is debug-only and literal-loopback-only; release builds require HTTPS.

The pinned reference image is `ghcr.io/dani-garcia/vaultwarden:1.37.4@sha256:efb3cde962015fcc036b2ea625242248611943b27212ebf4392841fb30fad055`. Current tests use the official GPL SDK commit recorded in Cargo.toml/Cargo.lock. Future compatibility is checked by the daily baseline/latest-stable workflow, not guaranteed by a version range.

The failed Cargo audit remains a release blocker. No advisory suppression or release tag is justified by these results. CI is arranged so independent build/packaging checks can still run while publication requires all gates to succeed.
