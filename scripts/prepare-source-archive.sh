#!/usr/bin/env bash
# Prepare the committed application's source and locked dependency package contents.
# All pnpm content comes from a new public-registry store, never the user's cache.
set -euo pipefail
umask 077
[[ $# == 1 ]] || { printf 'Usage: %s OUTPUT_DIRECTORY\n' "$0" >&2; exit 2; }
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
for tool in git cargo pnpm python3 tar realpath; do
  command -v "$tool" >/dev/null || { printf 'Required source tool missing: %s\n' "$tool" >&2; exit 2; }
done
mkdir -p -- "$1"
output_dir=$(realpath -- "$1")
commit=$(git -C "$project_root" rev-parse --verify 'HEAD^{commit}')
source_epoch=$(git -C "$project_root" log -1 --format=%ct "$commit")
scratch=$(mktemp -d "${TMPDIR:-/tmp}/veyra-release-source.XXXXXX")
source_dir="$scratch/source"
pending=
cleanup() {
  local outcome=$?
  trap - EXIT
  rm -rf -- "$scratch" || true
  if [[ -n "$pending" ]]; then rm -f -- "$pending" || true; fi
  exit "$outcome"
}
trap cleanup EXIT
mkdir "$source_dir"
git -C "$project_root" archive "$commit" | tar -x -C "$source_dir"
identity=$(python3 - "$source_dir/package.json" <<'PY'
import json, re, sys
package = json.load(open(sys.argv[1]))
version, manager = package['version'], package['packageManager']
if re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?', version) is None:
    raise SystemExit('Expected a canonical application version.')
if re.fullmatch(r'pnpm@[0-9]+\.[0-9]+\.[0-9]+', manager) is None:
    raise SystemExit('Expected an exact pnpm packageManager pin.')
print(version + '\t' + manager)
PY
)
read -r version manager <<< "$identity"
[[ "pnpm@$(pnpm --version)" == "$manager" ]] || { printf 'Use the packageManager version pinned by the committed source.\n' >&2; exit 2; }
if [[ ${GITHUB_REF_TYPE:-} == tag && ${GITHUB_REF_NAME:-} != "v$version" ]]; then
  printf 'Source version does not match the release tag.\n' >&2
  exit 2
fi
output="$output_dir/Veyra-$version-source.tar.gz"
[[ ! -e "$output" && ! -L "$output" ]] || { printf 'Refusing to overwrite %s\n' "$output" >&2; exit 2; }
for generated in pnpm-store rust-vendor node_modules SOURCE-BUILD.md SOURCE-IDENTITY.json; do
  [[ ! -e "$source_dir/$generated" && ! -L "$source_dir/$generated" ]] || { printf 'Committed source occupies reserved path: %s\n' "$generated" >&2; exit 2; }
done
[[ ! -e "$source_dir/.cargo/config.toml" && ! -L "$source_dir/.cargo/config.toml" ]] || { printf 'Review the committed Cargo configuration before adding vendor settings.\n' >&2; exit 2; }

# Empty configuration/cache roots avoid collecting credential files, registry
# configuration, unrelated packages or local postinstall side effects.
export XDG_CONFIG_HOME="$scratch/config"
export XDG_CACHE_HOME="$scratch/cache"
export XDG_DATA_HOME="$scratch/data"
export npm_config_cache="$scratch/npm-cache"
export pnpm_config_store_dir="$source_dir/pnpm-store"
pnpm_options=(--config.userconfig=/dev/null --config.registry=https://registry.npmjs.org --config.side-effects-cache=false)
cd "$source_dir"
CI=true pnpm "${pnpm_options[@]}" fetch --frozen-lockfile --ignore-scripts --store-dir "$pnpm_config_store_dir"
pnpm "${pnpm_options[@]}" store status --store-dir "$pnpm_config_store_dir"
# fetch creates a temporary virtual store. The archive carries only its isolated
# package-content store; consumers reconstruct node_modules through normal pnpm.
rm -rf -- "$source_dir/node_modules"
mkdir -p "$source_dir/.cargo"
cargo vendor --locked --versioned-dirs --manifest-path "$source_dir/src-tauri/Cargo.toml" "$source_dir/rust-vendor" > "$source_dir/.cargo/config.toml"
python3 - "$source_dir" "$commit" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
config = root / '.cargo/config.toml'
config.write_text(config.read_text().replace(str(root / 'rust-vendor'), 'rust-vendor'))
(root / 'SOURCE-IDENTITY.json').write_text(json.dumps({
    'commit': sys.argv[2], 'dependency_platform': 'linux-x64-glibc',
    'pnpm_registry': 'https://registry.npmjs.org',
    'frontend_policy_metadata_requires_network': True,
}, indent=2) + '\n')
PY
cp "$source_dir/docs/source-build.md" "$source_dir/SOURCE-BUILD.md"
# Verify the portable Cargo source mapping without building another native target.
cargo metadata --offline --locked --manifest-path "$source_dir/src-tauri/Cargo.toml" --format-version 1 > "$scratch/cargo-metadata.json"
pending=$(mktemp "$output_dir/.Veyra-$version-source.XXXXXX")
tar --sort=name --mtime="@$source_epoch" --owner=0 --group=0 --numeric-owner -czf "$pending" -C "$source_dir" .
python3 - "$pending" "$output" <<'PY'
import os, sys
# Exclusive creation also protects an output created after the initial check.
os.link(sys.argv[1], sys.argv[2])
os.unlink(sys.argv[1])
PY
pending=
printf 'Prepared source archive for %s (%s): %s\n' "$version" "$commit" "$output"
