"""Shared startup qualification must wait for each driver's own article."""

import importlib
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
