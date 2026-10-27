# Third-party components

Veyra original code is distributed under GPL-3.0-only; see LICENSE.

Password-vault integration uses official Bitwarden Rust components from:
https://github.com/bitwarden/sdk-internal/tree/4988da44b0ac55778e3a45ec7363394509298866

The selected crates offer GPL-3.0-only OR LicenseRef-Bitwarden-SDK. Veyra uses the GPL-3.0-only option. The separately restricted `bitwarden_license` directory is not used. Upstream copyright notices remain in the dependency source. The pinned git dependency and exact transitive dependencies are recorded in `src-tauri/Cargo.lock`.

Other Rust and JavaScript libraries retain their own licenses and copyright notices. Dependency source, versions and integrity metadata are recorded by Cargo.lock and pnpm-lock.yaml. Tagged releases are configured to include Veyra source, build scripts and vendored Rust dependency sources; JavaScript dependency retrieval follows the committed pnpm lockfile. Preserve third-party license files when redistributing. A dependency pin or the GPL choice does not imply upstream endorsement or a security guarantee.
