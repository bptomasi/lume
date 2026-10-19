"""Measure cached full-frame interaction and Unicode cursor work in release mode."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import tempfile

from bench_support import TIMING, benchmark_binary, run_workload


def measure(binary, stress, index, directory):
    arguments = ["--interaction-only"]
    if stress:
        arguments.append("--stress")
    resources = directory / f"resources-{stress}-{index}.json"
    completed = run_workload(binary, arguments, resources)
    metrics = {}
    for line in completed.stdout.splitlines():
        match = TIMING.fullmatch(line)
        if match:
            metrics[match[1]] = {
                "iterations": int(match[2]),
                "total_ms": float(match[3]),
                "mean_ms": float(match[3]) / int(match[2]),
            }
    if len(metrics) != 3:
        raise RuntimeError("Expected all three checked interaction workloads")
    return {"stress": stress, "run": index, "metrics": metrics,
            "resources": json.loads(resources.read_text()),
            "stdout": completed.stdout}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    binary = benchmark_binary()
    report = {
        "measured_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(),
        "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "notes": [
            "Three repeated processes per representative/stress workload.",
            "Setup, source layout and syntax completion precede operation timers.",
            "The sidebar uses projected metadata; no database query is timed there.",
            "Selection uses actual reader motion, workspace resize and cached frames.",
            "App-side elapsed work is measured, not physical terminal paint latency.",
            "Process resources include fixture setup and all three workloads.",
            "Means use total/iterations to retain sub-microsecond measurements.",
        ],
        "runs": [],
    }
    with tempfile.TemporaryDirectory(prefix="lume-interaction-") as temporary:
        for stress in (False, True):
            for index in range(1, 4):
                report["runs"].append(measure(binary, stress, index, Path(temporary)))
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(arguments.output)


if __name__ == "__main__":
    main()
