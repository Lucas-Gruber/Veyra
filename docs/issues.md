# Issue implementation status — 2026-11-01

These changes are committed locally for the nine open project issues. This table
records implementation and evidence; it does not mark the GitHub issues closed
or authorize a push, tag or release.

| Issue | Implemented | Evidence and remaining limits |
| --- | --- | --- |
| [#1 Dock icon](https://github.com/Lucas-Gruber/Veyra/issues/1) | Inset canonical SVG and regenerated native PNG; application/launcher IDs remain consistent. | 6.25% transparent inset, isolated launcher tests; actual dock scaling needs desktop acceptance. |
| [#2 Nested folders](https://github.com/Lucas-Gruber/Veyra/issues/2) | Folder IDs, actual empty folders, virtual parents, roll-up counts, collapse and keyboard navigation. Selecting a parent includes descendants. | Four unit cases and a connected-IPC browser test; folders named like navigation views remain distinct. |
| [#3 Shortcuts](https://github.com/Lucas-Gruber/Veyra/issues/3) | Defaults Ctrl+C username, Ctrl+Shift+C password, Ctrl+Alt+C TOTP, Return open-and-fill. All listed bindings are editable and persisted as non-sensitive preferences. | Selected search text keeps normal copy behavior; recording/collision/reload tests. Filling requires the optional [companion setup](browser-companion.md), with exact-page validation and confirmed DOM results. |
| [#4 Context menu](https://github.com/Lucas-Gruber/Veyra/issues/4) | Right-click/Shift+F10 actions, roving focus, Escape/outside dismissal and viewport positioning. | Keyboard and connected-IPC browser tests. Restricted actions are disabled and enforced again in Rust. |
| [#5 Open button](https://github.com/Lucas-Gruber/Veyra/issues/5) | Dedicated website-open button on Quick Access login results. | Browser contract verifies opening is independent of the result click/fill action. |
| [#6 Global Quick Access](https://github.com/Lucas-Gruber/Veyra/issues/6) | Single-instance CLI forwarding, cold Quick Access launch, automatic X11/Wayland portal registration and manual command fallback. | Parser/backend tests and an actual private D-Bus forwarding test. Real KDE/GNOME activation, portal prompts and focus remain unverified. |
| [#7 Settings scrolling](https://github.com/Lucas-Gruber/Veyra/issues/7) | Content scrolls independently; navigation and close remain reachable. | Browser checks at 1100×540 and 390×520 with 18px base text. |
| [#8 Entry management](https://github.com/Lucas-Gruber/Veyra/issues/8) | Shift-click/Ctrl-click selection, folder/collection assignment, duplicate, personal/organization moves, bulk trash/restore, per-item results. | Real Vaultwarden writes and partial/uncertain-result regressions. Unknown encrypted fields, attachments and passkeys block unsupported migrations. Two-organization migration has no separate live case; see [management boundaries](entry-management.md). |
| [#9 Other entry types](https://github.com/Lucas-Gruber/Veyra/issues/9) | Create/edit and explicit contents for secure notes, cards and identities. | Real supported-type CRUD tests plus browser create/edit/reveal/blur/lock/storage checks for each type. |

Independent review found and corrected future-field loss in full updates,
unverified-copy retry gating and a browser target/revision race. Session changes
close stale management dialogs and prevent a bulk loop from continuing in a new
session. See [validation](validation.md) for the final check inventory.
