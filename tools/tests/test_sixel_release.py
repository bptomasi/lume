"""External controls must reject source, wire, pixels and timing drift."""
import copy
import importlib
from pathlib import Path
from types import SimpleNamespace
import sys
import tempfile
import unittest
from unittest.mock import patch


class SixelReleaseComparison(unittest.TestCase):
    def setUp(self):
        tools = str(Path(__file__).resolve().parents[1])
        with patch.object(sys, "path", [tools, *sys.path]):
            self.driver = importlib.import_module("bench_sixel_release")
        self.sample = {
            "source_sha256": "source", "reference_sha256": "reference",
            "fitted_dimensions": [3, 2], "wire_bytes": 9, "strips": 1,
            "decoded_sha256": "pixels", "stages": ["decode", "encode"],
            "scenario": {"case": "photo"}, "backend": "production", "run": 0,
            "resources": {"peak_rss_kib": 100},
            "samples": [
                {"cache": "cold", "wire_sha256": "wire",
                 "timing": {"total_ns": 12000000, "stages_ns": [1, 6000000]}},
                {"cache": "warm", "wire_sha256": "wire",
                 "timing": {"total_ns": 10000000, "stages_ns": [0, 4000000]}},
            ],
        }

    def test_output_comparison_checks_every_control(self):
        self.driver.same_output(self.sample, copy.deepcopy(self.sample), decoded=True)
        for key in self.driver.IDENTITY + ("decoded_sha256",):
            candidate = copy.deepcopy(self.sample)
            candidate[key] = "changed"
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.driver.same_output(self.sample, candidate, decoded=True)
        candidate = copy.deepcopy(self.sample)
        candidate["samples"][0]["wire_sha256"] = "changed"
        with self.assertRaises(ValueError):
            self.driver.same_output(self.sample, candidate)

    def test_validation_requires_consumed_source_and_stable_warm_wire(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source"
            source.write_bytes(b"source bytes")
            valid = copy.deepcopy(self.sample)
            valid["source_sha256"] = self.driver.digest(source)
            self.driver.validate_sample(valid, source, 2)
            for key, value in (("source_sha256", "other"), ("samples", []),
                               ("fitted_dimensions", [0, 2]), ("strips", 0)):
                candidate = copy.deepcopy(valid)
                candidate[key] = value
                with self.subTest(key=key), self.assertRaises(ValueError):
                    self.driver.validate_sample(candidate, source, 2)
            candidate = copy.deepcopy(valid)
            candidate["samples"][1]["wire_sha256"] = "changed"
            with self.assertRaises(ValueError):
                self.driver.validate_sample(candidate, source, 2)
            candidate = copy.deepcopy(valid)
            candidate["samples"][1]["cache"] = "cold"
            with self.assertRaises(ValueError):
                self.driver.validate_sample(candidate, source, 2)

    def test_artifact_validation_reads_wire_instead_of_trusting_sample_hash(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            packet = directory / "strip-000.six"
            packet.write_bytes(b"\x1bPq?\x1b\\")
            sample = copy.deepcopy(self.sample)
            sample["wire_bytes"] = len(packet.read_bytes())
            sample["samples"][0]["wire_sha256"] = self.driver.digest(packet)
            self.driver.verify_wire(directory, sample)
            packet.write_bytes(b"\x1bPq~\x1b\\")
            with self.assertRaises(ValueError):
                self.driver.verify_wire(directory, sample)
            packet.write_bytes(b"\x1bPq?")
            with self.assertRaises(ValueError):
                self.driver.verify_wire(directory, sample)

    def test_natural_corpus_requires_pinned_original_hashes(self):
        fixtures = Path(__file__).resolve().parents[2] / "tests/fixtures/images"
        for case, (name, _) in self.driver.NATURAL.items():
            self.driver.verify_natural_source(case, fixtures / name)
        with tempfile.TemporaryDirectory() as directory:
            corrupt = Path(directory) / "camera.png"
            corrupt.write_bytes(b"different photograph")
            with self.assertRaisesRegex(ValueError, "Official source hash mismatch"):
                self.driver.verify_natural_source("camera", corrupt)

    def test_frozen_binary_command_does_not_use_new_flags(self):
        args = SimpleNamespace(width=120, height=36, repeats=20)
        scenario = dict(layout="inline", light=True, zoom=2)
        old = self.driver.render_command("old", "source", "out", scenario,
                                         args, "baseline")
        new = self.driver.render_command("new", "source", "out", scenario,
                                         args, "production")
        self.assertNotIn("--encoder", old)
        self.assertEqual(new[-4:], ["--encoder", "production", "--inline", "--light"])
        self.assertEqual(old[1:], new[1:-4] + ["--inline", "--light"])

    def test_pair_summary_uses_measured_total_and_separate_encode(self):
        candidate = copy.deepcopy(self.sample)
        candidate["samples"][1]["timing"] = {
            "total_ns": 8000000, "stages_ns": [0, 2000000],
            "encoder_scratch_bytes": 250,
        }
        row = self.driver.summarize_pair([self.sample], [candidate])
        self.assertAlmostEqual(row["warm_total"]["reduction_percent"], 20)
        self.assertAlmostEqual(row["warm_encode"]["reduction_percent"], 50)
        self.assertEqual(row["pairs"][0]["encoder_scratch_bytes"], [None, 250])
        with self.assertRaises(ValueError):
            self.driver.summarize_pair([self.sample], [candidate, candidate])

    def test_compact_evidence_preserves_raw_stage_and_total_timings(self):
        report = {"measurements": [self.sample], "summary": ["summary"]}
        result = self.driver.evidence(report)
        self.assertEqual(result["timing_columns"], ["cache", "total", "decode", "encode"])
        self.assertEqual(result["observations"][0]["timings"],
                         [["cold", 12000000, 1, 6000000],
                          ["warm", 10000000, 0, 4000000]])
        self.assertEqual(result["observations"][0]["wire_sha256"], "wire")
        changed = copy.deepcopy(self.sample)
        changed["stages"].reverse()
        report["measurements"].append(changed)
        with self.assertRaises(ValueError):
            self.driver.evidence(report)

    def test_scenarios_keep_mattes_zoom_and_distinct_natural_sources(self):
        cases = self.driver.scenarios(self.driver.CASES, ("native", "inline"))
        self.assertEqual(len(cases), 32)
        self.assertEqual(len({self.driver.label(item) for item in cases}), len(cases))
        self.assertEqual(sum(item["light"] for item in cases), 4)
        self.assertEqual(sum(item["zoom"] == 2 for item in cases), 2)
        for name in ("camera", "chelsea", "coffee", "photo"):
            self.assertTrue(any(item["case"] == name for item in cases))


if __name__ == "__main__":
    unittest.main()
