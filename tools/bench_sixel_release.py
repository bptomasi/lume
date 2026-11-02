"""Compare frozen and candidate encoders on identical exported source bytes.

Processes run serially in alternating order. Each process has one cold render
and nineteen warm renders by default. Decoder.render timings exclude file I/O,
metrics and terminal presentation; process RSS/CPU include fixture setup/export.
Independent ImageMagick decoding runs outside the timed process, once per
scenario/backend; all further runs must retain the same wire and fitted hashes.
An additional 119x35-cell gradient exercises partial six-row bands and a
partial final inline cell; its explicit geometry is recorded in the scenario.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tomllib

from bench_sixel import decode, low_frequency_quality
from visual_dependencies import pillow

ROOT = Path(__file__).resolve().parents[1]
CASES = ("diagram", "antialias", "gradient", "photo", "entropy", "transparent",
         "camera", "chelsea", "coffee", "ui", "dense", "rgba16")
ENCODERS = ("dense", "sparse", "production")
NATURAL = {
    "photo": ("astronaut.png", "88431cd9653ccd539741b555fb0a46b61558b301d4110412b5bc28b5e3ea6cb5"),
    "camera": ("camera.png", "b0793d2adda0fa6ae899c03989482bff9a42d3d5690fc7e3648f2795d730c23a"),
    "chelsea": ("chelsea.png", "596aa1e7cb875eb79f437e310381d26b338a81c2da23439704a73c4651e8c4bb"),
    "coffee": ("coffee.png", "cc02f8ca188b167c775a7101b5d767d1e71792cf762c33d6fa15a4599b5a8de7"),
}
IDENTITY = ("source_sha256", "reference_sha256", "fitted_dimensions",
            "wire_bytes", "strips")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_natural_source(case, source):
    expected = NATURAL.get(case)
    if expected is not None and digest(source) != expected[1]:
        raise ValueError(f'Official source hash mismatch: {case}: {source}')


def scenarios(cases, layouts):
    result = []
    for case in cases:
        for layout in layouts:
            result.append(dict(case=case, layout=layout, light=False, zoom=1))
    for case in ("transparent", "rgba16"):
        if case in cases:
            for layout in layouts:
                result.append(dict(case=case, layout=layout, light=True, zoom=1))
    if "photo" in cases:
        for layout in layouts:
            result.append(dict(case="photo", layout=layout, light=False, zoom=2))
    if "gradient" in cases:
        for layout in layouts:
            result.append(dict(case="gradient", layout=layout, light=False,
                               zoom=1, width=119, height=35))
    return result


def label(scenario):
    name = (f'{scenario["case"]}-{scenario["layout"]}-'
            f'{"light" if scenario["light"] else "dark"}-z{scenario["zoom"]}')
    if "width" in scenario:
        name += f'-{scenario["width"]}x{scenario["height"]}'
    return name


def render_command(binary, source, directory, scenario, args, encoder):
    command = [str(binary), "--input", str(source), "--strategy", "dedup",
               "--width", str(scenario.get("width", args.width)),
               "--height", str(scenario.get("height", args.height)),
               "--repeats", str(args.repeats), "--zoom", str(scenario["zoom"]),
               "--output", str(directory)]
    if encoder != "baseline":
        command.extend(["--encoder", encoder])
    if scenario["layout"] == "inline":
        command.append("--inline")
    if scenario["light"]:
        command.append("--light")
    return command


def measure(binary, source, directory, scenario, args, encoder):
    directory.mkdir(parents=True)
    resources = directory / "resources.json"
    command = ["/usr/bin/time", "-f",
               '{"wall_seconds":%e,"peak_rss_kib":%M,"user_seconds":%U,'
               '"system_seconds":%S}', "-o", str(resources),
               *render_command(binary, source, directory, scenario, args, encoder)]
    if args.cpu is not None:
        command = ["taskset", "-c", str(args.cpu), *command]
    completed = subprocess.run(command, text=True, capture_output=True)
    (directory / "stderr.log").write_text(completed.stderr)
    if completed.returncode:
        raise RuntimeError(f'Render failed: {directory}; {completed.stderr}')
    sample = json.loads((directory / "sample.json").read_text())
    sample.update(resources=json.loads(resources.read_text()),
                  artifacts=str(directory), scenario=scenario, backend=encoder)
    validate_sample(sample, source, args.repeats)
    verify_artifacts(directory, sample)
    if encoder != "baseline":
        for entry in sample["samples"]:
            calls = entry["timing"].get("encoder_calls", [])
            if len(calls) != 2 or sum(calls) != sample["strips"]:
                raise ValueError("Candidate did not record its actual encoder dispatch")
    return sample


def validate_sample(sample, source, repeats):
    if sample["source_sha256"] != digest(source):
        raise ValueError("Renderer did not consume the exported source")
    if len(sample["samples"]) != repeats:
        raise ValueError("Renderer lost timing samples")
    caches = [entry["cache"] for entry in sample["samples"]]
    if caches != ["cold", *(["warm"] * (repeats - 1))]:
        raise ValueError("Unexpected cold/warm ordering")
    if len({entry["wire_sha256"] for entry in sample["samples"]}) != 1:
        raise ValueError("Cold/warm changed emitted wire")
    if len(sample["fitted_dimensions"]) != 2 or not all(sample["fitted_dimensions"]):
        raise ValueError("Missing fitted image dimensions")
    if sample["wire_bytes"] <= 0 or sample["strips"] <= 0:
        raise ValueError("Empty Sixel output")
    for entry in sample["samples"]:
        timing = entry["timing"]
        if (timing["total_ns"] <= 0
                or len(timing["stages_ns"]) != len(sample["stages"])):
            raise ValueError("Incomplete stage timing")



def verify_wire(directory, sample):
    wire = hashlib.sha256()
    length = 0
    for index in range(sample["strips"]):
        packet = (directory / f"strip-{index:03}.six").read_bytes()
        if not packet.startswith(b"\x1bP") or not packet.endswith(b"\x1b\\"):
            raise ValueError("Incomplete Sixel frame")
        wire.update(packet)
        length += len(packet)
    if (length != sample["wire_bytes"]
            or wire.hexdigest() != sample["samples"][0]["wire_sha256"]):
        raise ValueError("Exported wire disagrees with recorded hash/size")


def verify_artifacts(directory, sample):
    verify_wire(directory, sample)
    with pillow().open(directory / "reference.png") as image:
        fitted = image.convert("RGB")
        if (list(fitted.size) != sample["fitted_dimensions"]
                or hashlib.sha256(fitted.tobytes()).hexdigest()
                != sample["reference_sha256"]):
            raise ValueError("Exported fitted image disagrees with recorded pixels")


def same_output(baseline, candidate, decoded=False):
    for name in IDENTITY:
        if baseline[name] != candidate[name]:
            raise ValueError(f'Output changed: {name}')
    if (baseline["samples"][0]["wire_sha256"]
            != candidate["samples"][0]["wire_sha256"]):
        raise ValueError("Candidate changed Sixel wire")
    if decoded and baseline["decoded_sha256"] != candidate["decoded_sha256"]:
        raise ValueError("Candidate changed independently decoded pixels")


def qualify(binary, sample):
    directory = Path(sample["artifacts"])
    sample["decoded_sha256"] = decode(directory, sample)
    measured = subprocess.check_output(
        [str(binary), "--compare", str(directory / "reference.png"),
         "--actual", str(directory / "decoded.png")], text=True)
    sample["quality"] = json.loads(measured)
    sample["low_frequency_quality"] = low_frequency_quality(binary, directory)


def milliseconds(sample, cache, stage=None):
    index = sample["stages"].index(stage) if stage else None
    return [entry["timing"]["total_ns"] / 1e6 if index is None else
            entry["timing"]["stages_ns"][index] / 1e6
            for entry in sample["samples"] if entry["cache"] == cache]


def reduction(old, new):
    if old <= 0:
        raise ValueError("Cannot compare zero-duration stage")
    return (1 - new / old) * 100


def summarize_pair(baselines, candidates):
    row = {"scenario": candidates[0]["scenario"],
           "backend": candidates[0]["backend"],
           "wire_bytes": candidates[0]["wire_bytes"], "pairs": []}
    for old, new in zip(baselines, candidates, strict=True):
        pair = {"run": old["run"]}
        for cache in ("cold", "warm"):
            for stage in (None, "encode"):
                name = f'{cache}_{stage or "total"}'
                previous = milliseconds(old, cache, stage)
                current = milliseconds(new, cache, stage)
                left, right = statistics.median(previous), statistics.median(current)
                pair[name] = {"baseline_ms": left, "candidate_ms": right,
                              "reduction_percent": reduction(left, right),
                              "baseline_range_ms": [min(previous), max(previous)],
                              "candidate_range_ms": [min(current), max(current)]}
        pair["resources"] = {"baseline": old["resources"],
                             "candidate": new["resources"]}
        pair["encoder_dispatch"] = [entry["timing"].get("encoder_calls")
                                    for entry in new["samples"]]
        pair["encoder_scratch_bytes"] = [
            entry["timing"].get("encoder_scratch_bytes")
            for entry in new["samples"]]
        row["pairs"].append(pair)
    for metric in ("warm_total", "warm_encode", "cold_total", "cold_encode"):
        row[metric] = {key: statistics.median(pair[metric][key]
                                            for pair in row["pairs"])
                       for key in ("baseline_ms", "candidate_ms", "reduction_percent")}
    return row


def execute(args):
    args.output.mkdir(parents=True, exist_ok=True)
    binaries = {"baseline": args.baseline, **{name: args.candidate
                                            for name in args.encoders}}
    hashes = {name: digest(binary) for name, binary in binaries.items()}
    sources = {}
    for case in args.cases:
        if case in NATURAL:
            verify_natural_source(case, ROOT / "tests/fixtures/images" / NATURAL[case][0])
        directory = args.output / "sources" / case
        subprocess.run([str(args.candidate), "--case", case, "--output",
                        str(directory), "--export-source-only"], check=True)
        sources[case] = directory / "source.img"
        verify_natural_source(case, sources[case])
    measurements = []
    for scenario in scenarios(args.cases, args.layouts):
        previous = {}
        for run in range(args.runs):
            names = list(binaries)
            if run % 2:
                names.reverse()
            current = {}
            for name in names:
                directory = args.output / label(scenario) / f'{run}-{name}'
                sample = measure(binaries[name], sources[scenario["case"]],
                                 directory, scenario, args, name)
                sample["run"] = run
                if run == 0:
                    qualify(binaries[name], sample)
                else:
                    same_output(previous[name], sample)
                current[name] = sample
            for name in args.encoders:
                same_output(current["baseline"], current[name], decoded=run == 0)
            if run == 0:
                previous = current
            measurements.extend(current.values())
        print(f'Qualified {label(scenario)}', flush=True)
    if any(digest(binary) != hashes[name] for name, binary in binaries.items()):
        raise RuntimeError("Executable changed during comparison")
    return measurements, hashes, sources


def source_state():
    status = subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=ROOT, text=True)
    # Record changed and untracked source files without embedding their content.
    paths = subprocess.check_output(
        ["git", "diff", "--name-only", "-z", "HEAD"], cwd=ROOT)
    paths += subprocess.check_output(
        ["git", "ls-files", "-o", "--exclude-standard", "-z"], cwd=ROOT)
    changed = {}
    for raw in paths.split(b"\0"):
        if raw:
            path = ROOT / os.fsdecode(raw)
            if path.is_file():
                changed[str(path.relative_to(ROOT))] = digest(path)
    return {"status": status, "changed_files_sha256": changed}


def cpu_model():
    information = Path("/proc/cpuinfo")
    if information.exists():
        for line in information.read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor()


def report(args, measurements, hashes, sources):
    rows = []
    for scenario in scenarios(args.cases, args.layouts):
        group = [item for item in measurements if item["scenario"] == scenario]
        baseline = sorted((item for item in group if item["backend"] == "baseline"),
                          key=lambda item: item["run"])
        for backend in args.encoders:
            candidate = sorted((item for item in group if item["backend"] == backend),
                               key=lambda item: item["run"])
            rows.append(summarize_pair(baseline, candidate))
    return {"created_utc": datetime.now(timezone.utc).isoformat(),
            "revision": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "compiler": subprocess.check_output(["rustc", "--version"], text=True).strip(),
            "decoders": {"pillow": pillow().__version__, "imagemagick":
                         subprocess.check_output(["convert", "-version"], text=True).splitlines()[0]},
            "platform": platform.platform(), "cpu_model": cpu_model(), "cpu": args.cpu,
            "source_state_at_report": source_state(),
            "rustflags": os.environ.get("RUSTFLAGS", ""),
            "cargo_encoded_rustflags": os.environ.get("CARGO_ENCODED_RUSTFLAGS", ""),
            "repository_release_profile": tomllib.loads(
                (ROOT / "Cargo.toml").read_text())["profile"]["release"],
            "build_provenance": "Supply frozen/candidate compiler and flags separately",
            "eligible_cpus": sorted(os.sched_getaffinity(0)),
            "child_cpu_affinity": ([args.cpu] if args.cpu is not None
                                   else sorted(os.sched_getaffinity(0))),
            "binaries_sha256": hashes,
            "sources_sha256": {name: digest(path) for name, path in sources.items()},
            "parameters": {key: str(value) if isinstance(value, Path) else value
                           for key, value in vars(args).items()},
            "summary": rows, "measurements": measurements,
            "scope": "Exact encoder; no quantizer/filter changes; no terminal presentation"}


def evidence(report):
    """Retain raw stage/total timings without repeating each sample's schema."""
    result = {key: value for key, value in report.items() if key != "measurements"}
    stages = report["measurements"][0]["stages"]
    result["timing_columns"] = ["cache", "total", *stages]
    result["timing_unit"] = "nanoseconds"
    result["observations"] = []
    for measured in report["measurements"]:
        if measured["stages"] != stages:
            raise ValueError("Cannot compact incompatible stage ordering")
        observation = {key: value for key, value in measured.items()
                       if key not in ("samples", "stages")}
        observation["wire_sha256"] = measured["samples"][0]["wire_sha256"]
        observation["timings"] = [[entry["cache"], entry["timing"]["total_ns"],
                                   *entry["timing"]["stages_ns"]]
                                  for entry in measured["samples"]]
        result["observations"].append(observation)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--encoders", nargs="+", choices=ENCODERS,
                        default=["production"])
    parser.add_argument("--cases", nargs="+", choices=CASES, default=CASES)
    parser.add_argument("--layouts", nargs="+", choices=("native", "inline"),
                        default=("native", "inline"))
    parser.add_argument("--runs", type=int, default=3, choices=range(1, 11))
    parser.add_argument("--repeats", type=int, default=20, choices=range(2, 21))
    parser.add_argument("--width", type=int, default=120)
    parser.add_argument("--height", type=int, default=36)
    parser.add_argument("--cpu", type=int)
    args = parser.parse_args()
    args.baseline = args.baseline.resolve(strict=True)
    args.candidate = args.candidate.resolve(strict=True)
    args.output = args.output.resolve()
    if args.cpu is not None and args.cpu not in os.sched_getaffinity(0):
        parser.error("Requested CPU is outside the permitted affinity")
    measurements, hashes, sources = execute(args)
    data = report(args, measurements, hashes, sources)
    (args.output / "report.json").write_text(json.dumps(data, indent=2) + "\n")
    (args.output / "evidence.json").write_text(json.dumps(evidence(data), indent=2) + "\n")


if __name__ == "__main__":
    main()
