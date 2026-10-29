#!/usr/bin/env bash
# Register a local native build with the user's dock and application launcher.
set -euo pipefail
[[ $# == 1 ]] || { printf 'Usage: pnpm desktop:integrate PATH_TO_NATIVE_BINARY\n' >&2; exit 2; }
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
binary=$(realpath -- "$1")
[[ -f "$binary" && -x "$binary" ]] || { printf 'Expected an executable Veyra binary.\n' >&2; exit 2; }
data_root=${XDG_DATA_HOME:-$HOME/.local/share}
python3 - "$project_root" "$binary" "$data_root" <<'PY'
import pathlib, sys
root, binary, data = map(pathlib.Path, sys.argv[1:])
app_id = 'io.github.LucasGruber.Veyra'
# Desktop Exec is not a shell command. Quote reserved characters and escape %
# field codes so spaces, quotes and metacharacters in build paths remain literal.
quoted = str(binary).replace('%', '%%')
for character in ('\\', '"', '`', '$'):
    quoted = quoted.replace(character, '\\' + character)
quoted = '"' + quoted + '"'
# Desktop string values interpret backslash escapes before Exec tokenization.
quoted = quoted.replace('\\', '\\\\')
desktop = data / 'applications' / f'{app_id}.desktop'
icon = data / 'icons/hicolor/scalable/apps' / f'{app_id}.svg'
desktop.parent.mkdir(parents=True, exist_ok=True)
icon.parent.mkdir(parents=True, exist_ok=True)
source = (root / f'packaging/flatpak/{app_id}.desktop').read_text()
desktop.write_text(source.replace('Exec=veyra', f'Exec={quoted}'))
icon.write_bytes((root / f'public/{app_id}.svg').read_bytes())
print(f'Registered Veyra launcher: {desktop}')
PY
if command -v update-desktop-database >/dev/null; then update-desktop-database "$data_root/applications"; fi
if command -v gtk-update-icon-cache >/dev/null && [[ -f "$data_root/icons/hicolor/index.theme" ]]; then
  gtk-update-icon-cache -f -t "$data_root/icons/hicolor"
fi
