"""The comparison refuses incomparable or pixel-changing candidate results."""
import copy
import importlib
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


def sample(strategy):
    return {"strategy": strategy, "reference_sha256": "reference",
            "decoded_sha256": "pixels",
            "samples": [{"wire_sha256": "wire"}, {"wire_sha256": "wire"}]}


class SixelProfile(unittest.TestCase):
    def setUp(self):
        tools = str(Path(__file__).resolve().parents[1])
        with patch.object(sys, "path", [tools, *sys.path]):
            self.driver = importlib.import_module("bench_sixel")

    def test_reference_equivalence_includes_wire_pixels_and_both_cache_passes(self):
        cases = [sample("reference"), sample("dedup"), sample("direct")]
        self.driver.qualify(cases)
        for location in ("reference_sha256", "decoded_sha256"):
            changed = copy.deepcopy(cases)
            changed[1][location] = "changed"
            with self.assertRaises(RuntimeError):
                self.driver.qualify(changed)
        changed = copy.deepcopy(cases)
        changed[1]["samples"][1]["wire_sha256"] = "changed"
        with self.assertRaises(RuntimeError):
            self.driver.qualify(changed)
        changed = copy.deepcopy(cases)
        for entry in changed[1]["samples"]:
            entry["wire_sha256"] = "changed stable wire"
        with self.assertRaises(RuntimeError):
            self.driver.qualify(changed)

    def test_alternative_quality_is_measured_without_requiring_reference_equality(self):
        cases = [sample("reference"), sample("dedup"), sample("direct")]
        cases[2]["decoded_sha256"] = "different pixels"
        for entry in cases[2]["samples"]:
            entry["wire_sha256"] = "different stable wire"
        self.driver.qualify(cases)


if __name__ == "__main__":
    unittest.main()
