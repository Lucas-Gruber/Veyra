# Building the release source archive

The archive contains the committed Veyra source, Cargo dependency sources in
`rust-vendor`, and package contents fetched from the committed pnpm lockfile in
`pnpm-store`. Package license files are retained. `SOURCE-IDENTITY.json` records
the source commit and dependency platform. These frontend packages cover the
Linux x86_64/glibc release target; additional platforms can require other packages.

Install the exact Node, pnpm and Rust versions declared by `.node-version`,
`package.json` and `rust-toolchain.toml`, plus the native system prerequisites in
`docs/development.md`. Compilers, operating-system packages, the GNOME runtime
and SDK, and development services are not included in the archive.

From the extracted archive root:

```sh
export pnpm_config_store_dir="$PWD/pnpm-store"
pnpm --config.side-effects-cache=false install --frozen-lockfile --store-dir "$pnpm_config_store_dir"
pnpm generate
cargo build --offline --locked --release --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

Keep the store export for later pnpm commands. The normal frozen install verifies
integrity and applies the project's supply-chain policies; no trust or age-check
override is needed. pnpm 11 still needs public registry metadata for those policy
checks even when all required package contents are present locally. Therefore the
frontend bootstrap is **not claimed to be fully offline**. The archive deliberately
does not include a user's registry configuration, authentication files, global
cache, pre-existing `node_modules`, or local postinstall side effects.

Cargo uses the relative `.cargo/config.toml` mapping to `rust-vendor`; its complete
locked dependency graph is checked offline while preparing the archive. This does
not install a missing Rust toolchain or system library without network access.

Run the validation commands documented in `docs/development.md`. Actual server
tests require a separately started disposable Vaultwarden instance and protected
synthetic fixture manifest. For Flatpak packaging, install the signed runtime/SDK
and follow `packaging/flatpak/README.md`; its final packaging build remains offline.

Archive headers use the source commit's timestamp. Package-manager metadata can
vary between fetches, so neither byte-identical archives nor byte-identical native
binaries are promised. Verify the release's `SHA256SUMS` before extracting.

## Preparing an unpublished release draft

The manually dispatched `prepare-release-draft.yml` workflow can attach files to
an existing, empty prerelease draft when the normal publication gate is blocked.
Run it from `main` with the existing prerelease `tag` and the completed tagged
release `run_id`. It requires successful Linux, installed Flatpak and real
Vaultwarden jobs, checks the tag/source binding and artifact hashes, and adds a
fresh source archive, checksums and the original security-job log. A failing
security audit remains visible. The workflow never publishes a release, grants
an advisory exception or overwrites assets; partial uploads require review.
