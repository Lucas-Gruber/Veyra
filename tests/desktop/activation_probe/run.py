#!/usr/bin/env python3
"""Optional GTK activation model; never connects to the user's desktop."""
import json
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile


def stop_group(process):
    for action in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(process.pid, action)
        except ProcessLookupError:
            break
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            pass


def main():
    programs = {name: shutil.which(name, path="/usr/bin:/bin") for name in (
        "dbus-run-session", "kwin_wayland", "Xwayland", "python3",
    )}
    missing = [name for name, path in programs.items() if path is None]
    if missing:
        print(json.dumps({"model": "gtk-x11-activation", "missing": missing}))
        return 2
    model = Path(__file__).resolve().with_name("gtk_model.py")
    with tempfile.TemporaryDirectory(prefix="veyra-activation-probe-", dir="/tmp") as temporary:
        root = Path(temporary)
        for name in ("home", "runtime", "config", "data", "cache"):
            (root / name).mkdir(mode=0o700)
        # Deliberately inherit no display, bus, authentication or user paths.
        env = {
            "PATH": "/usr/bin:/bin", "HOME": str(root / "home"),
            "XDG_RUNTIME_DIR": str(root / "runtime"),
            "XDG_CONFIG_HOME": str(root / "config"),
            "XDG_DATA_HOME": str(root / "data"),
            "XDG_CACHE_HOME": str(root / "cache"),
            "QT_QPA_PLATFORM": "offscreen", "LIBGL_ALWAYS_SOFTWARE": "1",
            "GDK_BACKEND": "x11", "NO_AT_BRIDGE": "1", "LANG": "C.UTF-8",
            "VEYRA_ACTIVATION_PROBE": "isolated",
        }
        launcher = root / "session.sh"
        launcher.write_text("#!/bin/sh\nexec " + shlex.join([
            programs["python3"], str(model), str(root),
        ]) + "\n")
        launcher.chmod(0o700)
        command = [
            programs["dbus-run-session"], "--", programs["kwin_wayland"],
            "--virtual", "--xwayland", "--socket", "veyra-activation-probe",
            "--no-lockscreen", "--no-global-shortcuts", "--no-kactivities",
            "--exit-with-session", str(launcher),
        ]
        with (root / "session.log").open("w") as log:
            process = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT,
                                       start_new_session=True)
            timed_out = False
            try:
                process.wait(timeout=25)
            except subprocess.TimeoutExpired:
                timed_out = True
            finally:
                stop_group(process)
        result_file = root / "result.json"
        result = json.loads(result_file.read_text()) if result_file.exists() else {
            "model": "gtk-x11-activation",
            "error": "Probe timed out after 25 seconds." if timed_out else "Probe exited without a result.",
        }
        expected = {
            "initial_active": True, "current_time_active": False,
            "fresh_timestamp_active": True, "entry_has_focus": True,
            "blur_fired": True, "hidden_after_external_activation": True,
        }
        passed = all(result.get(key) is value for key, value in expected.items())
        result["passed"] = passed
        print(json.dumps(result, sort_keys=True))
        if not passed:
            print((root / "session.log").read_text()[-8000:], file=sys.stderr)
        return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
