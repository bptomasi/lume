"""Exercise user-facing Make entry points without build or personal data I/O."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


PROJECT = Path(__file__).resolve().parents[2]


class MakeCommandsTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="lume-make-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.log = self.root / "cargo.log"
        self.cargo = self.root / "cargo"
        self.cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$@" >> "$CARGO_LOG"\n')
        self.cargo.chmod(0o755)
        self.environment = dict(os.environ, HOME=str(self.root / "home"),
                                CARGO_LOG=str(self.log))
        for key in ("MAKEFLAGS", "MFLAGS", "LUME_DATA_DIR"):
            self.environment.pop(key, None)

    def make(self, *arguments):
        return subprocess.run(
            ["make", "--no-print-directory", f"CARGO={self.cargo}", *arguments],
            cwd=PROJECT, env=self.environment, capture_output=True, text=True,
            timeout=15,
        )

    def test_default_is_help_and_never_builds_or_creates_a_library(self):
        default = self.make()
        explicit = self.make("help")
        self.assertEqual(default.returncode, 0, default.stderr)
        self.assertEqual(default.stdout, explicit.stdout)
        for target in ("install", "upgrade", "check-release", "paths", "clean"):
            self.assertIn(target, default.stdout)
        self.assertFalse(self.log.exists())
        self.assertFalse((self.root / "home").exists())

    def test_paths_and_desktop_targets_follow_external_build_directory(self):
        build = self.root / "build with spaces"
        result = self.make("paths", f"CARGO_TARGET_DIR={build}")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(str(build / "release/lume"), result.stdout)
        for target in ("check-foot", "check-latency"):
            result = self.make("-n", target, f"CARGO_TARGET_DIR={build}")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(f'--binary "{build}/release/lume"', result.stdout)

    def test_make_upgrade_preserves_data_and_builds_locked(self):
        binary = self.root / "release binary"
        binary.write_bytes(b"Upgraded executable")
        prefix = self.root / "installed prefix"
        executable = prefix / "bin/lume"
        executable.parent.mkdir(parents=True)
        executable.write_bytes(b"Old executable")
        data = self.root / "data directory"
        data.mkdir()
        library = data / "library.sqlite3"
        library.write_bytes(b"Existing library must not be opened or migrated")
        config = data / "config.toml"
        config.write_bytes(b"# Keep original configuration bytes\n")

        result = self.make("upgrade", f"BINARY={binary}", f"PREFIX={prefix}",
                           f"DATA_DIR={data}")

        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(self.log.read_text().splitlines(),
                         ["build", "--locked", "--release"])
        self.assertEqual(executable.read_bytes(), b"Upgraded executable")
        self.assertEqual(library.read_bytes(),
                         b"Existing library must not be opened or migrated")
        self.assertEqual(config.read_bytes(), b"# Keep original configuration bytes\n")


if __name__ == "__main__":
    unittest.main()
