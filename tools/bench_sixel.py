"""Profile real Sixel preparation and independently qualify emitted pixels.

Child-process RSS includes setup, PNG fixtures and reference export. Timed
samples contain only Decoder.render, excluding I/O, metric calculation and the
terminal/compositor. Algorithm order rotates; processes run serially.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import html
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess

from visual_dependencies import pillow

ROOT = Path(__file__).resolve().parents[1]
STRATEGIES = ("reference", "dedup", "direct", "undithered", "median-cut")
CASES = ("diagram", "antialias", "gradient", "photo", "entropy", "transparent")


def build():
    result = subprocess.run(
        ["cargo", "build", "--locked", "--release", "--example", "sixel_profile",
         "--message-format=json"], cwd=ROOT, check=True, text=True,
        capture_output=True,
    )
    binaries = [json.loads(line).get("executable")
                for line in result.stdout.splitlines()
                if line.startswith("{")]
    return Path(next(path for path in reversed(binaries) if path)).resolve()


def decode(directory, sample):
    image = pillow()
    width, height = sample["fitted_dimensions"]
    canvas = image.new("RGB", (width, height))
    top = 0
    environment = dict(os.environ, MAGICK_THREAD_LIMIT="1")
    for index in range(sample["strips"]):
        source = directory / f"strip-{index:03}.six"
        output = source.with_suffix(".png")
        subprocess.run(["convert", str(source), "-depth", "8", f"PNG24:{output}"],
                       env=environment, check=True, capture_output=True)
        with image.open(output) as strip:
            if strip.width != width or top + strip.height > height:
                raise RuntimeError("Sixel escaped its expected pixel bounds")
            canvas.paste(strip, (0, top))
            top += strip.height
    if top != height:
        raise RuntimeError("Sixel lost the partial final strip")
    canvas.save(directory / "decoded.png")
    return hashlib.sha256(canvas.tobytes()).hexdigest()


def run(binary, directory, case, strategy, layout, light, size):
    directory.mkdir(parents=True)
    arguments = [str(binary), "--case", case, "--strategy", strategy,
                 "--width", str(size[0]), "--height", str(size[1]),
                 "--output", str(directory)]
    if layout == "inline":
        arguments.append("--inline")
    if light:
        arguments.append("--light")
    resources = directory / "resources.json"
    subprocess.run(["/usr/bin/time", "-f",
                    '{"wall_seconds":%e,"peak_rss_kib":%M,"user_seconds":%U,'
                    '"system_seconds":%S}', "-o", str(resources), *arguments],
                   check=True, capture_output=True, text=True)
    sample = json.loads((directory / "sample.json").read_text())
    sample["decoded_sha256"] = decode(directory, sample)
    comparison = subprocess.check_output(
        [str(binary), "--compare", str(directory / "reference.png"),
         "--actual", str(directory / "decoded.png")], text=True)
    sample["quality"] = json.loads(comparison)
    sample["low_frequency_quality"] = low_frequency_quality(binary, directory)
    sample["resources"] = json.loads(resources.read_text())
    sample["artifacts"] = str(directory)
    return sample


def low_frequency_quality(binary, directory):
    """Eight-pixel neighborhoods expose diffusion's local color averaging."""
    image = pillow()
    for name in ("reference", "decoded"):
        with image.open(directory / f"{name}.png") as source:
            size = (max(1, source.width // 8), max(1, source.height // 8))
            source.resize(size, image.Resampling.BOX).save(
                directory / f"{name}-lowpass.png")
    result = subprocess.check_output(
        [str(binary), "--compare", str(directory / "reference-lowpass.png"),
         "--actual", str(directory / "decoded-lowpass.png")], text=True)
    return json.loads(result)


def summarize(runs):
    groups = {}
    for item in runs:
        key = (item["case"], item["inline"], item["light"], item["strategy"])
        groups.setdefault(key, []).append(item)
    result = []
    for key, items in groups.items():
        first = items[0]
        row = {name: first[name] for name in (
            "case", "strategy", "inline", "light", "fitted_dimensions",
            "source_sha256", "reference_sha256", "decoded_sha256", "wire_bytes",
            "quality", "low_frequency_quality", "stages")}
        row["runs"] = len(items)
        row["rss_kib"] = [item["resources"]["peak_rss_kib"] for item in items]
        row["cpu_seconds"] = [item["resources"]["user_seconds"] for item in items]
        for cache in ("cold", "warm"):
            timings = [sample["timing"] for item in items
                       for sample in item["samples"] if sample["cache"] == cache]
            row[cache + "_ms"] = [item["total_ns"] / 1e6 for item in timings]
            row[cache + "_stages_ms"] = {
                stage: statistics.median(item["stages_ns"][index] / 1e6 for item in timings)
                for index, stage in enumerate(first["stages"])}
        result.append(row)
    return result


def qualify(samples):
    """A faster implementation must preserve every reference wire and pixel."""
    baseline = next(item for item in samples if item["strategy"] == "reference")
    candidate = next(item for item in samples if item["strategy"] == "dedup")
    for sample in samples:
        if sample["reference_sha256"] != baseline["reference_sha256"]:
            raise RuntimeError("Alternatives did not use identical fitted pixels")
        hashes = {entry["wire_sha256"] for entry in sample["samples"]}
        if len(hashes) != 1:
            raise RuntimeError("Cold/warm cache changed emitted Sixel pixels")
    if (candidate["decoded_sha256"] != baseline["decoded_sha256"]
            or candidate["samples"][0]["wire_sha256"]
            != baseline["samples"][0]["wire_sha256"]):
        raise RuntimeError("Staged production quantizer changed reference pixels")


def gallery(output, runs):
    rows = []
    for item in runs:
        if item["run"] != 0:
            continue
        directory = Path(item["artifacts"]).relative_to(output.parent)
        label = html.escape(f'{item["case"]} · {item["strategy"]} · '
                            f'{"inline" if item["inline"] else "native"} · '
                            f'{"light" if item["light"] else "dark"}')
        rows.append(f'<figure><figcaption>{label}</figcaption>'
                    f'<a href="{directory}/decoded.png">'
                    f'<img src="{directory}/decoded.png"></a></figure>')
    page = ('<!doctype html><meta charset="utf-8"><title>Sixel comparison</title>'
            '<style>body{background:#eee;font:16px sans-serif}main{display:grid;'
            'grid-template-columns:repeat(5,1fr)}figure{margin:8px}img{width:100%}'
            '</style><h1>Independently decoded Sixel</h1><p>Open each image at '
            'native resolution to inspect edges and gradient banding.</p><main>'
            + ''.join(rows) + '</main>')
    output.with_suffix(".html").write_text(page)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=Path(".local/benchmarks/sixel/report.json"))
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--runs", type=int, default=3, choices=range(1, 11))
    parser.add_argument("--cases", nargs="+", choices=CASES, default=CASES)
    parser.add_argument("--layouts", nargs="+", choices=("native", "inline"),
                        default=("native", "inline"))
    parser.add_argument("--width", type=int, default=120)
    parser.add_argument("--height", type=int, default=36)
    args = parser.parse_args()
    binary = (args.binary or build()).resolve()
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    report = {"measured_at": datetime.now(timezone.utc).isoformat(),
              "platform": platform.platform(),
              "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "source_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT)),
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "scope": __doc__, "runs": []}
    for case in args.cases:
        for light in ([False, True] if case == "transparent" else [False]):
            for layout in args.layouts:
                for index in range(args.runs):
                    samples = []
                    offset = index % len(STRATEGIES)
                    order = STRATEGIES[offset:] + STRATEGIES[:offset]
                    for strategy in order:
                        name = f'{case}-{layout}-{"light" if light else "dark"}-{strategy}-{index}'
                        sample = run(binary, output.parent / name, case, strategy,
                                     layout, light, (args.width, args.height))
                        sample["run"] = index
                        samples.append(sample)
                    qualify(samples)
                    report["runs"].extend(samples)
                    output.write_text(json.dumps(report, indent=2) + "\n")
                    print(case, layout, light, index, "verified", flush=True)
    gallery(output, report["runs"])
    report["summary"] = summarize(report["runs"])
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(output)


if __name__ == "__main__":
    main()
