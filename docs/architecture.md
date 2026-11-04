# Architecture

Veyra is a statically generated Nuxt 4 client in Tauri 2. It has no runtime Nuxt server and loads no remote content inside its privileged WebView. The frontend uses Vue Composition API and Pinia only for non-sensitive preferences.

```mermaid
flowchart LR
  Main[Main WebView] --> IPC[Scoped Tauri commands]
  Quick[Quick Access WebView] --> IPC
  IPC --> Session[In-memory Rust session and generation gate]
  Session --> HTTP[Bounded HTTPS transport]
  HTTP --> VW[User's Vaultwarden server]
  Session --> SDK[Pinned official Bitwarden crypto]
  Session --> Clip[Conditional clipboard worker]
```

## Authority and data lifecycle

Rust owns the connection, session token, SDK key store and encrypted ciphers. The server is selected explicitly; URLs require HTTPS, have no embedded credentials/query/fragment and preserve a reverse-proxy subdirectory. Redirects are not followed. Bodies, item counts and KDF work are bounded before use. A narrow literal-loopback HTTP exception requires a debug build and an explicit test environment flag; release builds cannot enable it.

The master password exists briefly in the Vue form/IPC input, is immediately cleared from form state and moved into `Zeroizing` in Rust. It is not persisted. Rust sends a derived authentication hash, initializes SDK crypto and performs the initial sync before declaring a connected session. SDK authentication is not used because its second prelogin fetch would undermine the single checked KDF budget. All cryptographic derivation, unwrap, cipher decryption and TOTP use official SDK operations; the project does not invent a cipher protocol.

The explicitly requested sign-in profile stores exactly `serverUrl` and `email`
in `AppConfigDir/login-preferences.json`. Three scoped Rust commands load, save
and remove it; no general filesystem IPC is exposed. Saving validates HTTPS,
rejects embedded URL credentials/query/fragment and unknown fields, and
atomically replaces a size-bounded JSON file. Unix directories/files use
`0700`/`0600`; these identifiers are stored as local plaintext, without session
credentials. Neither identifier is written to browser storage. Submitting a
sign-in saves them even if authentication fails; typing or dismissing does not.
Both windows reload saved values without replacing an unfinished draft. Locking
retains this profile while clearing vault data and secrets; **Forget saved
sign-in details** deletes it. File failures do not prevent manual sign-in.

Only bounded display metadata crosses general IPC. Passwords and TOTP codes require explicit scoped access; seeds, tokens, keys, notes and custom fields are excluded from snapshots. The main window can explicitly request transient notes/card/identity contents or a scoped editor draft; drafts never enter shared frontend state or persistence. Contents hide after 60 seconds, window blur or lock. Login content viewing does not load passwords or TOTP seeds; those use the separate credential commands. An explicit editor can request a login seed to edit it. Reveals expire and guard against lock/selection races. Both windows read the same native metadata snapshot and observe session events. Frontend state is not an authentication authority.

Locking cancels current work and advances a generation. Stale async login/sync completions cannot restore it. Tokens and SDK keys are dropped/cleared and metadata disappears in both windows. Session expiry also locks. No refresh-token persistence or offline unlock is claimed. Sync is transactional: a fresh SDK key store, organization keys, collections and all supported ciphers must verify before replacing the previous snapshot. Unknown newer item types are counted visibly. Personal-only sessions do not import an unused account private key; organization sessions unwrap it using the official SDK and use its RSA organization-key path. Lock clears both stores and all organization metadata.

## Authenticated writes

Only main can request editor drafts/contents or save an encrypted item. Quick
Access can change a favorite; Trash/Restore are rejected there using the native
caller identity. The backend loads the original complete SDK cipher view,
checks permissions/reprompt and the captured session generation, and encrypts
with the pinned SDK. Existing per-cipher keys, custom fields, extra URIs,
attachments and passkeys survive supported edits. Unknown or opaque encrypted payload fields block editor saves instead of being silently discarded. New entries use SDK-generated per-cipher
keys. Changed passwords retain up to five encrypted history entries.

Editor updates include the full `lastKnownRevisionDate`; detected conflicts are errors, never an automatic retry/overwrite. Vaultwarden's
revision comparison has timestamp tolerance; it is not a strict atomic CAS
contract for changes made within that tolerance. Favorites use a partial endpoint after a fresh permission/revision check, preserving all encrypted payload fields. This endpoint has no atomic CAS. Trash/Restore are reversible server operations
with a fresh revision precheck; these endpoints do not support an atomic CAS,
so a race between checking and applying remains possible. No permanent-delete
command exists. A dispatched write is never automatically retried. Unknown
outcomes or failed post-write refreshes require sync/inspection before another
write. Native mutex/session guards keep snapshot commits in operation order.

Batch folder/collection assignment, duplication and owner migration remain backend-owned. Cross-owner organization moves verify a destination copy before soft-deleting the source. Accepted but unverifiable copies and unknown write outcomes stop remaining batch items and require synchronization. See [entry management](entry-management.md) for preservation checks and non-atomic server boundaries.

Explicit Linux browser filling uses a separate native-messaging binary and a private Unix socket. Rust binds a single-use expiring intent to the exact saved HTTPS URL, cipher revision and session generation; credentials come from one SDK view under a vault guard held through the bounded write. The extension verifies its sender, top frame and full target, fills a suitable same-origin form and acknowledges the result without submitting. The Veyra WebViews receive no credential values from this operation. See [browser companion](browser-companion.md) for the browser/same-UID trust boundaries.

## SDK and license decision

Veyra uses the published Bitwarden Rust SDK release `rust-v4.0.0` (2026-09-28). `bitwarden-core`, `bitwarden-crypto`, `bitwarden-vault` and API models are pinned to its exact commit `b865d1db0025d9d12bea871a3a131c05aaeed205`, rather than a later development snapshot. The repository and APIs remain internal and are not a public third-party compatibility guarantee. Secrets Manager SDK alone does not implement password-vault management. Selected SDK crates offer `GPL-3.0-only OR LicenseRef-Bitwarden-SDK`; Veyra selects GPL-3.0-only and licenses the combined application accordingly. Proprietary-only `bitwarden_license` paths are excluded. This does not establish upstream endorsement or compatibility with every Bitwarden/Vaultwarden deployment.

Supported account crypto is V1 with supported AES-CBC-HMAC envelopes and PBKDF2/Argon2id. Newer account states, unrecognized KDFs and failed integrity verification fail closed. The SDK's strict decryption feature flag is checked explicitly so a changed SDK flag cannot silently enable lenient decoding. Updates require a deliberate source-pin review and server matrix tests.

Sources: https://github.com/bitwarden/sdk-internal/releases/tag/rust-v4.0.0 , https://github.com/bitwarden/sdk-internal/tree/b865d1db0025d9d12bea871a3a131c05aaeed205 , its root LICENSE/LICENSE_GPL.txt, and https://github.com/dani-garcia/vaultwarden .

## Delivery boundaries

Flatpak uses a maintained GNOME 51 runtime. The current manifest packages a prebuilt native binary offline; this is offline packaging, not yet a fully vendored source build in the SDK sandbox. CI installs and functionally smoke-tests the bundle before publication. Global shortcuts and browser Native Messaging are implemented with the verification limits documented above. Signed companion distribution, Flatpak native messaging, offline encrypted storage, new-folder creation and conflict merging remain deferred.
