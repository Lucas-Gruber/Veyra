#!/usr/bin/env bash
# Register a local native build with the user's dock and application launcher.
set -euo pipefail
[[ $# == 1 ]] || { printf 'Usage: pnpm desktop:integrate PATH_TO_NATIVE_BINARY | --dev\n' >&2; exit 2; }
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
binary=$1
if [[ "$binary" != --dev && (! -f "$binary" || ! -x "$binary") ]]; then
  printf 'Expected an executable Veyra binary, or --dev.\n' >&2; exit 2
fi
data_root=${XDG_DATA_HOME:-$HOME/.local/share}
python3 - "$project_root" "$binary" "$data_root" <<'PY'
import os, pathlib, shutil, sys, tempfile
root, requested, data = pathlib.Path(sys.argv[1]), sys.argv[2], pathlib.Path(sys.argv[3])
app_id = 'io.github.LucasGruber.Veyra'
# Validate before creating directories or replacing a previous installation.
# The Desktop Entry specification forbids '=' in executable paths. GLib cannot
# load launchers with '%%' in that path, even though literal % is escaped that way.
def validate_path(path):
    if any(character in str(path) for character in ('%', '=')) or any(
        ord(character) < 32 or ord(character) == 127 for character in str(path)
    ):
        sys.exit('The launcher path must not contain %, = or control characters. Move the binary or checkout and retry.')

def executable(name):
    found = shutil.which(name)
    if found is None:
        sys.exit(f'{name} was not found. Run --dev from the configured Veyra development shell.')
    result = pathlib.Path(found).resolve(strict=True)
    validate_path(result)
    return result

if requested == '--dev':
    validate_path(root)
    node, pnpm, env = executable('node'), executable('pnpm'), executable('env')
    runtime_path = os.pathsep.join((str(node.parent), str(pnpm.parent), os.environ.get('PATH', '')))
    if any(ord(character) < 32 or ord(character) == 127 for character in runtime_path):
        sys.exit('The development PATH must not contain control characters.')
    command = [str(env), f'PATH={runtime_path}', str(pnpm), '--dir', str(root), 'desktop:dev']
else:
    binary = pathlib.Path(requested).resolve(strict=True)
    validate_path(binary)
    command = [str(binary)]
desktop = data / 'applications' / f'{app_id}.desktop'
icon = data / 'icons/hicolor/scalable/apps' / f'{app_id}.svg'
for destination in (desktop, icon):
    if destination.is_symlink() or (destination.exists() and not destination.is_file()):
        sys.exit(f'Refusing to replace a symlink or non-file: {destination}')
# Desktop Exec is not a shell command. Quote reserved characters so spaces,
# quotes and metacharacters in build paths remain literal.
def quote_argument(argument):
    quoted = argument.replace('%', '%%')
    for character in ('\\', '"', '`', '$'):
        quoted = quoted.replace(character, '\\' + character)
    # Desktop string values interpret backslash escapes before Exec tokenization.
    return ('"' + quoted + '"').replace('\\', '\\\\')

quoted = ' '.join(map(quote_argument, command))
source = (root / f'packaging/flatpak/{app_id}.desktop').read_text()
source = source.replace('Exec=veyra', f'Exec={quoted}')
if requested == '--dev':
    # Path is a desktop string value, not an Exec argument or shell fragment.
    working_directory = str(root).replace('\\', '\\\\').replace(' ', '\\s')
    source += f'Path={working_directory}\n'
launcher = source.encode()
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
