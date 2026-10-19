"""Shared release build and process-resource capture for checked Rust workloads."""

from pathlib import Path
import re
import subprocess


TIMING = re.compile(
    r"(.+): (\d+) iterations, ([\d.]+) ms total, ([\d.]+) ms/iteration"
)


def benchmark_binary():
    completed = subprocess.run(
        ["cargo", "bench", "--locked", "--bench", "reader", "--no-run"],
        check=True, capture_output=True, text=True,
    )
    matches = re.findall(
        r"Executable benches/reader.rs \(([^)]+)\)",
        completed.stdout + completed.stderr,
    )
    if not matches:
        raise RuntimeError("Cargo did not report the compiled benchmark path")
    return Path(matches[-1]).resolve()


def run_workload(binary, arguments, resources):
    """Run one checked workload; callers own its scenario and output contract."""
    command = [
        "/usr/bin/time", "-f",
        '{"wall_seconds":%e,"peak_rss_kib":%M,"user_seconds":%U,'
        '"system_seconds":%S,"input_blocks":%I,"output_blocks":%O}',
        "-o", str(resources), str(binary), *arguments,
    ]
    completed = subprocess.run(command, capture_output=True, text=True)
    if completed.returncode:
        raise RuntimeError(
            f"Checked workload failed ({completed.returncode}): "
            f"{completed.stdout}\n{completed.stderr}"
        )
    return completed
