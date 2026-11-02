"""Measure dense/scatter crossover independently from quantizer selection.

No build runs here. Use a release sixel_profile executable; every process is
serial and optionally pinned to one CPU. These synthetic probes inform integer
planner dispatch, not natural-image quality or whole-pipeline release gates.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess

from visual_dependencies import pillow


def source(width, colors):
    pixels = bytearray()
    state = 0x94A4172F
    for _ in range(width * 60):
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        color = state % colors
        pixels.extend(((color & 7) * 255 // 7,
                       ((color >> 3) & 7) * 255 // 7,
                       (color >> 6) * 255 // 3))
    return pillow().frombytes("RGB", (width, 60), bytes(pixels))


def measure(binary, image, directory, width, encoder, args):
    command = [str(binary), "--input", str(image), "--encoder", encoder,
               "--width", str(math.ceil(width / 10)), "--height", "3",
               "--repeats", "20", "--output", str(directory)]
    if args.cpu is not None:
        command = ["taskset", "-c", str(args.cpu), *command]
    subprocess.run(command, check=True, stdout=subprocess.DEVNULL)
    return json.loads((directory / "sample.json").read_text())


def case(binary, colors, width, args):
    image = args.output / f"k{colors}-w{width}.png"
    source(width, colors).save(image)
    records = {"dense": [], "sparse": []}
    for run in range(3):
        order = ["dense", "sparse"] if run % 2 == 0 else ["sparse", "dense"]
        for encoder in order:
            directory = args.output / f"k{colors}-w{width}-{run}-{encoder}"
            records[encoder].append(measure(
                binary, image, directory, width, encoder, args))
    hashes = {entry["wire_sha256"] for values in records.values()
              for record in values for entry in record["samples"]}
    references = {(record["reference_sha256"],
                   tuple(record["fitted_dimensions"]))
                  for values in records.values() for record in values}
    if len(hashes) != 1 or len(references) != 1:
        raise ValueError("Dense/scatter changed wire or fitted pixels")
    packet = (args.output / f"k{colors}-w{width}-0-dense" / "strip-000.six")
    row = dict(requested=colors, width=width, height=60,
               colors=len(re.findall(rb"#\d+;2;", packet.read_bytes())),
               wire_bytes=packet.stat().st_size,
               source_sha256=hashlib.sha256(image.read_bytes()).hexdigest())
    for encoder, values in records.items():
        index = values[0]["stages"].index("encode")
        times = [statistics.median([entry["timing"]["stages_ns"][index]
                 for entry in record["samples"][1:]]) for record in values]
        row[f"{encoder}_process_medians_ns"] = times
        row[f"{encoder}_ns"] = statistics.median(times)
        row[f"{encoder}_scratch"] = max(
            entry["timing"]["encoder_scratch_bytes"]
            for record in values for entry in record["samples"])
    row["sparse_ratio"] = row["sparse_ns"] / row["dense_ns"]
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cpu", type=int)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    rows = []
    for colors in [1, 2, 3, 8, 16, 24, 32, 48, 64, 128, 256]:
        for width in [1, 8, 32, 128, 511, 1200]:
            row = case(binary, colors, width, args)
            rows.append(row)
            print(json.dumps(row), flush=True)
    report = dict(binary=str(binary),
                  sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  cpu=args.cpu, eligible_affinity=sorted(os.sched_getaffinity(0)),
                  platform=platform.platform(), runs=3, warm_renders=19, rows=rows)
    (args.output / "summary.json").write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
