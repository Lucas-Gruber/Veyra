"""Internal GTK3 model processes launched only by run.py's private session."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
if (os.environ.get("VEYRA_ACTIVATION_PROBE") != "isolated"
        or os.environ.get("XDG_RUNTIME_DIR") != str(root / "runtime")
        or os.environ.get("WAYLAND_DISPLAY") != "veyra-activation-probe"
        or not re.fullmatch(r":[1-9][0-9]*(?:\.0)?", os.environ.get("DISPLAY", ""))):
    raise RuntimeError("Refusing to use an unverified desktop session.")

import gi  # noqa: E402
gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("GdkX11", "3.0")
from gi.repository import Gdk, GdkX11, GLib, Gtk  # noqa: E402


def present_fresh(window):
    surface = window.get_window()
    assert isinstance(surface, GdkX11.X11Window)
    surface.set_events(surface.get_events() | Gdk.EventMask.PROPERTY_CHANGE_MASK)
    timestamp = GdkX11.x11_get_server_time(surface)
    surface.set_user_time(timestamp)
    window.present_with_time(timestamp)


def external():
    window = Gtk.Window(title="GTK activation model: external")
    window.set_default_size(300, 250)
    window.add(Gtk.Entry())
    window.show_all()
    (root / "external-ready").touch()

    def activate():
        request = root / "activate-external"
        if request.exists():
            request.unlink()
            present_fresh(window)
        return True

    GLib.timeout_add(50, activate)
    GLib.timeout_add_seconds(16, lambda: Gtk.main_quit() or False)
    Gtk.main()


def quick():
    window = Gtk.Window(title="GTK activation model: quick")
    window.set_decorated(False)
    window.set_keep_above(True)
    window.set_accept_focus(True)
    window.set_default_size(520, 380)
    entry = Gtk.Entry()
    window.add(entry)
    result = {"model": "gtk-x11-activation"}
    children = []
    hide_on_blur = False

    def blur(*_):
        if hide_on_blur:
            result["blur_fired"] = True
            window.hide()
        return False

    def start_external():
        result["initial_active"] = window.is_active()
        window.hide()
        children.append(subprocess.Popen([sys.executable, __file__, str(root), "external"]))
        GLib.timeout_add(80, external_ready)
        return False

    def external_ready():
        if not (root / "external-ready").exists():
            return True
        GLib.timeout_add(500, baseline)
        return False

    def baseline():
        window.show_all()
        window.present_with_time(0)
        entry.grab_focus()
        GLib.timeout_add(600, check_baseline)
        return False

    def check_baseline():
        result["current_time_active"] = window.is_active()
        present_fresh(window)
        entry.grab_focus()
        GLib.timeout_add(600, check_fresh)
        return False

    def check_fresh():
        nonlocal hide_on_blur
        result["fresh_timestamp_active"] = window.is_active()
        result["entry_has_focus"] = entry.has_focus()
        hide_on_blur = True
        (root / "activate-external").touch()
        GLib.timeout_add(600, finish)
        return False

    def finish():
        result["hidden_after_external_activation"] = not window.get_visible()
        (root / "result.json").write_text(json.dumps(result))
        Gtk.main_quit()
        return False

    window.connect("focus-out-event", blur)
    window.show_all()
    present_fresh(window)
    entry.grab_focus()
    GLib.timeout_add(600, start_external)
    GLib.timeout_add_seconds(12, lambda: Gtk.main_quit() or False)
    try:
        Gtk.main()
    finally:
        for child in children:
            child.terminate()
            child.wait(timeout=2)


if __name__ == "__main__":
    external() if sys.argv[2:] == ["external"] else quick()
