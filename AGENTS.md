# Veyra development conventions

Veyra is a Linux-first Tauri 2 application with a statically generated Nuxt 4 / Vue 3 UI.

- Use pnpm and the committed pnpm lockfile; use Cargo and its committed lockfile for Rust.
- TypeScript is strict. Prefer small Vue Composition API components and scoped, typed IPC.
- The Rust backend owns vault sessions, authentication, tokens, keys, plaintext credential access, clipboard and privileged desktop operations. Never persist secrets in frontend stores or browser storage.
- Distinguish connected Vaultwarden mode from the explicitly labeled synthetic demo. Never present synthetic behavior as real authentication or synchronization. Real vault crypto must use official GPL-licensed Bitwarden Rust components from a published SDK release, pinned to that release's exact commit; do not use a development-head snapshot or add proprietary-only SDK paths.
- Original Veyra code and combined distributions use GPL-3.0-only. Preserve third-party notices and license obligations.
- Release only after real Vaultwarden integration is tested and required security/packaging checks pass; the user explicitly declined demo releases.
- Sole recorded exception: on 2026-10-29 the owner explicitly authorized manual publication of unchanged `v0.1.0-alpha.3` / `d7506000f7b5b66ef8958339949079811e20ca0d` with the disclosed `RUSTSEC-2023-0071` finding. See `docs/security.md`. This does not waive audits or release gates for any other version, source state or finding.
- Do not invent cryptography, weaken TLS or expose an unauthenticated local HTTP API.
- Frontend persistence is limited to non-sensitive preferences. Locking must remove vault metadata from visible interfaces and search results.
- Keep desktop capabilities minimal and distinguish desktop-only functionality from browser demonstration behavior.
- Validate with `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm generate` and `cargo test --manifest-path src-tauri/Cargo.toml`; use `cargo fmt` and Clippy when Rust changes.
- Document what is functional, deferred and unverified. Do not claim production security or compositor compatibility without evidence.
- Cloud tasks already have an isolated checkout; use it directly, without creating Git worktrees unless explicitly requested.
