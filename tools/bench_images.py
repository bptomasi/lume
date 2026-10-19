"""Measure checked image preparation and cached scrolling in release mode."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import platform
from pathlib import Path
import subprocess
import tempfile

from bench_support import TIMING, benchmark_binary, run_workload


def measure(binary, stress, index, directory):
    arguments = ["--images-only"]
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
                "mean_ms": float(match[4]),
            }
    if len(metrics) != 12:
        raise RuntimeError("Expected all twelve checked image workloads")
    return {
        "stress": stress,
        "run": index,
        "metrics": metrics,
        "resources": json.loads(resources.read_text()),
        "stdout": completed.stdout,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=Path("docs/image-inspection-benchmarks.json"))
    arguments = parser.parse_args()
    binary = benchmark_binary()
    report = {
        "measured_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(),
        "rust": subprocess.check_output(["rustc", "--version"], text=True)
        .strip(),
        "binary": binary.name,
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "notes": [
            "Three repeated processes per representative/stress workload.",
            "Setup and fallback font initialization are outside operation timers.",
            "Process resources include fixture generation and setup.",
            "Frames use TestBackend; physical terminal latency is not measured.",
            "Historical inline-only measurements remain in image-benchmarks.json.",
            "Native encoders are measured; physical emulator/display time is not.",
            "Inspector zoom and pan use original bytes or re-rendered SVG paths.",
            "Overlay PNG preparation and UI projection planning are measured.",
            "Bidirectional scrolling checks 24 clips without new image work.",
            "External helper/GPU display time is not part of frame timings.",
        ],
        "runs": [],
    }
    with tempfile.TemporaryDirectory(prefix="lume-image-bench-") as temporary:
        directory = Path(temporary)
        for stress in (False, True):
            for index in range(1, 4):
                report["runs"].append(measure(binary, stress, index, directory))
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(arguments.output)


if __name__ == "__main__":
    main()
