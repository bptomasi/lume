"""Frame correlation and fragmented terminal acknowledgements are mandatory."""

import base64
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


def module(name):
    path = Path(__file__).resolve().parents[1] / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


latency = module("terminal_latency")


class TerminalLatencyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lume-timing-")
        self.directory = Path(self.temporary.name)
        self.now = 1_000_000
        self.probe = latency.Probe(self.directory, lambda: self.now)
        self.sent = []

    def tearDown(self):
        self.probe.close()
        self.temporary.cleanup()

    def request(self, native=False):
        request = {"id": 0, "scenario": "owned", "sent_ns": 500_000,
                   "input": base64.b64encode(b"j").decode(),
                   "expected": "READY", "native": native}
        self.probe.receive(json.dumps(request).encode() + b"\n", self.sent.append)

    def frame(self, content):
        return latency.BEGIN + content + latency.END

    def test_unrelated_frame_cannot_acknowledge_a_key(self):
        self.request()
        self.probe.output(self.frame(b"Collection updated"), self.sent.append)
        self.assertEqual(self.sent, [b"j"])
        self.assertNotIn("frame_ns", self.probe.pending)

    def test_fragmented_frame_and_reply_keep_timings_and_unrelated_input(self):
        self.request()
        frame = self.frame(b"READY")
        self.now = 2_000_000
        for byte in frame:
            self.probe.output(bytes([byte]), self.sent.append)
        self.assertEqual(self.sent, [b"j", latency.STATUS])
        self.assertEqual(self.probe.response(b"x\x1b["), b"")
        self.now = 3_000_000
        self.assertEqual(self.probe.response(b"0ny"), b"xy")
        result = json.loads((self.directory / "latency.jsonl").read_text())
        self.assertEqual(result["input_to_frame_ms"], 1.5)
        self.assertEqual(result["input_to_terminal_ack_ms"], 2.5)
        self.assertIsNone(self.probe.pending)

    def test_native_sample_requires_pixels_in_the_matching_frame(self):
        self.request(native=True)
        self.probe.output(self.frame(b"READY"), self.sent.append)
        self.assertEqual(self.sent, [b"j"])
        self.probe.output(self.frame(b"READY\x1bPq...\x1b\\"), self.sent.append)
        self.assertEqual(self.sent[-1], latency.STATUS)

    def test_overlapping_admission_is_rejected(self):
        self.request()
        with self.assertRaisesRegex(RuntimeError, "sequential"):
            self.request()


class StartupTests(unittest.TestCase):
    def test_split_markers_report_milliseconds_once_and_bound_retained_bytes(self):
        from tools.terminal_latency import Startup
        with tempfile.TemporaryDirectory() as temporary:
            values = iter([1_000_000, 3_000_000])
            startup = Startup(Path(temporary), {"welcome": b"ready"}, clock=lambda: next(values))
            startup.output(b"x" * 10000 + b"rea")
            startup.output(b"dy")
            startup.output(b"ready again")
            self.assertEqual(startup.observed, {"welcome": 2.0})
            self.assertLessEqual(len(startup.tail), 5)


class GeometryTests(unittest.TestCase):
    def test_initial_geometry_waits_for_pixels_instead_of_accepting_80_by_24(self):
        import struct
        import sys
        from types import SimpleNamespace
        from unittest.mock import patch
        dependencies = {"qualify_alacritty": module("qualify_alacritty")}
        with patch.dict(sys.modules, dependencies):
            relay = module("foot_terminal")
        provisional = struct.pack("HHHH", 24, 80, 0, 0)
        ready = struct.pack("HHHH", 40, 120, 1200, 800)
        with patch.object(relay.fcntl, "ioctl", side_effect=[provisional, ready]):
            with patch.object(relay.sys, "stdin", SimpleNamespace(fileno=lambda: 0)):
                with patch.object(relay.time, "sleep") as pause:
                    self.assertEqual(relay.initial_geometry(), ready)
                    pause.assert_called_once_with(0.01)
