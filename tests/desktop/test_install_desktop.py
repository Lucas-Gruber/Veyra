"""Exercise launcher registration using temporary XDG data and harmless stubs."""
import os
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
APP_ID = "io.github.LucasGruber.Veyra"


class DesktopIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="veyra-desktop-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.data = self.root / "xdg"
        self.launcher = self.data / "applications" / f"{APP_ID}.desktop"
        self.icon = self.data / "icons/hicolor/scalable/apps" / f"{APP_ID}.svg"
        self.marker = self.root / "launched"
        self.environment = {
            **os.environ,
            "XDG_DATA_HOME": str(self.data),
            "VEYRA_DESKTOP_TEST_RESULT": str(self.marker),
        }

    def binary(self, directory):
        binary = self.root / directory / "veyra"
        binary.parent.mkdir(parents=True)
        binary.write_text('#!/bin/sh\nprintf success > "$VEYRA_DESKTOP_TEST_RESULT"\n')
        binary.chmod(0o755)
        return binary

    def install(self, binary):
        return subprocess.run(
            ["bash", str(ROOT / "scripts/install-desktop.sh"), str(binary)],
            env=self.environment, capture_output=True, text=True, check=False,
        )

    @unittest.skipUnless(shutil.which("gio"), "gio is needed to verify actual launcher parsing")
    def test_quoted_path_launches_literal_executable(self):
        binary = self.binary('build with spaces $ dollar ` tick " quote \\ slash')
        result = self.install(binary)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.icon.read_bytes(), (ROOT / f"public/{APP_ID}.svg").read_bytes())
        self.assertIn(f"StartupWMClass={APP_ID}\n", self.launcher.read_text())
        if shutil.which("desktop-file-validate"):
            subprocess.run(["desktop-file-validate", str(self.launcher)], check=True, capture_output=True)
        launched = subprocess.run(
            ["gio", "launch", str(self.launcher)], env=self.environment,
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(launched.returncode, 0, launched.stderr)
        for _ in range(40):
            if self.marker.exists():
                break
            time.sleep(0.025)
        self.assertEqual(self.marker.read_text(), "success")

    def test_unsupported_paths_fail_before_writing(self):
        for directory in ("percent%", "equal=", "new\nline", "tab\tcharacter", "control\x7f"):
            with self.subTest(directory=directory):
                result = self.install(self.binary(directory))
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("path must not contain", result.stderr)
                self.assertFalse(self.data.exists())

    @unittest.skipUnless(shutil.which("gio"), "gio is needed to verify actual launcher parsing")
    def test_dev_launcher_preserves_runtime_arguments_and_working_directory(self):
        tools = self.root / 'runtime with spaces $ dollar ` tick " quote \\ slash'
        tools.mkdir()
        node = tools / "node"
        node.write_text("#!/bin/sh\nexit 0\n")
        node.chmod(0o755)
        pnpm = tools / "pnpm"
        pnpm.write_text(
            '#!/usr/bin/python3\nimport json, os, pathlib, sys\n'
            'pathlib.Path(os.environ["VEYRA_DESKTOP_TEST_RESULT"]).write_text('
            'json.dumps({"args": sys.argv[1:], "cwd": os.getcwd(), "path": os.environ["PATH"]}))\n'
        )
        pnpm.chmod(0o755)
        self.environment["PATH"] = str(tools) + os.pathsep + os.environ["PATH"]
        result = self.install("--dev")
        self.assertEqual(result.returncode, 0, result.stderr)
        subprocess.run(["gio", "launch", str(self.launcher)], env=self.environment, check=True)
        for _ in range(40):
            if self.marker.exists():
                break
            time.sleep(0.025)
        launched = json.loads(self.marker.read_text())
        self.assertEqual(launched["args"], ["--dir", str(ROOT), "desktop:dev"])
        self.assertEqual(launched["cwd"], str(ROOT))
        self.assertEqual(launched["path"].split(os.pathsep)[:2], [str(tools), str(tools)])
        self.assertIn(f"StartupWMClass={APP_ID}\n", self.launcher.read_text())

    def test_existing_symlink_targets_are_untouched(self):
        binary = self.binary("ordinary")
        victim = self.root / "existing-file"
        victim.write_text("keep this file")
        for target in (self.launcher, self.icon):
            with self.subTest(target=target):
                target.parent.mkdir(parents=True, exist_ok=True)
                target.symlink_to(victim)
                result = self.install(binary)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("Refusing to replace", result.stderr)
                self.assertEqual(victim.read_text(), "keep this file")
                self.assertTrue(target.is_symlink())
                target.unlink()
                self.assertFalse(self.launcher.exists())
                self.assertFalse(self.icon.exists())


if __name__ == "__main__":
    unittest.main()
