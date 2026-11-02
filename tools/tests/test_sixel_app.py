"""App controls reject missing deliveries, substituted inputs and native drift."""
import copy
import importlib
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


class SixelAppComparison(unittest.TestCase):
    def setUp(self):
        tools = str(Path(__file__).resolve().parents[1])
        with patch.object(sys, "path", [tools, *sys.path]):
            self.driver = importlib.import_module("bench_sixel_app")
        self.profile = {"input_sha256": "source", "native_wire_bytes": 123,
                        "native_wire_sha256": "a" * 64,
                        "inspection_delivery_ms": 10.0}

    def test_parser_requires_three_completed_positive_deliveries(self):
        line = "production-profile: " + json.dumps(self.profile) + "\n"
        self.assertEqual(len(self.driver.profiles("unrelated\n" + line * 3)), 3)
        with self.assertRaises(ValueError):
            self.driver.profiles(line * 2)
        self.profile["inspection_delivery_ms"] = 0
        line = "production-profile: " + json.dumps(self.profile) + "\n"
        with self.assertRaises(ValueError):
            self.driver.profiles(line * 3)

    def test_qualification_checks_source_and_each_native_result(self):
        old = [copy.deepcopy(self.profile) for _ in range(3)]
        new = copy.deepcopy(old)
        self.driver.qualify(old, new, "source")
        for key, value in (("input_sha256", "other"),
                           ("native_wire_bytes", 124),
                           ("native_wire_sha256", "b" * 64)):
            changed = copy.deepcopy(new)
            changed[2][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.driver.qualify(old, changed, "source")


if __name__ == "__main__":
    unittest.main()
