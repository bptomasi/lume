"""Qualify distro terminal configuration and isolated compositor cleanup."""

import importlib
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch


with patch.object(sys, "path", [str(Path(__file__).resolve().parents[1]), *sys.path]):
    terminal = importlib.import_module("terminal_support")
    wayland = importlib.import_module("qualify_wayland")


class TerminalSupportTests(unittest.TestCase):
    def test_foot_configuration_handles_distro_and_current_versions(self):
        for version, security in [("1.16.2", False), ("1.20.0", True),
                                  ("1.21.0", True), ("2.0.0", True)]:
            with patch.object(terminal.subprocess, "check_output",
                              return_value=f"foot version: {version} +sixel"):
                configuration = terminal.foot_configuration()
                self.assertIn("DejaVu Sans Mono", configuration)
                self.assertEqual("[security]" in configuration, security)
        with patch.object(terminal.subprocess, "check_output", return_value="unknown"):
            with self.assertRaisesRegex(RuntimeError, "version"):
                terminal.foot_configuration()

    def test_startup_errors_identify_process_and_retain_bounded_log_tails(self):
        process = Mock(args=["/usr/bin/foot"], returncode=2)
        process.poll.return_value = 2
        with self.assertRaisesRegex(RuntimeError, r"foot exited \(2\)"):
            terminal.wait_for("startup", lambda: False, process)
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)
            (path / "foot.log").write_text("x" * 5000 + "missing seat")
            message = terminal.startup_failure("startup failed", path, ["foot.log"])
            self.assertIn("missing seat", message)
            self.assertLess(len(message), 4200)

    def test_private_compositor_is_stopped_when_client_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            process = Mock()
            process.poll.return_value = None
            with (
                patch.dict(wayland.os.environ, DISPLAY=":1"),
                patch.object(wayland.shutil, "which", return_value="tool"),
                patch.object(wayland.subprocess, "Popen", return_value=process),
                patch.object(wayland, "wait_for_compositor", return_value="wl_seat"),
                patch.object(wayland.subprocess, "run",
                             side_effect=OSError("client failed")),
                self.assertRaisesRegex(OSError, "client failed"),
            ):
                wayland.qualify(Path(temporary), ["false"])
            process.terminate.assert_called_once()
            process.wait.assert_called_once_with(timeout=5)

    def test_missing_seat_is_rejected_before_running_client(self):
        socket = Mock()
        socket.is_socket.return_value = True
        result = subprocess.CompletedProcess([], 0, "wl_compositor", "")
        with patch.object(wayland.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "no wl_seat"):
                wayland.wait_for_compositor(Mock(), socket, {})


if __name__ == "__main__":
    unittest.main()
