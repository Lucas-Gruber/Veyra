#!/usr/bin/env bash
# Package an explicitly supplied, already-built native x86_64 binary.
# The final Flatpak build uses local sources, disabled downloads and a sandbox.
set -euo pipefail
if [[ $# != 1 ]]; then
  printf 'Usage: %s PATH_TO_NATIVE_RELEASE_BINARY\n' "$0" >&2
  exit 2
fi
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
binary=$(realpath -- "$1")
[[ -f "$binary" && -x "$binary" ]] || { printf 'Expected an executable native binary.\n' >&2; exit 2; }
for tool in flatpak flatpak-builder desktop-file-validate appstreamcli python3 dbus-run-session xvfb-run xdotool xclip timeout; do
  command -v "$tool" >/dev/null || { printf 'Required tool missing: %s\n' "$tool" >&2; exit 2; }
done
app_id=io.github.LucasGruber.Veyra
manifest="$project_root/packaging/flatpak/$app_id.json"
if flatpak --user info "$app_id" >/dev/null 2>&1; then
  printf 'Refusing to replace an existing user installation of %s. Use a clean packaging account.\n' "$app_id" >&2
  exit 2
fi
version=$(python3 - "$project_root" "$binary" <<'PY'
import json, pathlib, struct, sys, tomllib, xml.etree.ElementTree as ET
root, binary = map(pathlib.Path, sys.argv[1:])
package = json.loads((root / 'package.json').read_text())
tauri = json.loads((root / 'src-tauri/tauri.conf.json').read_text())
cargo = tomllib.loads((root / 'src-tauri/Cargo.toml').read_text())
metadata = ET.parse(root / 'packaging/flatpak/io.github.LucasGruber.Veyra.metainfo.xml')
version = package['version']
if version != tauri['version'] or version != cargo['package']['version'] or version != metadata.find('releases/release').get('version'):
    raise SystemExit('Application, Cargo, Tauri and AppStream versions must match.')
import re
if re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?', version) is None:
    raise SystemExit('A MAJOR.MINOR.PATCH version with optional prerelease suffix is required.')
with binary.open('rb') as stream:
    header = stream.read(20)
if len(header) != 20 or header[:4] != b'\x7fELF' or header[4:6] != b'\x02\x01' or struct.unpack('<H', header[18:20])[0] != 62:
    raise SystemExit('Expected a little-endian 64-bit x86_64 Linux ELF binary.')
print(version)
PY
)
flatpak_branch=stable
if [[ "$version" == *-* ]]; then flatpak_branch=beta; fi
desktop-file-validate "$project_root/packaging/flatpak/$app_id.desktop"
appstreamcli validate --no-net "$project_root/packaging/flatpak/$app_id.metainfo.xml"
# Install these separately while online. This command neither installs nor updates
# runtime dependencies, and the final build does not receive network access.
flatpak --user info org.gnome.Platform//51 >/dev/null
flatpak --user info org.gnome.Sdk//51 >/dev/null
mkdir -p "$project_root/artifacts"
stage=$(mktemp -d "$project_root/artifacts/.flatpak-staging.XXXXXX")
printf 'Flatpak build staging: %s\n' "$stage"
python3 - "$manifest" "$binary" "$stage" "$flatpak_branch" <<'PY'
import hashlib, json, pathlib, shutil, sys
manifest_path, binary, stage = map(pathlib.Path, sys.argv[1:4])
manifest = json.loads(manifest_path.read_text())
manifest['default-branch'] = sys.argv[4]
for source in manifest['modules'][0]['sources']:
    origin = binary if source.get('dest-filename') == 'veyra' else (manifest_path.parent / source['path']).resolve()
    name = source.get('dest-filename', origin.name)
    shutil.copyfile(origin, stage / name)
    source['path'] = name
    source['sha256'] = hashlib.sha256((stage / name).read_bytes()).hexdigest()
(stage / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
(stage / 'input-binary.sha256').write_text(hashlib.sha256(binary.read_bytes()).hexdigest() + '  veyra\n')
PY
flatpak-builder --show-manifest "$stage/manifest.json" > "$stage/resolved-manifest.json"
flatpak-builder --user --arch=x86_64 --sandbox --disable-download --disable-rofiles-fuse \
  --state-dir="$stage/state" --repo="$stage/repo" "$stage/build" "$stage/manifest.json"
# Check the packaged binary against the selected runtime, not the developer host.
flatpak-builder --run "$stage/build" "$stage/manifest.json" /bin/sh -ec \
  'ldd /app/bin/veyra > /tmp/veyra-ldd.txt; cat /tmp/veyra-ldd.txt; ! grep -F "not found" /tmp/veyra-ldd.txt'
output="$project_root/artifacts/Veyra-$version-linux-x86_64.flatpak"
if [[ -e "$output" ]]; then
  printf 'Refusing to overwrite existing bundle: %s\n' "$output" >&2
  exit 2
fi
flatpak build-bundle --arch=x86_64 --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  "$stage/repo" "$output" "$app_id" "$flatpak_branch"
# Install only this newly built app into a clean user installation. Preserve
# existing runtimes, preferences and data; clean up only the app we just added.
installed_by_this_script=false
installed_commit=
cleanup_installation() {
  local outcome=$?
  trap - EXIT
  if [[ "$installed_by_this_script" == true ]]; then
    current_commit=$(flatpak --user info --show-commit "$app_id" 2>/dev/null || true)
    if [[ "$current_commit" == "$installed_commit" ]]; then
      flatpak uninstall --user --noninteractive "$app_id" >/dev/null 2>&1 || printf 'Could not remove the smoke-test installation of %s.\n' "$app_id" >&2
    fi
  fi
  exit "$outcome"
}
trap cleanup_installation EXIT
flatpak install --user --noninteractive "$output"
installed_by_this_script=true
installed_commit=$(flatpak --user info --show-commit "$app_id")
"$project_root/scripts/smoke-flatpak.sh"
printf 'Built and functionally checked installed bundle: %s\n' "$output"
printf 'Retained source checksums and staged build: %s\n' "$stage"
