# Veyra's minimal Flatpak patch

This directory preserves the complete published `tray-icon` 0.25.1 crate package.
The original MIT/Apache-2.0 licenses, copyright headers and package metadata remain
in place. It is selected explicitly through `[patch.crates-io]` in Veyra's Cargo
manifest. This patch does not change any Bitwarden SDK source or cryptography.

Upstream source: https://github.com/tauri-apps/tray-icon/tree/b09ceda9f7357b4a1f6bc361fd912a7dfca077cc

Published crate: https://static.crates.io/crates/tray-icon/tray-icon-0.25.1.crate

Original archive SHA-256 (verified before extraction):
`b2b9c52859a94554803ccd4a24b98f74148ebc73b90676d783f3490b1bff9d72`

The only upstream source change is in `src/platform_impl/ksni/mod.rs`: before
KSNI's initial `spawn()`, call its documented `disable_dbus_name(true)` when
`/.flatpak-info` exists or the runtime exports `FLATPAK_ID`. Outside Flatpak the
original behavior is preserved. In Flatpak, the status icon and D-Bus menu are
exported on the connection's unique name rather than claiming a well-known
`org.kde.StatusNotifierItem-PID-ID` name. The Tauri wrapper does not expose this
upstream KSNI option. This keeps Flatpak permissions limited to talking to
`org.kde.StatusNotifierWatcher`; no broad name ownership or session-bus access
is introduced.

All other crate files are byte-identical to the verified archive. This notice is
Veyra's additional documentation; it is not an upstream endorsement.
