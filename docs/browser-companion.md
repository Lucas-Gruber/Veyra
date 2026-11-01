# Explicit browser filling

The Linux desktop can open a login's exact HTTPS page and fill its username and
password through the Veyra Browser Companion. This is an opt-in development
companion, not a published/signed browser-store extension. It does not submit
the form. Opening a website without filling remains a separate action.

## Set up a native Linux checkout

Build the dedicated native-messaging executable:

```sh
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin veyra-native-host
```

For Firefox:

```sh
pnpm browser:prepare --browser firefox
pnpm browser:prepare --browser firefox --host "$PWD/src-tauri/target/debug/veyra-native-host" --apply
```

Load `dist/browser-companion/firefox/manifest.json` using **about:debugging →
This Firefox → Load Temporary Add-on**. Firefox removes temporary add-ons when
it exits. The companion has the fixed add-on ID `companion@veyra.app`. A signed
distribution is deferred; do not disable Firefox signature verification.

For Chromium or Google Chrome:

```sh
pnpm browser:prepare --browser chromium
```

Use **chrome://extensions → Developer mode → Load unpacked**, selecting
`dist/browser-companion/chromium`. Copy that extension's exact ID, then register
only that ID (replace the sample value):

```sh
pnpm browser:prepare --browser chromium --chromium-id abcdefghijklmnopabcdefghijklmnop --host "$PWD/src-tauri/target/debug/veyra-native-host" --apply
```

Use `--browser chrome` instead for Google Chrome's native-host manifest location.
The command without `--apply` prepares source and prints the intended paths; it
does not register a native host or modify a browser profile. `--apply` writes a
user-owned, mode-0600 native-host manifest and a private Chromium ID allowlist.
It never enables/installs an extension in the browser itself. Rerun registration
after relocating the host binary or changing the unpacked Chromium extension ID.

Start Veyra, then open/reload an HTTPS page in the companion-enabled default
browser. This detects the companion for the current desktop session. Choose a
connected login and use **Open and fill** (default Quick Access Return). Success
is reported only after the content script confirms that both fields contain
the requested values. A missing companion, mismatched page or unsuitable form
is an error. The desktop does not fall back to pasting into an arbitrary window.

## Boundaries

- Only native Linux sessions are supported. Flatpak native-messaging integration,
  macOS/Windows hosts and signed browser-store packages are deferred.
- Targets must match the saved HTTPS URL exactly: scheme, host, port, path,
  query and fragment. Redirects to a different page do not receive credentials.
  Save the final login-page URL first. Multi-page login flows, shadow DOM,
  iframe logins, ambiguous forms and password-change/registration forms are not
  supported. A form's submission destination must have the same HTTPS origin.
- The extension needs HTTPS content-script access to see a marked destination
  after the desktop opens it. It does not enumerate/search the vault, offer
  automatic suggestions or collect page values. No HTTP-page access, clipboard,
  extension storage, external messaging, form submission or network API is used.
- Each explicit action creates a UUIDv4 intent, expires after 30 seconds and can
  release credentials only once. It pins the source login's local revision and
  saved URL. A sync or local edit changing either invalidates the request. Both
  credentials come from a single SDK view and its vault mutex remains held through
  the bounded delivery. The intent marker temporarily occupies the end
  of the URL fragment; the original fragment is restored before filling. The
  intended page can observe that marker, which contains no credential and grants
  no access to any other URL or item. History may retain the original marked
  navigation until the content script replaces it.
- The Rust service checks the current unlocked session generation before
  decrypting and during the bounded socket write. Lock/session replacement
  cancels the pending action and clears its item/URL metadata. Credentials already
  delivered to the intended browser page cannot be withdrawn by subsequently
  locking the desktop. The content script waits for a suitable form **before**
  requesting plaintext and performs no delayed retry after delivery.
- Native Messaging checks the exact browser-provided extension ID. Local IPC uses
  a user-owned mode-0700 runtime directory, a mode-0600 Unix socket, Linux
  `SO_PEERCRED` UID checks on both ends and bounded, timed message framing. There
  is no HTTP listener. This isolates other OS users and unauthorized browser
  extensions; it does not authenticate against arbitrary native code already
  executing as the same user, which can impersonate process arguments or alter
  the user's extension/configuration. That is an explicit trust boundary.
- Plaintext is held in Rust zeroizing buffers and transient extension message/DOM
  values. The browser receives the credentials by design; its JavaScript strings
  and page fields cannot provide Rust-style guaranteed memory erasure. No
  credentials are written to frontend stores, browser storage, logs or disk by
  the companion. A compromised target website/browser remains able to read filled
  credentials, as with manual entry.

## Verification and remaining evidence

```sh
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin veyra-native-host
pnpm test:browser-companion
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib browser_bridge
```

Tests cover exact URL/marker and extension sender checks, replay/expiry/session
generation rejection, native framing and caller allowlists, secure installation
paths, and real headless Chromium DOM fixtures. A fresh isolated Chromium profile
also loads the actual extension, invokes the built Rust native host, exchanges
bounded messages with a synthetic private socket fixture, and confirms the filled
DOM and restored URL. That transport test is skipped when the host binary has not
been built. A separate ignored live test against disposable Vaultwarden PBKDF2
and Argon2id accounts verifies SDK credential extraction and target/revision/lock
rejection:

```sh
VEYRA_TEST_FIXTURE_FILE=/absolute/path/to/disposable/accounts.json cargo test --locked --manifest-path src-tauri/Cargo.toml --lib actual_vaultwarden_browser_credentials_bind_target_revision_and_lock -- --ignored
```

All fixtures use synthetic credentials. These checks do not establish a complete
Vaultwarden → running native Veyra → installed Firefox/Chromium companion →
compositor desktop round trip. That native browser/compositor matrix remains
unverified. No user browser profile or real personal vault was used.

Protocol references: [Mozilla Native Messaging](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging),
[Chrome Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging).
