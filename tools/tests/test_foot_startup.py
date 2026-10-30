"""Foot qualification checks source readiness and explicit popup footprints."""

import importlib
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest
from unittest.mock import patch


class FootStartupTests(unittest.TestCase):
    def setUp(self):
        tools = str(Path(__file__).resolve().parents[1])
        with patch.object(sys, "path", [tools, *sys.path]):
            self.driver = importlib.import_module("qualify_foot")

    def exercise(self, expected, displayed):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            output = directory / "terminal-output.bin"
            output.write_bytes("▀ Press any key to begin".encode())

            def send(_directory, payload):
                self.assertEqual(payload, b"q")
                with output.open("ab") as capture:
                    capture.write(b"Inbox\x1b[3;5H" + displayed)

            def wait(_description, condition, _process):
                self.assertTrue(condition(), _description)

            with patch.object(self.driver, "send", side_effect=send):
                with patch.object(self.driver, "wait_for", side_effect=wait):
                    return self.driver.enter_library(directory, None, expected)

    def test_each_fixture_accepts_its_own_fragmented_ready_source(self):
        for expected, displayed in [
            ("Request ownership", b"Request\x1b[1Cownership"),
            ("ALPHA HEADMARK @@@@@@@@@@@@",
             b"ALPHA\x1b[2CHEADMARK\x1b[1C@@@@@@@@@@@@"),
        ]:
            with self.subTest(expected=expected):
                result = self.exercise(expected, displayed)
                self.assertTrue(result["entry_key_consumed"])

    def test_unrelated_article_does_not_qualify_startup(self):
        with self.assertRaisesRegex(AssertionError, "prepared article"):
            self.exercise("ALPHA HEADMARK", b"Request ownership")

    def test_partial_popups_allow_uncovered_pixels_only_when_requested(self):
        current = {"offset": 10, "strips": [{"row": 35}],
                   "content": b"Set\x1b[4;7Htings"}
        with patch.object(self.driver, "frame", return_value=current):
            self.assertIsNone(self.driver.service_frame(None, 9, b"Settings"))
            self.assertIs(
                self.driver.service_frame(None, 9, b"Settings", allow_native=True),
                current,
            )
            self.assertIsNone(
                self.driver.service_frame(None, 10, b"Settings", allow_native=True),
            )
            self.assertIsNone(
                self.driver.service_frame(None, 9, b"Wrong popup", allow_native=True),
            )

    def test_linked_pixels_must_fit_between_header_and_controls(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "terminal.json").write_text(json.dumps({
                "rows": 40, "columns": 120, "cell_pixels": [10, 20],
            }))
            valid = {"row": 3, "column": 2, "width": 1160, "height": 20}
            self.driver.verify_linked_bounds(directory, {"strips": [valid]})
            for changes in ({"row": 2}, {"row": 37}, {"column": 1},
                            {"width": 1161}, {"height": 21}):
                with self.subTest(changes=changes):
                    with self.assertRaisesRegex(RuntimeError, "overlaps"):
                        self.driver.verify_linked_bounds(
                            directory, {"strips": [valid | changes]},
                        )

    def test_sixel_origin_includes_painted_margins_but_not_erase_cells(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for prefix in (b"\x1b[16;21H\x1b[;m  \x1b[76X",
                           "\x1b[16;1H│\x1b[;m                     \x1b[76X".encode()):
                wire = (self.driver.BEGIN + prefix
                        + b'\x1bPq"1;1;760;19\x1b\\' + self.driver.END)
                (directory / "terminal-output.bin").write_bytes(wire)
                strip = self.driver.frame(directory)["strips"][0]
                self.assertEqual((strip["row"], strip["column"]), (15, 22))

    def test_native_transitions_require_clear_before_the_first_pixel_payload(self):
        pixels = b'\x1bPq"1;1;20;20!20~\x1b\\'
        self.driver.verify_native_transition(b"\x1b[2J" + pixels)
        for wire in (pixels, pixels + b"\x1b[2J", b"\x1b[2J"):
            with self.subTest(wire=wire):
                with self.assertRaisesRegex(RuntimeError, "stale pixels"):
                    self.driver.verify_native_transition(wire)

    def test_sixel_origin_follows_a_painted_sidebar_row(self):
        # Reproduce v0.2.1 CI: a complete FEEDS row precedes the reader strip.
        prefix = ("\x1b[21;1H│▾ FEEDS             1││"
                  + " " * 34 + "││ ").encode()
        content = prefix + b"\x1b[;m\x1b[57X"
        cursor = re.search(rb"\x1b\[(\d+);(\d+)H", content)
        self.assertEqual(
            self.driver.strip_position(content, cursor, len(content)),
            (20, 61),
        )

    def test_unsupported_cursor_motion_and_text_widths_fail_closed(self):
        for suffix in (b"\x1b[5C", b"\t", "界".encode(), "a\u0301".encode()):
            with self.subTest(suffix=suffix):
                content = b"\x1b[21;1H" + suffix
                cursor = re.search(rb"\x1b\[(\d+);(\d+)H", content)
                with self.assertRaisesRegex(RuntimeError, "Unexpected content"):
                    self.driver.strip_position(content, cursor, len(content))


if __name__ == "__main__":
    unittest.main()
