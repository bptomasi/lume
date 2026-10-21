"""Measure checked note operations and large notebooks with process resources."""

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
    r"note context/frame latency: p50=([\d.]+) p95=([\d.]+) "
    r"p99=([\d.]+) ms"
)


def measure(binary, workload, stress, trial, resources):
    arguments = [f"--{workload}-only"]
    if stress:
        arguments.append("--stress")
    completed = run_workload(binary, arguments, resources)
    timings = [
        {
            "operation": operation,
            "iterations": int(iterations),
            "total_ms": float(total),
            "ms_per_iteration": float(per_iteration),
        }
        for operation, iterations, total, per_iteration in
        TIMING.findall(completed.stdout)
    ]
    expected = 4 if workload == "notes" else 6
    if len(timings) != expected:
        raise AssertionError(f"Expected {expected} checked workloads: {completed.stdout}")
    result = {
        "suite": workload,
        "workload": "stress" if stress else "representative",
        "trial": trial,
        **json.loads(resources.read_text()),
        "timings": timings,
    }
    latency = LATENCY.search(completed.stdout)
    if workload == "notebook":
        if latency is None:
            raise AssertionError("Notebook latency percentiles were not reported")
        result["annotations"] = 100_000 if stress else 10_000
        result["articles"] = 1000 if stress else 100
        result["context_frame_latency_ms"] = dict(
            zip(("p50", "p95", "p99"), map(float, latency.groups()))
        )
    else:
        source = re.search(
            r"note input: (\d+) source bytes, (\d+) annotations",
            completed.stdout,
        )
        if source is None:
            raise AssertionError("Annotation input size was not reported")
        result["source_bytes"], result["annotations"] = map(int, source.groups())
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path(".local/benchmarks/notebook.json"))
    arguments = parser.parse_args()
    binary = benchmark_binary()
    results = []
    with tempfile.TemporaryDirectory(prefix="lume-notebook-measure-") as name:
        resources = Path(name) / "resources.json"
        for workload in ("notes", "notebook"):
            for stress in (False, True):
                for trial in range(1, 4):
                    result = measure(binary, workload, stress, trial, resources)
                    results.append(result)
                    print(f"{workload} {result['workload']} trial {trial}: "
                          f"{result['wall_seconds']} s, {result['peak_rss_kib']} KiB")
    cpu = next((
        line.split(":", 1)[1].strip()
        for line in Path("/proc/cpuinfo").read_text().splitlines()
        if line.startswith("model name")
    ), "unreported")
    report = {
        "command": "python3 tools/bench_notes.py",
        "environment": platform.platform(),
        "cpu": cpu,
        "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "profile": "Optimized release, thin LTO, one codegen unit",
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "timing_scope": (
            "Setup excluded from operation timers; process wall/RSS/CPU/I/O "
            "include fixtures and all suite operations. Each operation checks "
            "correctness. Context/frame percentiles use TestBackend, not an "
            "actual terminal emulator; polling uses yield_now."
        ),
        "baseline": None,
        "results": results,
    }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
