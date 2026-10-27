# Application versions and release identity

`package.json` is the canonical application version. Use canonical [Semantic Versioning](https://semver.org/) without a leading `v` or build metadata:

- `0.1.0-alpha.1`, `0.1.0-alpha.2`, `0.1.0-beta.1`, `0.1.0-rc.1`: development/prerelease channel.
- `0.1.0`: stable channel; higher precedence than its alpha/beta/RC candidates.
- Patch releases fix compatible behavior; minor releases add functionality; major releases indicate incompatible changes. Before 1.0, document compatibility changes explicitly.

```sh
pnpm version:set 0.1.0-alpha.2 --date 2026-10-28
pnpm version:check
RELEASE_TAG=v0.1.0-alpha.2 pnpm version:check
```

The setter updates `package.json`, the native Cargo manifest and local root package in Cargo.lock, Tauri configuration and the current AppStream entry together. New versions are prepended to AppStream history; existing notes remain intact. Repeating the same version is idempotent and preserves its date. Prereleases use AppStream `development`; stable versions use `stable`. Unrelated dependency versions, GNOME runtime 51 and the desktop-entry specification's `Version=1.0` are not application versions and remain unchanged.

The checker rejects version drift, a mismatched tag, malformed/noncanonical versions, duplicate AppStream history, incorrect license or inconsistent application IDs. It runs before a release is published. `pnpm version:set` validates input and prepares all replacements before modifying files. A normal downgrade is rejected. The one-time `--allow-unreleased-downgrade` flag exists to migrate the original untagged `0.1.0` draft to the first `0.1.0-alpha.1` candidate; it must never be used to relabel a published release.

Nuxt reads the canonical version into `appConfig.appVersion` at build time. Native program information reports Tauri's actual package version, derived from Cargo, rather than hardcoded UI text. The automatic update checker uses strict SemVer precedence: stable installations ignore development releases; development installations can discover newer candidates and stable releases. No update is installed automatically by this check.

A Git tag is `v` followed by the exact manifest version. Published tags are immutable: never move or force-push one. To fix a release, bump to a new version and create a new tag. The setter does not commit, create tags or publish. Tagging starts the guarded release pipeline; it does not prove a successful build. A downloadable release requires successful compatibility, security and installed Flatpak checks. The current unresolved RSA audit remains a gate; version changes do not waive it.

Flatpak packaging derives its application branch from the same version: prereleases use `beta`, stable versions use `stable`. The GNOME runtime branch (51) and desktop-entry format version (1.0) have separate meanings. The original untagged 0.1.0 development placeholder was not a published release and is not listed as release history.

About checks published GitHub Releases only: a pushed tag, draft or failed build is not offered as an update. Checks run when due at startup and daily while open, can be disabled, and expose manual status/error handling. They never automatically install binaries. A standalone Flatpak bundle is updated by installing a newer bundle; automatic package-manager delivery would require a configured app repository.
