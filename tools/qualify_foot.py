"""Qualify cached Sixel strips through a real Foot window on native Wayland.

A private PTY relay supplies deterministic input without changing user settings.
Captured output is decoded independently with ImageMagick. This proves wire
pixels, clipping, cached scroll and restoration; compositor presentation latency
and physical screen pixels require separate visual inspection.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

from foot_terminal import relay
from qualify_alacritty import wait_for, write_json

ADDRESS = "https://example.org/request-path.svg"
BEGIN = b"\x1b[?2026h"
END = b"\x1b[?2026l"
SIXEL = re.compile(rb"\x1bP[^q]*q.*?\x1b\\", re.DOTALL)


def seed(directory):
    project = Path(__file__).resolve().parents[1]
    library = directory / "data/lume"
    library.mkdir(parents=True)
    body = (
        "# Request ownership\n\nEach boundary owns a responsibility.\n\n"
        f"![Request path]({ADDRESS})\n\n"
        + "Read the mechanism and question the tradeoff.\n\n" * 80
    )
    with sqlite3.connect(library / "library.sqlite3") as connection:
        connection.executescript(
            (project / "src/store/sql/create_schema.sql").read_text()
        )
        connection.executescript(
            (project / "src/store/sql/test_setup_install_library.sql").read_text()
        )
        connection.execute("UPDATE articles SET body = ?, unread = 1", (body,))
    cache = library / "images"
    cache.mkdir()
    name = hashlib.sha256(ADDRESS.encode()).hexdigest() + ".img"
    (cache / name).write_bytes(
        (project / "tests/fixtures/images/request-path.svg").read_bytes()
    )
    return body


def send(directory, data):
    descriptor = os.open(directory / "input.fifo", os.O_WRONLY | os.O_NONBLOCK)
    try:
        os.write(descriptor, data)
    finally:
        os.close(descriptor)


def command(directory, text):
    for character in ":" + text:
        send(directory, character.encode())
        time.sleep(0.01)
    send(directory, b"\r")


def frame(directory):
    path = directory / "terminal-output.bin"
    if not path.exists():
        return None
    raw = path.read_bytes()
    end = raw.rfind(END)
    start = raw.rfind(BEGIN, 0, end)
    if start < 0 or end < 0:
        return None
    content = raw[start + len(BEGIN):end]
    strips = []
    for match in SIXEL.finditer(content):
        position = list(re.finditer(rb"\x1b\[(\d+);(\d+)H", content[:match.start()]))
        dimensions = re.search(rb'"1;1;(\d+);(\d+)', match[0])
        if not position or not dimensions:
            raise RuntimeError("Sixel strip lacks a bounded position/dimensions")
        cursor = position[-1]
        strips.append({
            "row": int(cursor[1]) - 1, "column": int(cursor[2]) - 1,
            "width": int(dimensions[1]), "height": int(dimensions[2]),
            "sha256": hashlib.sha256(match[0]).hexdigest(),
            "data": match[0],
        })
    return {"offset": end, "content": content, "strips": strips}


def native_frame(directory, after=-1):
    current = frame(directory)
    if current and current["offset"] > after and current["strips"]:
        return current
    return None


def reader_frame(directory, geometry):
    current = native_frame(directory)
    cell_width, cell_height = geometry["cell_pixels"]
    if not (1 <= cell_width <= 64 and 1 <= cell_height <= 128):
        cell_width, cell_height = 10, 20
    width = round(min(80 * cell_width, 12 * cell_height * 800 / 240))
    if current and any(strip["width"] == width for strip in current["strips"]):
        return current
    return None


def verify_pixels(directory, initial, cell_height):
    from PIL import Image

    results = []
    decoded = []
    for index, strip in enumerate(initial["strips"]):
        if strip["height"] > cell_height:
            raise RuntimeError("Sixel strip is taller than a terminal row")
        path = directory / f"strip-{index}.sixel"
        path.write_bytes(strip["data"])
        target = path.with_suffix(".png")
        subprocess.run(["convert", str(path), str(target)], check=True, timeout=10)
        with Image.open(target) as image:
            if image.width != strip["width"]:
                raise RuntimeError("Independent decoder changed strip width")
            if image.height != strip["height"]:
                raise RuntimeError("Sixel pixels overflow the declared strip height")
            decoded.append(image.convert("RGB"))
            results.append({"size": image.size, "sha256": strip["sha256"]})
    if len(results) < 4:
        raise RuntimeError("Too few decoded strips to qualify inline detail")
    canvas = Image.new(
        "RGB", (decoded[0].width, sum(image.height for image in decoded)),
    )
    offset = 0
    for image in decoded:
        canvas.paste(image, (0, offset))
        offset += image.height
    canvas.save(directory / "native-reader-pixels.png")
    verify_colors(canvas)
    return results


def verify_colors(image):
    # icy_sixel 0.1.3 bins RGB into five-bit channels, then emits integer
    # percentages. These independent fixture expectations include that
    # documented quantization; one value permits decoder rounding.
    for x, y, expected in [
        (20, 100, (224, 176, 105)),
        (320, 100, (143, 176, 199)),
        (620, 100, (176, 191, 153)),
        (250, 0, (15, 15, 23)),
    ]:
        point = (round(x * image.width / 800), round(y * image.height / 240))
        actual = image.getpixel(point)
        if any(abs(left - right) > 1 for left, right in zip(actual, expected)):
            raise RuntimeError(f"Known Sixel color changed: {point}: {actual}")


def scrolling(directory, process, initial):
    identities = {strip["sha256"] for strip in initial["strips"]}
    current = initial
    samples = []
    for key in [b"\x05"] * 8 + [b"\x19"] * 8:
        send(directory, key)
        previous = current["offset"]
        current = wait_for(
            "clipped native rows", lambda: native_frame(directory, previous), process,
        )
        if any(strip["sha256"] not in identities for strip in current["strips"]):
            raise RuntimeError("Scroll replaced a cached source strip")
        if any(strip["row"] < 6 for strip in current["strips"]):
            raise RuntimeError("Native pixels crossed the Reader header")
        samples.append([strip["row"] for strip in current["strips"]])
    if len({tuple(rows) for rows in samples}) < 3:
        raise RuntimeError("Scroll did not move the native rows")
    return samples


def selection_scroll(directory, process, initial):
    identities = {strip["sha256"] for strip in initial["strips"]}
    samples = []
    for key in (b"v", b"V"):
        send(directory, b"gg")
        time.sleep(0.1)
        send(directory, key)
        time.sleep(0.1)
        current = selected_scroll(directory, process)
        if any(strip["sha256"] not in identities for strip in current["strips"]):
            raise RuntimeError("Visual selection replaced cached image pixels")
        samples.append({"mode": key.decode(), "rows": dimensions(current)})
        send(directory, b"\x1b\x19")
    first = initial["strips"][0]
    column, row = first["column"] + 2, first["row"] - 2
    send(directory, f"\x1b[<0;{column};{row}M".encode())
    time.sleep(0.1)
    current = selected_scroll(directory, process)
    if any(strip["sha256"] not in identities for strip in current["strips"]):
        raise RuntimeError("Mouse selection replaced cached image pixels")
    send(directory, f"\x1b[<0;{column};{row}m".encode())
    send(directory, b"\x1b\x19")
    return {"keyboard": samples, "mouse_down_scroll": dimensions(current)}


def selected_scroll(directory, process):
    before = frame(directory)["offset"]
    send(directory, b"\x05")
    return wait_for(
        "native rows remain during selection",
        lambda: native_frame(directory, before), process,
    )


def inspect(directory, process, initial, cell_height):
    send(directory, b"gg")
    time.sleep(0.1)
    first = initial["strips"][0]
    column = first["column"] + 2
    row = first["row"] + 2
    before = frame(directory)["offset"]
    send(directory, f"\x1b[<0;{column};{row}M".encode())
    send(directory, f"\x1b[<0;{column};{row}m".encode())
    fitted = wait_for(
        "click opens native inspection",
        lambda: inspector_frame(directory, before, cell_height), process,
    )
    send(directory, b"+")
    zoomed = wait_for(
        "native source zoom",
        lambda: inspector_frame(directory, fitted["offset"], cell_height), process,
    )
    send(directory, b"\x1b")
    wait_for(
        "return to inline strips",
        lambda: returned_frame(directory, zoomed["offset"], cell_height), process,
    )
    return {"fit": dimensions(fitted), "zoom": dimensions(zoomed)}


def dimensions(current):
    return [(strip["width"], strip["height"]) for strip in current["strips"]]


def inspector_frame(directory, after, cell_height):
    current = native_frame(directory, after)
    if current and any(strip["height"] > cell_height for strip in current["strips"]):
        return current
    return None


def returned_frame(directory, after, cell_height):
    current = native_frame(directory, after)
    if current and all(strip["height"] <= cell_height for strip in current["strips"]):
        return current
    return None


def qualify(directory, process):
    wait_for(
        "Foot terminal startup", lambda: (directory / "terminal.json").exists(),
        process,
    )
    time.sleep(0.25)
    geometry = json.loads((directory / "terminal.json").read_text())
    command(directory, "fullscreen")
    initial = wait_for(
        "fullscreen native rows", lambda: reader_frame(directory, geometry), process,
    )
    pixels = verify_pixels(directory, initial, geometry["cell_pixels"][1])
    samples = scrolling(directory, process, initial)
    selections = selection_scroll(directory, process, initial)
    before = frame(directory)["offset"]
    command(directory, "help")
    wait_for(
        "help covers native rows",
        lambda: newer_plain_frame(directory, before), process,
    )
    send(directory, b"\x1b")
    wait_for("reader returns", lambda: native_frame(directory, before), process)
    inspector = inspect(directory, process, initial, geometry["cell_pixels"][1])
    command(directory, "quit")
    process.wait(timeout=10)
    cleanup = json.loads((directory / "exit.json").read_text())
    if cleanup != {"returncode": 0, "termios_restored": True}:
        raise RuntimeError(f"Foot terminal cleanup failed: {cleanup}")
    return {"geometry": geometry, "decoded_strips": pixels,
            "scroll_rows": samples, "selection": selections,
            "inspector": inspector, "cleanup": cleanup}


def newer_plain_frame(directory, after):
    current = frame(directory)
    if current and current["offset"] > after and not current["strips"]:
        return current if b"LAYOUT AND APPEARANCE" in current["content"] else None
    return None


def launch(binary, directory):
    for name in ("foot", "convert"):
        if shutil.which(name) is None:
            raise RuntimeError(f"Required qualification tool is missing: {name}")
    if not os.environ.get("WAYLAND_DISPLAY"):
        raise RuntimeError("Native Wayland is required")
    body = seed(directory)
    configuration = directory / "foot.ini"
    configuration.write_text(
        "[main]\nfont=FiraCode Nerd Font Mono:size=12\n"
        "initial-window-size-chars=120x40\npad=0x0\n"
    )
    environment = os.environ.copy()
    environment.update(LUME_DATA_DIR=str(directory / "data/lume"))
    with (directory / "foot.log").open("w") as log:
        process = subprocess.Popen([
            "foot", "--config", str(configuration), "--title",
            "Lume Foot qualification", sys.executable, str(Path(__file__).resolve()),
            "--terminal-child", "--binary", str(binary), "--artifacts", str(directory),
        ], env=environment, stdout=log, stderr=log)
        try:
            report = qualify(directory, process)
            with sqlite3.connect(directory / "data/lume/library.sqlite3") as db:
                actual = db.execute("SELECT body FROM articles").fetchall()
            if not actual or any(row[0] != body for row in actual):
                raise RuntimeError("Image interaction changed publisher text")
            report["publisher_content_preserved"] = True
            return report
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/lume"))
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--terminal-child", action="store_true", help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    directory = arguments.artifacts or Path(tempfile.mkdtemp(prefix="lume-foot-"))
    directory = directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    binary = arguments.binary.resolve(strict=True)
    if arguments.terminal_child:
        return relay(binary, directory)
    if any(directory.iterdir()):
        raise RuntimeError("Use an empty artifact directory for isolation")
    report = launch(binary, directory)
    report.update(
        passed=True, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        foot_version=subprocess.check_output(["foot", "--version"], text=True).strip(),
    )
    write_json(directory / "result.json", report)
    print(f"PASS: Foot native Sixel images; evidence: {directory / 'result.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
