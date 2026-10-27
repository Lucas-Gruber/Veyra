#!/usr/bin/env bash
# Exercise the installed bundle's WebView, native Quick Access, Rust clipboard
# command and lock action. This uses public synthetic data only. It is not a
# Vaultwarden/Wayland/security-review substitute; those have separate checks.
set -euo pipefail
app_id=io.github.LucasGruber.Veyra
for tool in flatpak dbus-run-session xvfb-run xdotool xclip timeout; do
  command -v "$tool" >/dev/null || { printf 'Smoke prerequisite missing: %s\n' "$tool" >&2; exit 2; }
done
flatpak --user info "$app_id" >/dev/null
if [[ ${1:-} != --inside-private-display ]]; then
  [[ $# == 0 ]] || { printf 'Usage: %s\n' "$0" >&2; exit 2; }
  smoke_dir=$(mktemp -d "${TMPDIR:-/tmp}/veyra-flatpak-smoke.XXXXXX")
  printf 'Installed Flatpak smoke diagnostics: %s\n' "$smoke_dir"
  exec dbus-run-session -- xvfb-run -a -s '-screen 0 1440x1000x24' \
    "$0" --inside-private-display "$smoke_dir"
fi
[[ $# == 2 && -d "$2" ]] || exit 2
smoke_dir=$(realpath -- "$2")
app_pid=
cleanup() {
  local outcome=$?
  trap - EXIT
  if [[ $outcome != 0 ]]; then
    if command -v import >/dev/null; then import -window root "$smoke_dir/failure.png" 2>/dev/null || true; fi
    printf 'Installed Flatpak smoke failed. See %s/application.log and optional failure.png.\n' "$smoke_dir" >&2
    # Application stderr contains no credential values. Preserve it as a local
    # diagnostic artifact rather than echoing arbitrary upstream errors here.
  fi
  if [[ -s "$smoke_dir/instance-id" ]]; then
    instance_id=$(tr -d '\n\r' < "$smoke_dir/instance-id")
    if [[ "$instance_id" =~ ^[0-9]+$ ]]; then flatpak kill "$instance_id" >/dev/null 2>&1 || true; fi
  fi
  if [[ -n "$app_pid" ]]; then
    kill "$app_pid" 2>/dev/null || true
    for ((stop_attempt=0; stop_attempt<20; stop_attempt++)); do
      kill -0 "$app_pid" 2>/dev/null || break
      sleep .1
    done
    if kill -0 "$app_pid" 2>/dev/null; then kill -KILL "$app_pid" 2>/dev/null || true; fi
    wait "$app_pid" 2>/dev/null || true
  fi
  exit "$outcome"
}
trap cleanup EXIT
# Test-specific preference/cache paths are inside the app's sandbox. Do not read
# or remove a user's existing ~/.var/app data, and never disable sandbox flags.
inside_tmp="/tmp/veyra-packaging-smoke-$BASHPID"
flatpak run --user --die-with-parent --instance-id-fd=3 \
  --env=XDG_CONFIG_HOME="$inside_tmp/config" \
  --env=XDG_DATA_HOME="$inside_tmp/data" \
  --env=XDG_CACHE_HOME="$inside_tmp/cache" \
  "$app_id" 3> "$smoke_dir/instance-id" > "$smoke_dir/application.log" 2>&1 &
app_pid=$!
main_window=
for ((attempt=0; attempt<25; attempt++)); do
  kill -0 "$app_pid" 2>/dev/null || { printf 'Installed app exited during startup.\n' >&2; exit 1; }
  main_window=$(xdotool search --onlyvisible --name '^(Veyra|veyra)$' 2>/dev/null | head -n 1 || true)
  [[ -n "$main_window" ]] && break
  sleep 1
done
[[ -n "$main_window" ]] || { printf 'Installed app did not show its main window.\n' >&2; exit 1; }
# A visible window alone is not success: the checks below must execute real UI
# actions and observe the native clipboard change, then clearing on explicit lock.
sleep 3
timeout 5s xdotool windowfocus --sync "$main_window"
xdotool key --clearmodifiers ctrl+alt+space
quick_window=
for ((attempt=0; attempt<20; attempt++)); do
  quick_window=$(xdotool search --onlyvisible --name '^Veyra Quick Access$' 2>/dev/null | head -n 1 || true)
  [[ -n "$quick_window" && "$quick_window" != "$main_window" ]] && break
  sleep .25
done
[[ -n "$quick_window" && "$quick_window" != "$main_window" ]] || { printf 'No separate native Quick Access window appeared.\n' >&2; exit 1; }
# A bare Xvfb session has no window manager; focus the native window explicitly.
sleep 2
timeout 5s xdotool windowfocus --sync "$quick_window"
# Locked Quick Access focuses Server address. Four tabs reach the explicit
# Explore demo button (server -> email -> password -> connect -> explore).
xdotool key --clearmodifiers --delay 150 Tab Tab Tab Tab Return
sleep 2
xdotool type --clearmodifiers --delay 80 github
copy_started=$SECONDS
xdotool key --clearmodifiers ctrl+p
clipboard_value=
for ((attempt=0; attempt<15; attempt++)); do
  clipboard_value=$(timeout 2s xclip -selection clipboard -o 2>/dev/null || true)
  [[ "$clipboard_value" == 'Veyra-demo-only!2026' ]] && break
  sleep 1
done
[[ "$clipboard_value" == 'Veyra-demo-only!2026' ]] || { printf 'Installed UI did not copy the expected public demo field.\n' >&2; exit 1; }
(( SECONDS - copy_started < 15 )) || { printf 'Clipboard copy was too slow to distinguish explicit locking from timed expiry.\n' >&2; exit 1; }
printf 'Installed Flatpak: UI search and native clipboard copy passed.\n'
timeout 5s xdotool windowfocus --sync "$main_window"
xdotool key --clearmodifiers ctrl+l
for ((attempt=0; attempt<8; attempt++)); do
  clipboard_value=$(timeout 2s xclip -selection clipboard -o 2>/dev/null || true)
  [[ -z "$clipboard_value" ]] && break
  sleep 1
done
[[ -z "$clipboard_value" ]] || { printf 'Explicit lock did not clear the app-owned clipboard value.\n' >&2; exit 1; }
(( SECONDS - copy_started < 25 )) || { printf 'Lock clearing must be observed before the 30-second clipboard timer.\n' >&2; exit 1; }
printf 'Installed Flatpak: explicit native lock and clipboard clearing passed.\n'
