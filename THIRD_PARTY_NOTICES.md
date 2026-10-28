# Third-party components

Veyra original code is distributed under GPL-3.0-only; see LICENSE.

Password-vault integration uses the published Bitwarden Rust SDK release
`rust-v4.0.0`, pinned to its exact source commit:
https://github.com/bitwarden/sdk-internal/tree/b865d1db0025d9d12bea871a3a131c05aaeed205

The selected crates offer GPL-3.0-only OR LicenseRef-Bitwarden-SDK. Veyra uses the GPL-3.0-only option. The separately restricted `bitwarden_license` directory is not used. Upstream copyright notices remain in the dependency source. The pinned git dependency and exact transitive dependencies are recorded in `src-tauri/Cargo.lock`.

The upstream repository's `LICENSE` notice at that release states:

> Source code in this repository is covered by one of two licenses:
> (i) the GNU General Public License (GPL) v3.0
> (ii) the BITWARDEN SOFTWARE DEVELOPMENT KIT LICENSE v2.0.
>
> The default license throughout the repository is your choice of GPL v3.0 OR
> BITWARDEN SOFTWARE DEVELOPMENT KIT LICENSE unless the header specifies another
> license. Anything contained within a directory named bitwarden_license is
> covered solely by the BITWARDEN SOFTWARE DEVELOPMENT KIT LICENSE.
>
> GPL v3.0:
> https://github.com/bitwarden/sdk-internal/blob/main/LICENSE_GPL.txt
>
> BITWARDEN SOFTWARE DEVELOPMENT KIT LICENSE v2.0:
> https://github.com/bitwarden/sdk-internal/blob/main/LICENSE_SDK.txt
>
> No grant of any rights in the trademarks, service marks, or logos of Bitwarden is
> made (except as may be necessary to comply with the notice requirements as
> applicable), and use of any Bitwarden trademarks must comply with Bitwarden
> Trademark Guidelines
> <https://github.com/bitwarden/server/blob/main/TRADEMARK_GUIDELINES.md>.

The full GPL-3.0 license selected for Veyra and these SDK components is included
in Veyra's `LICENSE` file. The quoted alternative-license notice does not select
the alternative SDK license or grant rights to restricted components.

Other Rust and JavaScript libraries retain their own licenses and copyright notices. Dependency source, versions and integrity metadata are recorded by Cargo.lock and pnpm-lock.yaml. Tagged releases include Veyra source, build scripts, vendored Rust dependency sources and isolated pnpm package contents for the Linux x86_64/glibc target, preserving their license files. See `SOURCE-BUILD.md` in the archive, or `docs/source-build.md` in the repository, for rebuilding and the remaining online registry-metadata checks. Preserve third-party license files when redistributing. A dependency pin or the GPL choice does not imply upstream endorsement or a security guarantee.
