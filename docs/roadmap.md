# Delivery roadmap

The brief defines a mature product. Foundation and an initial read-only personal-vault integration are implemented;
future milestones are not represented as working features. See validation.md for executed checks.

1. **Foundation and polished UI.** Static Nuxt application, Tauri desktop shell,
   synthetic vault interaction, keyboard-driven Quick Access, light/dark themes,
   non-sensitive settings, explicit demo lock state, narrow native field actions,
   tests and build/packaging foundations. Validate frontend and desktop separately;
   state any untested compositor behavior.
2. **Read-only Vaultwarden.** Complete the SDK/license gate, configure server URLs,
   implement real authentication and supported 2FA, vetted compatible decryption,
   read-only sync, explicit field access and actual TOTP. Use dedicated test-server
   accounts and fixtures. Remove demo-only code from this real-vault path.
3. **Desktop and offline security.** Encrypted offline cache, session/key lifecycle,
   auto-lock, configurable clipboard expiry, portal-based global Quick Access,
   startup/tray integration where supported. Test Wayland and X11 explicitly.
4. **Vault management.** Create/edit/delete/restore, folders, favorites and supported
   organization/collection behavior, with conflict-aware synchronization and
   interrupted-write recovery. No fake successful save actions.
5. **Browser companion.** Firefox and Chromium WebExtension, authenticated Native
   Messaging, reviewed URI matching, multiple accounts, multi-step fills and
   open-and-autofill. Test host registration for native and sandboxed browsers.
6. **Distribution.** Fully offline final Flatpak build from pinned dependencies,
   functional installable bundle, metadata validation, minimal sandbox rights,
   checksum-bearing versioned releases and a KDE/GNOME test matrix. A manifest or
   CI file alone does not prove an installed application works. Flathub submission
   remains an explicit later distribution decision.

## Evidence required for acceptance

Frontend: typecheck, lint, unit tests, a static production build, keyboard-driven
functional UI tests, responsive light/dark review, focus visibility and reduced
motion. Native: Cargo formatting, Clippy and tests, built binary startup plus a
representative native operation in a display session. Flatpak: isolated build,
install and launch on a supported runtime, without development tools present.

A cloud headless test does not establish KDE Plasma, GNOME, Wayland focus handling,
portal shortcut support, multimonitor positioning or external clipboard-history
behavior. Record those as manual acceptance checks until actually performed.

Security-critical future features require failure tests as well as successful
flows: wrong credentials, unsupported KDF/2FA, invalid ciphertext, expired tokens,
lock during a request, server errors, sync conflicts, hostile origins and denied
Native Messaging/portal permissions. Never substitute a zero-test run or mock
login success for those checks.

The initial read-only connection now includes tested PBKDF2/Argon2, authenticator 2FA, decryption, sync and field/TOTP access. Other account/organization features remain outside the supported boundary. Publication remains blocked by the unresolved RSA audit and must pass installed Flatpak CI.
