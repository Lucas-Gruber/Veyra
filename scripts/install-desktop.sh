#!/usr/bin/env bash
# Register a local native build with the user's dock and application launcher.
set -euo pipefail
[[ $# == 1 ]] || { printf 'Usage: pnpm desktop:integrate PATH_TO_NATIVE_BINARY\n' >&2; exit 2; }
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
binary=$1
[[ -f "$binary" && -x "$binary" ]] || { printf 'Expected an executable Veyra binary.\n' >&2; exit 2; }
data_root=${XDG_DATA_HOME:-$HOME/.local/share}
python3 - "$project_root" "$binary" "$data_root" <<'PY'
import os, pathlib, sys, tempfile
root, binary, data = map(pathlib.Path, sys.argv[1:])
binary = binary.resolve(strict=True)
app_id = 'io.github.LucasGruber.Veyra'
# Validate before creating directories or replacing a previous installation.
# The Desktop Entry specification forbids '=' in executable paths. GLib cannot
# load launchers with '%%' in that path, even though literal % is escaped that way.
if any(character in str(binary) for character in ('%', '=')) or any(
    ord(character) < 32 or ord(character) == 127 for character in str(binary)
):
    sys.exit('The Veyra binary path must not contain %, = or control characters. Move the binary and retry.')
desktop = data / 'applications' / f'{app_id}.desktop'
icon = data / 'icons/hicolor/scalable/apps' / f'{app_id}.svg'
for destination in (desktop, icon):
    if destination.is_symlink() or (destination.exists() and not destination.is_file()):
        sys.exit(f'Refusing to replace a symlink or non-file: {destination}')
# Desktop Exec is not a shell command. Quote reserved characters so spaces,
# quotes and metacharacters in build paths remain literal.
quoted = str(binary)
for character in ('\\', '"', '`', '$'):
    quoted = quoted.replace(character, '\\' + character)
quoted = '"' + quoted + '"'
# Desktop string values interpret backslash escapes before Exec tokenization.
quoted = quoted.replace('\\', '\\\\')
source = (root / f'packaging/flatpak/{app_id}.desktop').read_text()
launcher = source.replace('Exec=veyra', f'Exec={quoted}').encode()
image = (root / f'public/{app_id}.svg').read_bytes()

def replace_file(destination, contents):
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as output:
            temporary = pathlib.Path(output.name)
            output.write(contents)
        temporary.chmod(0o644)
        os.replace(temporary, destination)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)

replace_file(desktop, launcher)
replace_file(icon, image)
print(f'Registered Veyra launcher: {desktop}')
PY
if command -v update-desktop-database >/dev/null; then update-desktop-database "$data_root/applications"; fi
if command -v gtk-update-icon-cache >/dev/null && [[ -f "$data_root/icons/hicolor/index.theme" ]]; then
  gtk-update-icon-cache -f -t "$data_root/icons/hicolor"
fi
