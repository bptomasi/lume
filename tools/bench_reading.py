"""Measure checked reading services in representative and stress workloads."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import tempfile

from bench_support import TIMING, benchmark_binary, run_workload


def measure(binary, workload, stress, trial, resources):
    arguments = [f"--{workload}-only"]
    if stress:
        arguments.append("--stress")
    result = run_workload(binary, arguments, resources)
    timings = TIMING.findall(result.stdout)
    if len(timings) != 3:
        raise RuntimeError(f"Incomplete checked workload: {result.stdout}")
    cleanup = re.search(r"cache cleanup .* mean=([\d.]+) ms", result.stdout)
    return {
        "workload": workload,
        "stress": stress,
        "trial": trial,
        "timings": [
            {"operation": name, "iterations": int(count),
             "total_ms": float(total), "mean_ms": float(mean)}
            for name, count, total, mean in timings
        ],
        "cleanup_mean_ms": float(cleanup[1]) if cleanup else None,
        "resources": json.loads(resources.read_text()),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=Path(".local/benchmarks/reading.json"))
    parser.add_argument("--binary", type=Path)
    arguments = parser.parse_args()
    binary = arguments.binary or benchmark_binary()
    with tempfile.TemporaryDirectory(prefix="lume-reading-bench-") as name:
        resources = Path(name) / "resources.json"
        runs = [measure(binary, workload, stress, trial, resources)
                for workload in ("reading-services", "service-interaction")
                for stress in (False, True) for trial in range(1, 4)]
    report = {
        "measured_at": datetime.now(timezone.utc).isoformat(),
        "environment": platform.platform(),
        "cpu": next((line.split(":", 1)[1].strip()
                     for line in Path("/proc/cpuinfo").read_text().splitlines()
                     if line.startswith("model name")), "unreported"),
        "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "scope": [
            "Release profile; three fresh processes per workload and size.",
            "Plans: 100/512 scored metadata candidates; 30 total minutes.",
            "Offline: 10/100 cached full articles and a decoded pinned SVG.",
            "Cleanup: 10/100 Full caches; fixture/cache refill excluded.",
            "Selector: 200/2000 articles, 50 choices, input pair plus two frames.",
            "Linked pages: 50/500 sections, worker layout and delivery; no HTTP.",
            "Correctness checks verify bounds, identities, content and previews.",
            "Setup excluded from operation timers; resources include setup.",
            "No previous equivalent baseline or general speedup is claimed.",
            "TestBackend frames exclude terminal transport and display latency.",
        ],
        "runs": runs,
    }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(arguments.output)


if __name__ == "__main__":
    main()
