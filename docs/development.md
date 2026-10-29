# Development

## Toolchains

- Node **24.19.0** (`.node-version`); Nuxt requires a compatible recent Node release.
- pnpm **11.25.0** (`packageManager`). Install with `npm install --global pnpm@11.25.0` or a trusted tool manager.
- Rust **1.99.1**, rustfmt and Clippy (`rust-toolchain.toml`).
- Linux C/C++ build tools, pkg-config, OpenSSL development headers, GTK 3 and WebKitGTK 4.1.

On Debian/Ubuntu with administrative rights:

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev patchelf
```

From the repository root:

```sh
pnpm install --frozen-lockfile
pnpm exec nuxt prepare
pnpm dev
```

`pnpm generate` produces static client assets in `.output/public`; there is no Nuxt server inside the desktop application. `pnpm desktop:dev` starts the development asset server then Tauri. `pnpm desktop:build` generates assets and builds the native binary without packaging. The desktop supports read-only personal Vaultwarden connections. Use disposable fixtures for development; never use production credentials in CI. The browser remains sample-only.

## Validation

```sh
pnpm typecheck
pnpm lint
pnpm test
pnpm generate
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
pnpm exec playwright install --with-deps chromium
pnpm test:e2e
pnpm audit --audit-level high
```

For an existing Chromium installation, set `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH=/usr/bin/chromium` rather than downloading another browser. CI uses Playwright's pinned browser. Browser tests validate the demonstration UX; they do not validate WebKit, native clipboard permissions, Wayland or Vaultwarden interoperability.

## This unprivileged cloud environment

The snapshot retains local toolchains outside the checkout. No system package writes were possible, so verified Debian package archives were extracted into `/workspace/.toolchains/linux`. Signed indexes, archive hashes and TLS verification are retained. This is a cloud development workaround, not a distribution mechanism.

```sh
export CARGO_HOME=/workspace/.toolchains/cargo
export RUSTUP_HOME=/workspace/.toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
. /workspace/.toolchains/linux-env.sh
export npm_config_cache=/workspace/.cache/npm
export XDG_CACHE_HOME=/workspace/.cache
export XDG_DATA_HOME=/workspace/.local/share
export pnpm_config_store_dir=/workspace/.cache/pnpm/store
cd /workspace/Veyra
pnpm install --frozen-lockfile --store-dir /workspace/.cache/pnpm/store
pnpm exec nuxt prepare
```

Keep these cache exports for later pnpm commands. pnpm 11 reads
`pnpm_config_store_dir`; `npm_config_store_dir` does not configure its store.
Using a different store after installation makes pnpm attempt to recreate
`node_modules`, which fails in a non-interactive restricted home directory.

The retained `/workspace/.toolchains/bootstrap-linux.sh` can refresh the local Linux dependencies. The native WebKit helper binaries use absolute paths; `/workspace/.toolchains/run-linux.sh` maps only that helper directory using PRoot. On this machine run a built application with:

```sh
dbus-run-session -- xvfb-run -a /workspace/.toolchains/run-linux.sh ./src-tauri/target/debug/veyra
```

This starts a private headless X11 session for smoke checks. It is not evidence of KDE/GNOME/Wayland support. Do not disable the WebKit sandbox to make a smoke check pass. In a normal desktop installation, install system dependencies and launch the binary normally.

Cloud tasks already use an isolated checkout. Reuse it; do not create a worktree unless explicitly requested. Installed files survive snapshots; development servers, desktop processes and DBus sessions must be restarted. Never embed credentials in setup scripts or logs.

Favorites and recent-item history in this synthetic preview are local to each WebView; they do not synchronize between the main and Quick Access windows. Real vault state will need a backend-owned metadata service.

## Contribution boundaries

Follow `AGENTS.md`. Keep synthetic metadata visibly labeled. Sensitive operations belong in Rust and must retain explicit authorization, generation checks and real integration tests. Frontend storage is restricted to validated appearance/action preferences; transient sign-in form inputs must be cleared. Run required validation after changes. The integration preview does not provide a production-security guarantee. See docs/security.md for the raw RSA finding and the source-bound Alpha3 acceptance used by CI.

## Real server regression tests

See [fixture instructions](../tests/integration/README.md). Seed fresh protected fixtures, then set `VEYRA_TEST_FIXTURE_FILE` to the output path and run `cargo test --locked --manifest-path src-tauri/Cargo.toml vaultwarden::tests::actual_vaultwarden_ -- --ignored`. This executes both live tests; a default Cargo test run intentionally does not count them as passed. The native debug app accepts a literal loopback fixture URL only with `VEYRA_ALLOW_LOOPBACK_TEST_SERVER=1`; release builds always require HTTPS.
