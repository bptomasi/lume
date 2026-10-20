"""Repeated, correctness-checked long-row frames and process resource evidence."""

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


def measure(binary, stress, index, directory):
    arguments = ["--code-viewport-only"] + (["--stress"] if stress else [])
    resources = directory / f"resources-{stress}-{index}.json"
    result = run_workload(binary, arguments, resources)
    timings = [match for line in result.stdout.splitlines()
               if (match := TIMING.fullmatch(line))]
    if len(timings) != 1:
        raise RuntimeError("Long-row behavioral benchmark did not report its timing")
    match = timings[0]
    return {"stress": stress, "run": index, "iterations": int(match[2]),
            "mean_ms": float(match[3]) / int(match[2]),
            "resources": json.loads(resources.read_text()), "stdout": result.stdout}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--baseline-binary", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    binary = arguments.binary or benchmark_binary()
    binaries = {"current": binary}
    if arguments.baseline_binary:
        binaries = {"baseline": arguments.baseline_binary, **binaries}
    report = {
        "measured_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(),
        "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "scope": (
            "Two cursor motions, a selected full interactive frame, and exact "
            "source/tail assertions. Source layout precedes timers; process "
            "resources include setup."
        ),
        "binaries": {},
    }
    with tempfile.TemporaryDirectory(prefix="lume-code-") as temporary:
        for name, executable in binaries.items():
            runs = [measure(executable, stress, index, Path(temporary))
                    for stress in (False, True) for index in range(3)]
            report["binaries"][name] = {
                "sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
                "runs": runs,
                "median_mean_ms": {
                    str(stress): statistics.median(
                        run["mean_ms"] for run in runs
                        if run["stress"] == stress
                    )
                    for stress in (False, True)
                },
            }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(arguments.output)


if __name__ == "__main__":
    main()
