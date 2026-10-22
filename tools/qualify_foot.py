"""Qualify cached Sixel strips through a real Foot window on native Wayland.

A private PTY relay supplies deterministic input without changing user settings.
Captured output is decoded independently with ImageMagick. This proves wire
pixels, clipping, cached scroll and restoration; compositor presentation latency
and physical screen pixels require separate visual inspection.
"""

import argparse
import base64
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

from visual_dependencies import pillow
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
        r"$R{\mathcal{I}}=\text{Combine}\circ\text{Disintegrate}$"
        "\n\n"
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
    Image = pillow()

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
    write_json(directory / "native-color-uniformity.json",
               verify_uniform_regions(canvas))
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


def verify_uniform_regions(image):
    """Flat fixture fills must not acquire different colors between strips.

    Average small regions to permit continuous diffusion while detecting the
    horizontal bands caused by independent adaptive palettes on each row.
    """
    result = []
    for x in (20, 320, 620):
        means = []
        for y in range(60, 180, 20):
            left, right = round(x * image.width / 800), round((x + 16) * image.width / 800)
            top, bottom = round(y * image.height / 240), round((y + 20) * image.height / 240)
            pixels = [image.getpixel((column, row))
                      for row in range(top, bottom)
                      for column in range(left, right)]
            means.append([sum(pixel[c] for pixel in pixels) / len(pixels)
                          for c in range(3)])
        spread = [max(mean[c] for mean in means) - min(mean[c] for mean in means)
                  for c in range(3)]
        if max(spread) > 2:
            raise RuntimeError(f"Flat image fill has color bands: x={x}, spread={spread}")
        result.append({"source_x": x, "strip_mean_rgb": means, "channel_spread": spread})
    return result


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


def reader_aliases(directory, process):
    expected = "Request ownership"
    request = b"\x1b]52;c;" + base64.b64encode(expected.encode()) + b"\x1b\\"
    path = directory / "terminal-output.bin"
    before = path.stat().st_size
    # Normalize visual selection regardless of the preceding mouse gesture.
    send(directory, b"ggIVvAy")
    wait_for(
        "I/A visual yank from the heading",
        lambda: request in path.read_bytes()[before:], process,
    )
    return {"keys": "ggIVvAy", "yank": expected, "host_clipboard": "disabled"}


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
    welcome = enter_library(directory, process)
    command(directory, "fullscreen")
    initial = wait_for(
        "fullscreen native rows", lambda: reader_frame(directory, geometry), process,
    )
    pixels = verify_pixels(directory, initial, geometry["cell_pixels"][1])
    samples = scrolling(directory, process, initial)
    selections = selection_scroll(directory, process, initial)
    aliases = reader_aliases(directory, process)
    before = frame(directory)["offset"]
    command(directory, "help")
    wait_for(
        "help covers native rows",
        lambda: newer_plain_frame(directory, before), process,
    )
    guide = guide_search(directory, process)
    send(directory, b"\x1b")
    wait_for("reader returns", lambda: native_frame(directory, before), process)
    inspector = inspect(directory, process, initial, geometry["cell_pixels"][1])
    services = reading_services(directory, process)
    preference_edits = preferences(directory, process)
    command(directory, "quit")
    process.wait(timeout=10)
    cleanup = json.loads((directory / "exit.json").read_text())
    if cleanup != {"returncode": 0, "termios_restored": True}:
        raise RuntimeError(f"Foot terminal cleanup failed: {cleanup}")
    return {"geometry": geometry, "welcome": welcome, "decoded_strips": pixels,
            "scroll_rows": samples, "selection": selections, "reader_aliases": aliases,
            "help_search": guide, "inspector": inspector,
            "reading_services": services, "preferences": preference_edits,
            "cleanup": cleanup}


def wire_contains(raw, text):
    raw = SIXEL.sub(b"", raw)
    raw = re.sub(rb"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)", b"", raw)
    raw = re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", raw)
    # Ratatui may emit spaces as cursor advances and split text at highlights.
    visible = "".join(raw.decode("utf-8", errors="replace").split())
    return "".join(text.split()) in visible


def preferences(directory, process):
    output = directory / "terminal-output.bin"
    results = []
    source = r"$R{\mathcal{I}}=\text{Combine}\circ\text{Disintegrate}$"
    for enabled, visible in [(False, source), (True, "Rℐ = Combine ∘ Disintegrate")]:
        start = output.stat().st_size
        command(directory, "latex-formulas " + ("on" if enabled else "off"))
        command(directory, "find Combine")
        wait_for("formula display policy", lambda: wire_contains(output.read_bytes()[start:], visible), process)
        time.sleep(0.1)
        start = output.stat().st_size
        send(directory, b"IVvAy")
        request = b"\x1b]52;c;" + base64.b64encode(visible.encode()) + b"\x1b\\"
        wait_for("formula visible-text copy", lambda: request in output.read_bytes()[start:], process)
        results.append({"enabled": enabled, "visible": visible, "copied": visible})
        send(directory, b"\x1b")
        time.sleep(0.1)
    before = frame(directory)["offset"]
    command(directory, "settings")
    wait_for("editable settings", lambda: service_frame(directory, before, b"Ctrl-s"), process)
    send(directory, b"j\r")
    time.sleep(0.1)
    send(directory, b"\x1b[200~96\x1b[201~\r")
    time.sleep(0.1)
    send(directory, b"\x13")
    wait_for("settings return to reader", lambda: native_frame(directory, before), process)
    return {"formula_policies": results, "settings_enter_edit_ctrl_s_save": True}


def guide_search(directory, process):
    output = directory / "terminal-output.bin"
    start = output.stat().st_size
    send(directory, b"/")
    time.sleep(0.1)
    send(directory, b"\x1b[200~resize\x1b[201~")
    wait_for("live guide search", lambda: b"matches" in output.read_bytes()[start:]
             and b"resize" in output.read_bytes()[start:], process)
    send(directory, b"\r")
    time.sleep(0.1)
    for key in (b"n", b"N", b"\x1b"):
        previous = frame(directory)["offset"]
        send(directory, key)
        wait_for("guide search navigation", lambda: service_frame(directory, previous, b""), process)
    raw = output.read_bytes()
    if "Rℐ = Combine ∘ Disintegrate".encode() not in raw or b"\\mathcal" in raw:
        raise RuntimeError("Terminal formula projection did not match the source fixture")
    return {"live_query": "resize", "next_previous": True,
            "escape_clears_first": True, "unicode_formula": True}


def reading_services(directory, process):
    before = frame(directory)["offset"]
    command(directory, "open-page https://journal.example/article")
    linked = wait_for(
        "temporary linked reading without native image leakage",
        lambda: service_frame(directory, before, b":save-page"), process,
    )
    send(directory, b"\x0f")
    wait_for("linked back restores native rows",
             lambda: native_frame(directory, linked["offset"]), process)
    before = frame(directory)["offset"]
    command(directory, "performance on")
    enabled = wait_for(
        "opt-in performance report", lambda: service_frame(directory, before, b"p99"),
        process,
    )
    send(directory, b"\x1b")
    wait_for("diagnostic close restores native rows",
             lambda: native_frame(directory, enabled["offset"]), process)
    command(directory, "performance off")
    disabled = wait_for(
        "disabled performance report",
        lambda: service_frame(directory, enabled["offset"], b"disabled"), process,
    )
    send(directory, b"\x1b")
    wait_for("final reading restored",
             lambda: native_frame(directory, disabled["offset"]), process)
    return {"linked_text_view": True, "back_restores_images": True,
            "diagnostics_opt_in": True}


def service_frame(directory, after, marker):
    current = frame(directory)
    if (current and current["offset"] > after and not current["strips"]
            and marker in current["content"]):
        return current
    return None


def enter_library(directory, process):
    output = directory / "terminal-output.bin"
    wait_for(
        "native welcome screen",
        lambda: output.exists() and b"Press any key to begin" in output.read_bytes(),
        process,
    )
    initial = output.read_bytes()
    if "▀".encode() not in initial or b"Inbox" in initial:
        raise RuntimeError("Welcome must show native art before the library")
    # Even q must be consumed as the entry key rather than quitting the library.
    before = len(initial)
    send(directory, b"q")
    wait_for(
        "library after entry key",
        lambda: b"Inbox" in output.read_bytes()[before:], process,
    )
    wait_for(
        "prepared article after background startup",
        lambda: b"Request ownership" in output.read_bytes()[before:], process,
    )
    return {"native_pixel_art": True, "entry_key_consumed": True}


def newer_plain_frame(directory, after):
    current = frame(directory)
    if current and current["offset"] > after and not current["strips"]:
        return current if b"LAYOUT AND APPEARANCE" in current["content"] else None
    return None


def launch(binary, directory):
    pillow()
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
        "[security]\nosc52=disabled\n"
    )
    environment = os.environ.copy()
    environment.update(LUME_DATA_DIR=str(directory / "data/lume"),
                           LUME_CONFIG=str(directory / "config.toml"))
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
                saved = json.loads(db.execute(
                    "SELECT value FROM settings WHERE key = 'workspace'",
                ).fetchone()[0])
                if saved["reading_width"] != 96 or not saved["latex_formulas"]:
                    raise RuntimeError("Entered preferences were not persisted")
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
        return relay(binary, directory, startup_markers={
            "welcome_output_ms": b"Press any key to begin",
            "library_output_ms": b"Inbox",
            "reader_output_ms": b"Request ownership",
        })
    if any(directory.iterdir()):
        raise RuntimeError("Use an empty artifact directory for isolation")
    report = launch(binary, directory)
    report.update(
        passed=True, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        foot_version=subprocess.check_output(["foot", "--version"], text=True).strip(),
        lume_version=subprocess.check_output([str(binary), "--version"], text=True).strip(),
    )
    report["startup_output_milestones"] = json.loads((directory / "startup.json").read_text())
    write_json(directory / "result.json", report)
    print(f"PASS: Foot native Sixel images; evidence: {directory / 'result.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
