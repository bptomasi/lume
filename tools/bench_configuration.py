"""Measure checked TOML publication and idle revision queries at scale."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile

from bench_support import TIMING, benchmark_binary, run_workload


def measure(binary, stress, trial, resources):
    arguments = ["--configuration-only"]
    if stress:
        arguments.append("--stress")
    result = run_workload(binary, arguments, resources)
    timings = TIMING.findall(result.stdout)
    if len(timings) != 2:
        raise RuntimeError(f"Configuration workload did not report: {result.stdout}")
    operation, iterations, total, mean = timings[0]
    idle = timings[1]
    return {
        "feeds": 5000 if stress else 250,
        "smart_lists": 256 if stress else 32,
        "trial": trial,
        "operation": operation,
        "iterations": int(iterations),
        "total_ms": float(total),
        "mean_ms": float(mean),
        "revision_checks_ms": float(idle[3]) / 200,
        "resources": json.loads(resources.read_text()),
    }


def compare(baseline, current):
    comparisons = []
    for feeds in (250, 5000):
        for metric in ("mean_ms", "revision_checks_ms"):
            before = statistics.median(run[metric] for run in baseline
                                       if run["feeds"] == feeds)
            after = statistics.median(run[metric] for run in current
                                      if run["feeds"] == feeds)
            comparisons.append({"feeds": feeds, "metric": metric,
                                "baseline_ms": before, "current_ms": after,
                                "ratio": before / after})
    return comparisons


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=Path(".local/benchmarks/configuration.json"))
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--baseline", type=Path)
    arguments = parser.parse_args()
    binary = arguments.binary or benchmark_binary()
    with tempfile.TemporaryDirectory(prefix="lume-config-bench-") as name:
        resources = Path(name) / "resources.json"
        runs = [measure(binary, stress, trial, resources)
                for stress in (False, True) for trial in range(1, 4)]
    report = {
        "measured_at": datetime.now(timezone.utc).isoformat(),
        "environment": platform.platform(),
        "cpu": next((line.split(":", 1)[1].strip()
                     for line in Path("/proc/cpuinfo").read_text().splitlines()
                     if line.startswith("model name")), "unreported"),
        "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "scope": (
            "Each iteration renames a folder, toggles a source and durably publishes "
            "TOML, then checks reread identities, state, queries and retained comments. "
            "Revision checks use the exact operational SQL and row mapping, "
            "verify pending/published state, then time 200 checks per iteration. "
            "Setup excluded from operation timers; process resources include setup. "
            "No earlier equivalent baseline or speedup comparison is claimed."
        ),
        "median_mean_ms": {
            str(feeds): statistics.median(run["mean_ms"] for run in runs
                                         if run["feeds"] == feeds)
            for feeds in (250, 5000)
        },
        "runs": runs,
    }
    if arguments.baseline:
        baseline = json.loads(arguments.baseline.read_text())
        if (baseline["environment"], baseline["cpu"], baseline["rust"]) != (
            report["environment"], report["cpu"], report["rust"],
        ):
            raise ValueError("Baseline environment/toolchain does not match")
        report["baseline"] = baseline
        report["comparison"] = compare(baseline["runs"], runs)
        report["scope"] = report["scope"].replace(
            "No earlier equivalent baseline or speedup comparison is claimed.",
            "Compared against the same checked workloads before the review edits.",
        )
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(arguments.output)


if __name__ == "__main__":
    main()
