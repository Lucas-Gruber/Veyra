# Optional native activation model

Run from the repository root:

```sh
python3 tests/desktop/activation_probe/run.py
```

Requires Linux, KWin Wayland, Xwayland, `dbus-run-session`, and the system
Python's PyGObject GTK3/GdkX11 bindings. It is deliberately outside automated
unit-test discovery. No Veyra binary, account, vault, or existing desktop is used.

The runner starts KWin's virtual backend and Xwayland on a private D-Bus with
temporary HOME/XDG directories and an allowlist environment. It rejects display
`:0`, enforces a 25-second timeout, and cleans up its process group and files.

Two GTK3 model processes reproduce stale `present_with_time(0)` activation,
compare it with a fresh X11 server timestamp, then check focus loss and hiding
when the other process activates through GTK. A successful run prints one JSON
object with `passed: true`; unsuccessful runs include private-session diagnostics
on stderr. A compositor that accepts the stale timestamp will fail the expected
reproduction and requires interpreting the reported booleans.

This checks GTK activation and entry focus in the model. It does not exercise
Veyra/WebKit, a real portal shortcut, physical keyboard input, or outside clicks.
It never uses XTest, direct input-focus overrides, or compositor policy changes.
