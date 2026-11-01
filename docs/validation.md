# Quick Access focus and outside dismissal — 2026-11-01

Quick Access activation now waits for the native window to be visible and
restored before requesting focus. GTK's focus request otherwise ignores hidden
or minimized windows. Actual native focus events activate the search/sign-in
form; losing focus hides the popup without trying to steal focus back. Reported
restoration/activation failures hide it. Browser previews make background
content inert, close on outside pointer down or window blur, and restore their
opener's keyboard focus only while the page is active. Dismissed or unmounted
components ignore deferred input-focus callbacks and remove event listeners.

The full Chromium suite passed 56 cases. Four new focus/dismissal cases also
passed under headless WebKit; they cover background focus prevention, immediate
outside dismissal, browser blur, native IPC activation contracts and retained
searches. Headless tabs emulate focus independently, so browser blur is supplied
as a DOM event; this is not a native window-manager acceptance test. TypeScript,
ESLint, static Nuxt generation, 47 frontend unit tests, Rust formatting/Clippy and 42 Rust unit tests
passed (14 environment/child-fixture cases remain explicitly ignored).
The working version stays `0.1.1`. Actual KDE/GNOME/Wayland and Windows/macOS
activation/dismissal still require desktop verification.

# Quick Access, tray and settings — 2026-11-01

Local checks after the selected-item retention, main-client shortcut, tray and
settings fixes, with the owner's working version `0.1.1` unchanged:

| Check | Result |
| --- | --- |
| Strict TypeScript, ESLint, static Nuxt generation and coordinated version manifests | Passed |
| Frontend unit suite | 47 passed |
| Full Chromium UI suite | 52 passed with explicit demo/synthetic IPC fixtures |
| Headless WebKit targeted UI checks | Seven settings, eight retention and five main-client shortcut cases passed across dedicated runs; includes 320px width and long bindings |
| Rust formatting, Clippy all targets with warnings denied, default tests | Passed; 42 passed and 14 environment/child-fixture tests explicitly ignored |
| Native application and native host debug build | `cargo build --locked --manifest-path src-tauri/Cargo.toml --bins --features custom-protocol` passed with generated static assets |
| Linux tray host and real KSNI registration | Private D-Bus probe passed without DISPLAY/Wayland; icon pixel export and host availability checked |
| Flatpak tray D-Bus permissions | Real `xdg-dbus-proxy --filter --talk=org.kde.StatusNotifierWatcher` round trip passed with unique-name registration, icon pixels, menu interface/layout and activation; no tray-name ownership grant |
| Single-instance CLI | Private D-Bus forwarding test passed without a user desktop session |
| Browser companion | Five passed, none skipped, including an isolated Chromium extension/native-host/DOM round trip with synthetic bridge transport |
| Audit-policy and launcher regression tests | 18 audit-policy and four isolated launcher/GIO cases passed |
| Changed CI workflow | Pinned actionlint 1.7.12 passed |

Search and selection remain only in memory for 30 seconds after dismissal and
clear on locking or session replacement. Tests cover reordering by stable item
ID, expiration after suspend and late fill responses after reopening. Settings
checks include wheel/keyboard scrolling, six viewport sizes with 18px text,
background inertness and focus restoration. WebKit checks used temporary runtime
libraries without changing the host installation or disabling its sandbox.

The published tray-icon 0.25.1 package was verified against its Cargo checksum;
all upstream files are retained with their licenses. The only upstream source
patch enables the documented KSNI unique-name option in Flatpak. Its provenance
is recorded under `src-tauri/vendor/tray-icon/VEYRA-PATCH.md`.

Actual panel icon placement/menu interaction, native GTK scrollbar styling,
compositor focus, Windows/macOS tray behavior and an installed current Flatpak
remain unverified. Live Vaultwarden cases were not rerun for these desktop
changes; earlier live results are recorded below. The changed source/dependency
inputs remain outside the Alpha3 RSA exception. No release, tag or push was made.

# Earlier open-project-issue baseline — 2026-11-01

The nine open issues have local implementations and separate commits; see
[the issue inventory](issues.md). Final checks on this workstation:

| Check | Result |
| --- | --- |
| Strict TypeScript, ESLint, static Nuxt generation and coordinated version manifests | Passed; version remains unreleased `0.1.0-alpha.4` |
| Frontend unit suite | 47 passed |
| Full Playwright UI suite | 33 passed, using explicit demo/synthetic IPC fixtures |
| Rust formatting, Clippy all targets with warnings denied, default tests | Passed; 41 unit tests, 11 explicitly ignored environment-specific tests |
| Current native debug binaries | `cargo build --locked --bin veyra --bin veyra-native-host` passed after static asset generation |
| Actual Vaultwarden 1.37.4 at the committed digest | All ten live cases executed successfully across dedicated runs: KDF/login/2FA/org sync, supported CRUD, management/preservation/uncertain results and browser target/revision binding |
| Single-instance CLI | Actual private D-Bus forwarding test passed; no GTK or user desktop window |
| Browser companion | Five passed, none skipped; includes real isolated Chromium extension → Rust native host → synthetic private socket → confirmed DOM fill |
| Audit-policy regression tests | 18 passed; synthetic unit inputs isolated from the working version/count, actual checkout rejects the recorded Alpha3 exception |
| Local launcher/GIO tests | Four passed with harmless stubs and temporary XDG directories |
| CI workflow syntax | Pinned actionlint 1.7.12 passed; CI now runs private D-Bus, launcher and companion tests |
| Frontend dependency audit | Exit 0 at the high threshold; two low and seven moderate findings remain |
| Current Rust dependency audit | Not run: cargo-audit unavailable locally. The known unresolved RSA finding has no exception for these changed sources/build inputs. |

Independent review corrected unknown encrypted-field loss in full editor/favorite
updates, missing write gates after failed copy verification, a browser
target/revision race and stale management selections across session replacement.
Regression checks exercise each correction. Review is not an independent
production security audit.

Responsive UI checks used synthetic data: context menu at 900×700 and 18px base
text, management/settings at 390×844 and 18px, plus the automated short-window
settings cases. The brand badge now wraps rather than being clipped. The local
development launcher/icon was refreshed without launching Veyra in the user's
desktop. Dedicated containers, volumes, temporary credentials and test servers
were cleaned up; user browser profiles and real vaults were not used.

Actual global-key activation, portal permission/focus behavior, dock rendering
and current installed Flatpak execution remain unverified. Browser tests do not
establish the full live Veyra/Compositor/Firefox or Chromium round trip; Flatpak
native messaging is deferred. Two-organization migration and a server-enforced
read-only member/policy matrix have no separate live acceptance case. No new
source release is approved by these tests.

# Organization/write and desktop fixes — 2026-10-30

This checkout adds organization/collection selection, encrypted entry management,
explicit contents, scalable typography and desktop integration. Local checks:

| Check | Result |
| --- | --- |
| `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm generate` | Passed; 40 frontend unit tests |
| Playwright complete suite | 17 passed; explicitly synthetic IPC/browser contracts, including clearing hidden Quick Access login fields |
| Cargo formatting, Clippy all-targets with warnings denied, unit tests | Passed; 26 tests, six live tests separately ignored in unit runs |
| Dedicated Vaultwarden 1.37.4 at committed digest | Six live tests passed: personal KDFs, 2FA, org keys/collections, supported entry CRUD, conflict/uncertain-write recovery |
| Audit-exception regression tests | 17 passed; changed application sources remain outside the Alpha3 exception |
| Frontend audit | Exit 0 at high threshold; two low findings remain |
| Local launcher/GIO tests | Four passed in isolated XDG directories with harmless stubs |
| Debian launcher packaging | Actual bundler with harmless binary stub; canonical ID desktop entry/icon verified, not a functional native-app package test |
| Visual UI review | Synthetic demo/org/editor at 1280×820 and 760×560, including large text |
| `cargo audit` on this workstation | Not run: cargo-audit is not installed; unresolved RSA finding is not waived |

The development launcher/icon was registered for the current checkout with
`pnpm desktop:integrate --dev`. No Veyra application was launched in the user's
desktop session. Native KDE/GNOME/Wayland/X11 dock rendering, compositor focus-loss
events and the reported GPU startup failure still need verification on the
user's actual setup. New-source installed Flatpak testing and independent
security review are not established by these local checks.

Writes preserve original custom fields, extra URIs, attachments and passkeys;
only supported editor fields change. Real organization owner writes pass, while
local restriction flags are tested fail-closed. A dedicated server-side
read-only-member/policy matrix remains unverified. Trash/Restore check a fresh
revision but the server provides no atomic CAS. Update/favorite revisions use
the server's check, including Vaultwarden's timestamp tolerance; this is not a
guarantee of strict cross-client serialization. No permanent deletion, existing
owner/collection moves, folder management or attachment/custom-field editor is
implemented. No release, tag or push was created.

# Current-instance validation — 2026-10-28

These are actual results from the Debian 13 x86_64 cloud instance. Remote CI, publication, a new task, Flatpak and desktop compositors are separate claims.

| Check | Outcome |
| --- | --- |
| Frozen pnpm installation | Empty-cache installation passed after the 2026-10-28 age cutoff, including all 1,025 supply-chain policy entries; no age exception or advisory ignore |
| Strict TypeScript / ESLint | Passed |
| Frontend unit suite | 33 passed: 8 search and 25 coordinated-versioning cases |
| Nuxt static generation | Passed |
| Browser E2E | 11 passed on the alpha UI: five demo/keyboard/persistence, two mocked desktop-IPC connection and four About/update cases |
| Cargo formatting / Clippy all targets, warnings denied | Passed |
| Default Rust suite | 24 passed; 2 live integration tests explicitly excluded from default execution |
| Live Rust integration | Both tests executed and passed against actual Vaultwarden 1.37.4: PBKDF2/Argon2, account/per-item keys, fields/TOTP, sync, wrong password, expiry, lock, actual authenticator challenge/wrong code/correct code |
| Cipher integrity behavior | Corrupted MAC rejects refresh; previous complete snapshot remains intact |
| Final strict-mode guard | Real authentication/sync/decryption revalidated after adding explicit SDK strict-flag verification |
| Native embedded builds | Debug and optimized release builds passed |
| Native UI with actual server | Form login, real decrypted entries, exact clipboard field, separate Quick Access, manual sync and cross-window locking passed under Xvfb/DBus |
| Native clipboard | Own unchanged value clears after 30s; external replacement preserved; lock conditionally clears |
| Independent fixtures | Fresh pinned venv verified 3 accounts/2 KDFs/6 encrypted entries, actual 2FA and four output/origin safety rejections |
| GitHub Actions YAML | actionlint 1.7.12 passed; tool archive checksum verified |
| Frontend audit | Exit 0 at high threshold; low-severity dependency advisories remain, including Windows esbuild development-server behavior |
| Cargo audit | **Exit 1: RUSTSEC-2023-0071 rsa 0.10.0-rc.19**, no fixed upstream upgrade; five additional maintenance/unsoundness warnings |
| Flatpak metadata | Desktop/AppStream/manifest local validation passed |
| Flatpak actual sandbox | Blocked in cloud by read-only user-namespace uid-map setup; no sandbox bypass |
| GNOME / KDE / Wayland / multi-monitor / independent security review | Not verified |

Browser IPC fixtures validate UI behavior only; they are not evidence of real authentication. The live Rust tests and native server smoke provide that evidence. Real tests used only disposable synthetic credentials, never a production vault. The low-level test HTTP override is debug-only and literal-loopback-only; release builds require HTTPS.

The pinned reference image is `ghcr.io/dani-garcia/vaultwarden:1.37.4@sha256:efb3cde962015fcc036b2ea625242248611943b27212ebf4392841fb30fad055`. Current tests use the official GPL SDK commit recorded in Cargo.toml/Cargo.lock. Future compatibility is checked by the daily baseline/latest-stable workflow, not guaranteed by a version range.

The failed Cargo audit remains a release blocker. Source candidate tags and unpublished drafts do not waive it: an installable release requires all validation gates to succeed. No advisory suppression is used. CI is arranged so independent build/packaging checks can still run while publication requires all gates to succeed.

## First remote CI diagnosis

The initial main push reached GitHub Actions. The runner correctly rejected node-forge 1.4.1 because it was published at 2026-10-27 04:44:07 UTC and the package manager enforces a 24-hour minimum release age. The security patch stays pinned; the age check is not bypassed. It becomes eligible after 2026-10-28 04:44:07 UTC. The daily CI is scheduled after that cutoff and also reruns baseline/latest Vaultwarden tests. A separate service-start failure was diagnosed as missing `/data` volume and corrected with a dedicated named service volume: upstream deliberately rejects anonymous Docker volumes. The volume is confined to its ephemeral CI runner.

## 0.1.0-alpha.1 program information and versioning

The About/update implementation passed 8 native update tests, 25 versioning tests, the existing 8 search tests and 11 browser E2E cases. Rust's default suite now runs 24 passing tests with 2 live tests explicitly excluded; both live Vaultwarden tests were rerun and passed on the alpha candidate. Type checking, ESLint, Rustfmt, Clippy and static/native debug builds passed.

The native About screen was exercised against the real public GitHub HTTPS API while the vault was locked. It displayed the actual `0.1.0-alpha.1` runtime version, Linux/x86_64/GPL metadata and a successful “No published releases yet” response. Browser preview made zero GitHub requests. UI fixtures additionally exercised available updates, automatic-check opt-out and explicit network failure without a false “up to date.”

The compatibility pipeline on commit `514299c` completed both real server tests on GitHub (run `40457861341`, job `122035483340`). Source candidate tagging and draft preparation do not change the known audit/minimum-age/publication gates.

## Personal-key hardening and CI follow-up — 2026-10-28

Personal-only sessions now initialize only the AES user key through the unchanged
official SDK key-store APIs. The unused account RSA private key is not unwrapped
or imported. The [source assessment](rsa-assessment.md) records the reviewed
paths and their limits; the RSA advisory is still a release blocker.

The updated code passed 24 default Rust tests and both separately executed live
Vaultwarden tests. Live assertions verify AES-key presence, account RSA/signing-key
absence, full snapshot and password preservation after corrupted MAC/RSA-envelope
responses, and AES-key removal on lock. Rustfmt, Clippy with warnings denied,
TypeScript, ESLint, 33 frontend unit tests and coordinated-version checks passed.

Both the original alpha candidate's full Tauri optimized build and the hardened
native optimized build succeeded. The updated release binary was exercised in a
private D-Bus/Xvfb host session: separate Quick Access, demo search, native
clipboard copying and explicit-lock clearing passed. This is a native host
check, not an installed Flatpak test or evidence of real GNOME/KDE Wayland support.
The release source-archive procedure also produced a relocatable vendor
configuration whose complete 698-package Cargo graph resolved offline.

CI now audits the frontend lockfile without installing dependencies or running
lifecycle scripts; the Linux build retains frozen installation and the explicit
1,440-minute release-age policy. Rust CI disables incremental artifacts and debug
symbols to reduce disk use without changing the tests or release profile.

After the 2026-10-28 04:44:07 UTC cutoff, an isolated checkout with an empty pnpm
store downloaded the dependency graph and passed all 1,025 supply-chain entries
in a frozen installation. Coordinated-version validation also passed. This
resolves the package-age blocker; it does not resolve the separate RSA audit.

GitHub run `40634187389` confirmed the fresh installation, all 70 executed tests
and the optimized Linux build. Its Flatpak sandbox build reached AppStream
composition, where Ubuntu 24.04 lacked the recommended SVG decoder. The exact
AppStream 1.0.2 error was reproduced with the repository assets: composition failed
without `librsvg2-common` and succeeded after installing only that signed distro
package. The packaging prerequisites now include it explicitly. The corrected
run `40636148532` then passed the installed Flatpak gate, including Quick Access,
search, native clipboard copy and explicit-lock clearing. The alpha-2 tag run
`40638890126` passed those checks too; publication was skipped because of RSA.

## Published SDK release migration — 0.1.0-alpha.3

The four direct SDK dependencies now use the published `rust-v4.0.0` release at
`b865d1db0025d9d12bea871a3a131c05aaeed205`, rather than the later development
snapshot. The adapter required no API changes. All 24 locked SDK packages offer
the GPL option; no proprietary-only SDK path or unsafe debug/memory-hardening
feature is enabled. The locked package graph decreased from 698 to 696 packages.

The migration passed 24 default Rust tests, both separately executed actual
Vaultwarden tests, 33 frontend unit tests and 11 browser E2E cases: 70 executed
tests in total. Rustfmt, Clippy with warnings denied, type checking, ESLint,
static generation and coordinated-version validation passed. The optimized
Alpha3 binary built successfully and its exact SHA-256 was preserved through a
native-host smoke covering main/Quick Access, search, native clipboard copying
and explicit locking. The labelled fixture server was stopped with its test
container/data retained.

The new release pin still resolves `rsa` 0.10.0-rc.19. A fresh raw `cargo audit`
returned exit 1 with exactly RUSTSEC-2023-0071 and the previous five warnings.
No audit suppression or publication-policy exception has been applied.

The exact Alpha3 tag commit `d7506000f7b5b66ef8958339949079811e20ca0d`
then passed all 70 tests, the optimized Linux build and the installed Flatpak
smoke in [GitHub run 40732003146](https://github.com/Lucas-Gruber/Veyra/actions/runs/40732003146).
The smoke exercised native Quick Access, search, copying and explicit-lock
clipboard clearing. The sole failed job was the dependency security audit;
publication was therefore skipped. The downloaded artifact ZIP matched GitHub's
SHA-256 digest, and its Flatpak matched the included `SHA256SUMS`:
`7f3b8858860610b4620c0c52b343baf09348af12c8dd409bb2706abe49b69dc3`.

The committed source-archive helper also completed locally for that exact
commit. Its 696-package Rust vendor graph resolved offline after extraction.
The relocated isolated pnpm content store restored the frontend with a frozen
lockfile and all 1,025 supply-chain policy checks; registry metadata still
requires network access as documented in [source-build.md](source-build.md).
Third-party license files and notices were verified in the archive. This
packaging evidence does not resolve the failed audit or authorize publication.

## Alpha3 owner publication decision — 2026-10-29

The owner explicitly answered “Lass den audit und bau es” to the specific question
about releasing Alpha3 with a documented RSA-risk exception. This authorizes
manual publication only of unchanged `v0.1.0-alpha.3` /
`d7506000f7b5b66ef8958339949079811e20ca0d`, using published SDK `rust-v4.0.0` at
`b865d1db0025d9d12bea871a3a131c05aaeed205`. The fresh audit remains exit 1 for
`RUSTSEC-2023-0071` / `rsa` 0.10.0-rc.19, with four unmaintained and one unsoundness
warning. This acceptance does not fix the dependency or certify the application;
audits and automatic publication gates remain unchanged for future releases.

[Draft preparation run 40914689543](https://github.com/Lucas-Gruber/Veyra/actions/runs/40914689543)
successfully attached the tested Flatpak, source archive, checksums and original
security evidence. All five checksum entries passed after downloading the files
again. Their assessment and validation metadata precede the owner's decision;
the `publication_authorized: false` field describes that earlier snapshot. The
release notes record the subsequent Alpha3-only approval.

## Applying the accepted Alpha3 finding to CI — 2026-10-29

The initial manual publication left the validation workflow red because the raw
Rust audit still exited 1. Following the owner's failure report, CI now evaluates
the same Alpha3-only acceptance separately from the unsuppressed audit. The
fixed policy binds the reviewed application, SDK, Cargo graph, exact finding and
build inputs. It cannot accept a different version or changed finding. Raw JSON,
stderr and the true exit code remain downloadable, and the summary identifies
the accepted risk. The immutable published tag and binaries are unchanged.

The policy evaluator passed 17 negative/transition tests and an evaluation of
the actual 2026-10-29 audit report. The real report retains exit 1; the separate
decision accepts only the recorded finding. A clean future audit passes without
this policy, including new informational warnings. Actionlint passed, and an
independent review verified all 70 protected-file hashes and the evaluator hash.
Application, dependency and packaging inputs remain identical to `d750600`.

## NVIDIA/Wayland startup report — 2026-10-29

A user reported GTK assertions followed by Wayland protocol error 71 on
CachyOS/KDE/Wayland/NVIDIA while starting the development app. Upstream WebKit
and Tauri track matching graphics-stack failures; the exact cause on this
machine is not yet confirmed. The per-launch DMA-BUF workaround passed the
native X11/Xvfb main/Quick Access, search, clipboard and explicit-lock smoke.
The affected user subsequently confirmed that the app starts with
`WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm desktop:dev` on that setup. This is a
user-confirmed startup result, not an automated NVIDIA/Wayland test or evidence
for focus, clipboard, shortcuts or general compositor compatibility. Separate
renderer/explicit-sync workarounds and their limits are documented in
[desktop.md](desktop.md).
No native code or published release files changed.
# Local desktop fixes and organization integration — 2026-10-29

Local typecheck/lint and the real-server personal, authenticator and organization
tests passed against disposable Vaultwarden 1.37.4 at the committed baseline
digest. Organization coverage includes RSA-unwrapped keys, collection names,
per-cipher-key credentials, failed-refresh preservation and lock clearing.
These results do not validate the user's compositor or authorize a release.
The changed sources are outside the unchanged-Alpha3 audit exception.

