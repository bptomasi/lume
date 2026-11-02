"""Compare matched reader benchmarks through App/worker image delivery.

Use the same bench-only fixture patch on both revisions. No build runs here.
RSS/CPU cover the complete process; delivery excludes terminal presentation.
"""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess

from bench_sixel_release import digest, reduction, verify_natural_source


def profiles(stdout):
    result = [json.loads(line.removeprefix("production-profile: "))
              for line in stdout.splitlines()
              if line.startswith("production-profile: ")]
    if len(result) != 3:
        raise ValueError("Expected three checked App inspections")
    for item in result:
        if (item["inspection_delivery_ms"] <= 0
                or item["native_wire_bytes"] <= 0
                or len(item["native_wire_sha256"]) != 64):
            raise ValueError("Incomplete App delivery or native wire")
    return result


def qualify(old, new, expected):
    for sample in [*old, *new]:
        if sample["input_sha256"] != expected:
            raise ValueError("App used an unexpected input")
    identities = {(item["native_wire_sha256"], item["native_wire_bytes"])
                  for item in [*old, *new]}
    if len(identities) != 1:
        raise ValueError("App changed delivered Sixel wire")


def measure(binary, directory, args):
    directory.mkdir(parents=True)
    environment = dict(os.environ)
    environment.pop("LUME_BENCH_SIXEL_INPUT", None)
    if args.input:
        environment["LUME_BENCH_SIXEL_INPUT"] = str(args.input)
    resources = directory / "resources.json"
    command = ["/usr/bin/time", "-f",
               '{"peak_rss_kib":%M,"user_seconds":%U,"system_seconds":%S}',
               "-o", str(resources), str(binary), "--sixel-production-only"]
    if args.cpu is not None:
        command = ["taskset", "-c", str(args.cpu), *command]
    completed = subprocess.run(command, env=environment, text=True,
                               capture_output=True)
    (directory / "stdout.log").write_text(completed.stdout)
    (directory / "stderr.log").write_text(completed.stderr)
    if completed.returncode:
        raise RuntimeError(f'App qualification failed: {directory}: {completed.stderr}')
    return {"samples": profiles(completed.stdout),
            "resources": json.loads(resources.read_text())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--input", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, choices=range(1, 11), default=3)
    parser.add_argument("--cpu", type=int)
    args = parser.parse_args()
    args.baseline = args.baseline.resolve(strict=True)
    args.candidate = args.candidate.resolve(strict=True)
    args.input = args.input.resolve(strict=True) if args.input else None
    if args.cpu is not None and args.cpu not in os.sched_getaffinity(0):
        parser.error("Requested CPU is outside permitted affinity")
    source = args.input or Path(__file__).resolve().parents[1] / "tests/fixtures/images/astronaut.png"
    if args.input is None:
        verify_natural_source("photo", source)
    expected = digest(source)
    hashes = {name: digest(getattr(args, name)) for name in ("baseline", "candidate")}
    runs = []
    for run in range(args.runs):
        order = ["baseline", "candidate"] if run % 2 == 0 else ["candidate", "baseline"]
        pair = {}
        for name in order:
            pair[name] = measure(getattr(args, name), args.output / f'{run}-{name}', args)
        qualify(pair["baseline"]["samples"], pair["candidate"]["samples"], expected)
        medians = {name: statistics.median(item["inspection_delivery_ms"]
                                         for item in pair[name]["samples"])
                   for name in ("baseline", "candidate")}
        pair["delivery_ms"] = medians
        pair["reduction_percent"] = reduction(medians["baseline"], medians["candidate"])
        runs.append(pair)
    qualify([sample for pair in runs for sample in pair["baseline"]["samples"]],
            [sample for pair in runs for sample in pair["candidate"]["samples"]], expected)
    if any(digest(getattr(args, name)) != value for name, value in hashes.items()):
        raise ValueError("App executable changed during qualification")
    report = {"input_sha256": expected, "input_bytes": source.stat().st_size,
              "binaries_sha256": hashes, "cpu": args.cpu,
              "runs": runs, "median_paired_reduction_percent":
              statistics.median(item["reduction_percent"] for item in runs)}
    (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(args.output / "report.json")


if __name__ == "__main__":
    main()
