"""Measure local recommendations at 10k/50k articles with checked Rust workloads."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import tempfile

from bench_support import TIMING, benchmark_binary, run_workload


LATENCY = re.compile(
    r"recommendation navigation latency: p50=([\d.]+) p95=([\d.]+) "
    r"p99=([\d.]+) ms"
)


def measure(binary, stress, trial, resources):
    arguments = ["--recommendations-only"]
    if stress:
        arguments.append("--stress")
    completed = run_workload(binary, arguments, resources)
    timings = [
        {"operation": name, "iterations": int(count), "total_ms": float(total),
         "ms_per_iteration": float(mean)}
        for name, count, total, mean in TIMING.findall(completed.stdout)
    ]
    latency = LATENCY.search(completed.stdout)
    if len(timings) != 5 or latency is None:
        raise AssertionError(f"Incomplete checked workloads: {completed.stdout}")
    return {
        "workload": "stress" if stress else "representative",
        "trial": trial,
        "articles": 50_000 if stress else 10_000,
        "sources": 10,
        **json.loads(resources.read_text()),
        "timings": timings,
        "navigation_latency_ms": dict(
            zip(("p50", "p95", "p99"), map(float, latency.groups()))
        ),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=Path(".local/benchmarks/recommendations.json"))
    baseline_options = parser.add_mutually_exclusive_group()
    baseline_options.add_argument("--baseline-binary", type=Path)
    baseline_options.add_argument("--baseline-report", type=Path)
    arguments = parser.parse_args()
    binary = benchmark_binary()
    results = []
    baseline = []
    with tempfile.TemporaryDirectory(prefix="lume-recommend-measure-") as name:
        if arguments.baseline_binary:
            for trial in range(1, 4):
                baseline.append(measure(arguments.baseline_binary.resolve(), False,
                                        trial, Path(name) / "resources.json"))
                print(f"baseline trial {trial}: {baseline[-1]['wall_seconds']} s",
                      flush=True)
        for stress in (False, True):
            for trial in range(1, 4):
                result = measure(binary, stress, trial, Path(name) / "resources.json")
                results.append(result)
                print(f"{result['workload']} trial {trial}: "
                      f"{result['wall_seconds']} s, {result['peak_rss_kib']} KiB",
                      flush=True)
    cpu = next((line.split(":", 1)[1].strip()
                for line in Path("/proc/cpuinfo").read_text().splitlines()
                if line.startswith("model name")), "unreported")
    report = {
        "command": ("python3 tools/bench_recommendations.py"
                    + (f" --baseline-binary {arguments.baseline_binary}"
                       if arguments.baseline_binary else "")),
        "environment": platform.platform(),
        "cpu": cpu,
        "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "scope": (
            "Release workloads, three runs per size. Setup excluded from operation "
            "timers, included in wall/RSS/CPU/I/O. Every workload checks correctness. "
            "Navigation is a down/up pair with synchronous selected-source loads "
            "and two TestBackend frames, not terminal-emulator or physical-input latency."
        ),
        "results": results,
        "baseline": {
            "scope": "Experimental pre-batch-optimization binary; same 10k fixture",
            "binary_sha256": (hashlib.sha256(arguments.baseline_binary.read_bytes()).hexdigest()
                              if arguments.baseline_binary else None),
            "results": baseline,
        },
    }
    if arguments.baseline_report:
        before = json.loads(arguments.baseline_report.read_text())
        if len(before["results"]) != 3:
            raise AssertionError("Baseline must contain three comparable runs")
        report["baseline"] = before
        report["command"] += f" --baseline-report {arguments.baseline_report}"
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
