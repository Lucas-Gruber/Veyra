# Security boundaries and release gates

Veyra is an early read-only integration preview and has not received independent security review. Do not equate passing tests with production security.

## Threats and controls

| Input or boundary | Current control |
| --- | --- |
| Server address and transport | HTTPS with certificate validation, no embedded credentials/query/fragment, no redirects, timeouts and response size limits |
| Server-controlled KDF | Single prelogin fetch; signed integers checked before conversion; PBKDF2 100k..2m iterations, Argon2id 16..256MiB, 2..10 iterations, 1..8 lanes |
| Keys and encrypted entries | Pinned official GPL Bitwarden crypto, explicit strict-decryption flag check; integrity failures reject the refresh |
| IPC and frontend | Narrow local-window permissions; metadata projection excludes tokens, keys, passwords, seeds, notes and custom-field values; explicit field access |
| Password input | Transient form and IPC only, cleared after submit/unmount; moved into zeroizing native storage; never browser persistence |
| Lock/expiry/racing operations | Cancellation plus generation authority in Rust, keys/tokens cleared, metadata removed across both windows |
| Clipboard | Native worker keeps ownership, clears after 30s only if unchanged; lock also requests conditional clear |
| Untrusted vault metadata | Vue text rendering, bundled local icons, no remote favicon/content execution; HTTPS-only explicit website opening |
| Synchronization | Read-only, transactional supported-personal-vault snapshot; previous complete snapshot retained on failure |

A debug-only explicit flag permits HTTP solely to literal loopback test servers. It cannot enable plaintext remote connections or weaken release TLS. Tests use disposable synthetic accounts, never production credentials.

The native key store/allocator, JS engine, OS clipboard and third-party history managers may retain copies beyond application control. Zeroization reduces lifetimes; it does not promise perfect erasure. Decrypted metadata exists in WebView memory while unlocked. Explicit reveals necessarily cross IPC as transient plaintext. The app currently has no encrypted offline cache, biometric unlock, background refresh-token storage or automatic inactivity lock; server-token expiry and explicit locking are implemented.

## Supported account behavior

V1 personal vaults are the current boundary. Organizations, signed/new account crypto, unsupported item types and unavailable authentication methods are reported, not silently downgraded. Only authenticator TOTP two-factor login is supported; email/WebAuthn and other flows are not claimed. Locking signs out and requires full sign-in. Notes/cards/identities expose metadata only in this version.

## Dependency audit status

The previous high-severity `node-forge` signature-verification advisory **GHSA-86w9-cpqp-85rv** is fixed by the compatible, integrity-verified 1.4.1 dependency pin. No advisory ignore is used. The latest lockfile-only `pnpm audit --audit-level high` exits 0; low-severity dependency advisories remain, including the Windows-development-server `esbuild` advisory GHSA-g7r4-m6w7-qqqr. Development binds to loopback. Node development-server packages are not a password-vault crypto implementation and are not deployed as a runtime server inside Tauri. Auditing the lockfile does not install dependencies or bypass the build's frozen-lockfile and explicit 1,440-minute minimum-release-age checks.

The published SDK `rust-v4.0.0` is pinned to `b865d1db0025d9d12bea871a3a131c05aaeed205`. Its graph still introduces **RUSTSEC-2023-0071** in `rsa` 0.10.0-rc.19: the Marvin timing side-channel advisory, with no fixed upgrade currently available. Selecting this SDK release does not fix the advisory. `cargo audit` exits **1**. The release gate remains failing; the advisory is not suppressed. Personal AES vault use does not establish a general proof of unreachable RSA risk. A supported upstream remedy or a separately reviewed resolution is required before publishing an installable release. A source candidate tag and unpublished release draft may exist while validation fails; neither means the release has passed its gates or offers an installable build.

Personal-only sessions now initialize only the AES user key using the pinned official SDK's key-store APIs. They do not unwrap, import or retain the unused account RSA private key. The encrypted private-key envelope is still checked for its supported format; this does not claim verification of its unused contents. See the [scoped source assessment and regression boundaries](rsa-assessment.md). This reduces unnecessary secret handling while leaving the dependency audit and release gate intact.

Additional Cargo warnings remain: unmaintained `proc-macro-error`, `zbus_names`, `zvariant`, `zvariant_derive`, and `glib` 0.18.5 `VariantStrIter` unsoundness (RUSTSEC-2024-0429). Incompatible major dependency overrides are not a safe workaround. Re-run audits and inspect actual operations before release.

## Remaining engineering gates

Installed Flatpak behavior passed on GitHub for the previous alpha candidates, including native Quick Access, clipboard copying and explicit locking. It must pass again for the SDK-release migration. This cloud container blocks user namespace uid-map writes; no sandbox bypass is used. Real KDE/GNOME Wayland/X11, portals, multi-monitor focus, accessibility/contrast and external clipboard managers require their own tests. Browser autofill needs authenticated Native Messaging and reviewed origin matching; no unauthenticated local service is provided.
