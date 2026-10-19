"""Qualify persistent native image pixels and scrolling in real Alacritty.

Requires an isolated X11 display, Alacritty, xdotool, xev, ImageMagick and Pillow.
Only a fresh owned library/window is opened. Full bitmap comparison distinguishes
native pixels from character approximations. X11 color rounding may differ by
one channel value; geometry, process and note cleanup are checked independently.
"""

import argparse
import json
import os
import re
from pathlib import Path
import shlex
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

from PIL import Image, ImageChops

from qualify_alacritty import (
    Session, configure_terminal, run, terminal_child, wait_for, write_json,
)

ADDRESS = "https://example.org/request-path.svg"


def seed(directory):
    import hashlib
    project = Path(__file__).resolve().parents[1]
    library = directory / "data/lume"
    library.mkdir(parents=True)
    body = (
        "# One request, clear ownership\n\n"
        "Each boundary owns one responsibility.\n\n"
        f"![Request path]({ADDRESS})\n\n"
        f"![Second request path]({ADDRESS})\n\n"
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
    # A non-cell-aligned width catches accidental helper resampling after crops.
    fixture = (project / "tests/fixtures/images/request-path.svg").read_bytes()
    (cache / name).write_bytes(fixture.replace(
        b'width="800" height="240"', b'width="797" height="240"', 1,
    ))
    return body


def equal_pixels(actual, expected):
    difference = ImageChops.difference(actual, expected)
    return all(maximum <= 1 for _, maximum in difference.getextrema())


def bitmap_positions(screen, expected):
    """Locate a solid-row anchor, then compare every source pixel at full size."""
    screen = screen.convert("RGB")
    expected = expected.convert("RGB")
    if expected.width > screen.width or expected.height > screen.height:
        return []
    row = expected.height // 2
    expected_row = expected.crop((0, row, expected.width, row + 1))
    needle = expected_row.crop((0, 0, min(16, expected.width), 1)).tobytes()
    matches = []
    for y in range(row, screen.height - expected.height + row + 1):
        pixels = screen.crop((0, y, screen.width, y + 1)).tobytes()
        offset = pixels.find(needle)
        while offset >= 0:
            x, top = offset // 3, y - row
            actual_row = screen.crop((x, y, x + expected.width, y + 1))
            if offset % 3 == 0 and equal_pixels(actual_row, expected_row):
                actual = screen.crop(
                    (x, top, x + expected.width, top + expected.height)
                )
                if equal_pixels(actual, expected):
                    matches.append((x, top))
            offset = pixels.find(needle, offset + 1)
    return matches


def capture(session, name):
    path = session.directory / f"{name}.png"
    run("import", "-window", session.window, str(path))
    with Image.open(path) as image:
        return image.convert("RGB")


def native_bitmap(session, minimum=1, latest=False):
    sources = []
    for path in (session.directory / "prepared").glob("lume-image-*.png"):
        try:
            sources.append((path.stat().st_mtime_ns, path))
        except FileNotFoundError:
            continue
    sources.sort(reverse=True)
    if latest:
        sources = sources[:1]
    screen = capture(session, "current-frame")
    for _, source in sources:
        try:
            with Image.open(source) as expected:
                positions = bitmap_positions(screen, expected)
                if len(positions) >= minimum:
                    return source, positions, expected.size
        except OSError:
            # An unfinished PNG or an evicted crop is not a displayed frame.
            continue
    return None


def qualify_reader(session):
    session.command("fullscreen")
    initial = session.wait("two native bitmaps", lambda: native_bitmap(session, 2))
    source, positions, dimensions = initial
    shutil.copyfile(source, session.directory / "reader-source.png")
    capture(session, "reader-pixels")
    session.position(70, 20)
    expected_y = positions[0][1] - round(session.cell_height)

    def moved_one_line():
        current = native_bitmap(session)
        return current if current and current[1][0][1] == expected_y else None

    # Observe only the press: Xvfb can emit another detent on XTest release.
    run("xdotool", "mousedown", "5")
    try:
        session.wait("one-row image movement from a wheel press", moved_one_line)
    finally:
        run("xdotool", "mouseup", "5")
    return dimensions


def process_resources(session):
    pid = json.loads((session.directory / "terminal.json").read_text())["pid"]
    root = Path(f"/proc/{pid}")
    fields = (root / "stat").read_text().rsplit(")", 1)[1].split()
    cpu = (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")
    status = (root / "status").read_text()
    peak = re.search(r"^VmHWM:\s+(\d+) kB$", status, re.MULTILINE)
    if peak is None:
        raise RuntimeError("Lume peak resident memory is unavailable")
    return {"cpu_seconds": cpu, "peak_rss_kib": int(peak[1])}


def qualify_continuous_scroll(session):
    session.keys("g", "g")
    session.wait("settled persistent windows", lambda: native_bitmap(session, 2))
    journal = session.directory / "window-events.log"
    with journal.open("w") as log:
        observer = subprocess.Popen([
            "stdbuf", "-oL", "xev", "-id", session.window,
            "-event", "substructure",
        ], stdout=log, stderr=log)
        try:
            time.sleep(0.05)
            started = time.monotonic()
            before = process_resources(session)
            result = observe_scrolling(session)
            after = process_resources(session)
            result.update(
                elapsed_seconds=round(time.monotonic() - started, 6),
                lume_cpu_seconds=round(after["cpu_seconds"] - before["cpu_seconds"], 6),
                lume_peak_rss_kib=after["peak_rss_kib"],
            )
        finally:
            observer.terminate()
            observer.wait(timeout=10)
    events = journal.read_text()
    counts = {name: len(re.findall(rf"\b{name} event,", events)) for name in (
        "ConfigureNotify", "CreateNotify", "DestroyNotify", "UnmapNotify", "MapNotify",
    )}
    if counts["ConfigureNotify"] < 4:
        raise RuntimeError(f"No reliable window movement observation: {counts}")
    if any(counts[name] for name in counts if name != "ConfigureNotify"):
        raise RuntimeError(f"Scroll recreated or hid image windows: {counts}")
    return dict(result, window_events=counts)


def observe_scrolling(session):
    session.keys("g", "g")
    session.wait("initial continuous-scroll pixels", lambda: native_bitmap(session, 2))
    repeats = 32 if getattr(session, "image_stress", False) else 8
    motions = (["ctrl+e"] * 9 + ["ctrl+y"] * 9) * repeats
    stream = subprocess.Popen([
        "xdotool", "key", "--clearmodifiers", "--delay", "8", *motions,
    ], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    observed = []
    try:
        while stream.poll() is None:
            current = native_bitmap(session)
            if current and stream.poll() is None:
                observed.append(current[1][0][1])
        if stream.returncode != 0:
            raise RuntimeError("Continuous input injection failed")
    finally:
        if stream.poll() is None:
            stream.terminate()
        stream.wait(timeout=10)
    if len(observed) < 3 or len(set(observed)) < 2:
        raise RuntimeError(f"Images did not move during active input: {observed}")
    capture(session, "continuous-scroll-pixels")
    return {"visible_samples": len(observed), "distinct_positions": len(set(observed))}


def qualify_clipping(session, dimensions):
    session.keys("g", "g")
    original = session.wait("full image before clipping", lambda: native_bitmap(session, 2))
    with Image.open(original[0]) as source:
        source = source.convert("RGB")
        cell_height = dimensions[1] // 12
        # The fullscreen fixture has a six-row fixed header and twelve image rows.
        removed = 14 * cell_height - original[1][0][1]
        if not 0 < removed < source.height:
            raise RuntimeError(f"Clipping fixture does not cross the header: {removed}")
        expected = source.crop((0, removed, source.width, source.height))
    expected.save(session.directory / "cropped-source.png")
    session.keys(*["ctrl+e"] * 8)

    def clipped_bitmap():
        positions = bitmap_positions(capture(session, "current-frame"), expected)
        return (original[1][0][0], 6 * cell_height) in positions

    session.wait("exact persistent-window clipping", clipped_bitmap)
    capture(session, "scrolled-pixels")
    session.keys("question")
    session.wait("native windows hidden for help", lambda: not image_windows())
    capture(session, "help-without-overlay")
    session.keys("Escape")
    session.wait("restored native image", lambda: native_bitmap(session))


def image_windows():
    result = subprocess.run([
        "xdotool", "search", "--onlyvisible", "--name", "^lume-(image-|inspector)",
    ], check=False, capture_output=True, text=True, timeout=10)
    if result.returncode not in (0, 1):
        raise RuntimeError(f"Cannot inspect image windows: {result.stderr}")
    return result.stdout.splitlines()


def qualify_font_change(session, dimensions):
    session.keys("g", "g", "ctrl+equal")

    def resized_bitmap():
        current = native_bitmap(session, latest=True)
        return current if current and current[2] != dimensions else None

    image = session.wait("native pixels after terminal font resize", resized_bitmap)
    shutil.copyfile(image[0], session.directory / "font-resized-source.png")
    capture(session, "font-resized-pixels")
    return image[2]


def qualify_inspector(session, dimensions):
    session.keys("g", "g")
    inline = session.wait("image before mouse inspection", lambda: native_bitmap(session))
    x, y = inline[1][0]
    run("xdotool", "mousemove", "--sync", "--window", session.window,
        str(x + inline[2][0] // 2), str(y + inline[2][1] // 2))
    run("xdotool", "click", "1")

    def larger_bitmap(height):
        current = native_bitmap(session, latest=True)
        return current if current and current[2][1] > height else None

    inspector = session.wait(
        "native image inspector", lambda: larger_bitmap(dimensions[1])
    )
    shutil.copyfile(inspector[0], session.directory / "inspector-fit-source.png")
    capture(session, "inspector-fit")
    session.keys("plus", "l")
    zoomed = session.wait(
        "source-based inspector zoom", lambda: larger_bitmap(inspector[2][1])
    )
    shutil.copyfile(zoomed[0], session.directory / "inspector-zoom-source.png")
    capture(session, "inspector-zoom")
    session.keys("Escape")
    return {
        "click_opens_inspector": True,
        "inspector_fit_dimensions": inspector[2],
        "inspector_zoom_dimensions": zoomed[2],
    }


def qualify(session, body):
    dimensions = qualify_reader(session)
    continuous = qualify_continuous_scroll(session)
    resized = qualify_font_change(session, dimensions)
    qualify_clipping(session, resized)
    inspector = qualify_inspector(session, resized)
    session.keys("q")
    session.wait(
        "terminal cleanup", lambda: (session.directory / "exit.json").exists()
    )
    with sqlite3.connect(session.database) as connection:
        saved_body = connection.execute(
            "SELECT body FROM articles WHERE id=42"
        ).fetchone()
        notes = connection.execute("SELECT count(*) FROM comments").fetchone()
    if saved_body != (body,) or notes != (1,):
        raise RuntimeError("Publisher text or private note changed")
    return {
        "continuous_scroll": continuous,
        "native_bitmap_dimensions": dimensions,
        "font_resized_bitmap_dimensions": resized,
        "maximum_channel_error": 1,
        "simultaneous_images": 2,
        "single_wheel_press_row": True,
        "cropping": True,
        "menu_hides_native_windows": True,
        "backend": "persistent X11",
        "source_and_note_preserved": True,
        **inspector,
    }


def launch(arguments, directory):
    body = seed(directory)
    config, environment = configure_terminal(
        directory, required=("alacritty", "xdotool", "import", "xev", "stdbuf")
    )
    prepared = directory / "prepared"
    prepared.mkdir()
    environment.update(LUME_OVERLAY_BACKEND="x11", TMPDIR=str(prepared))
    if arguments.helper is not None:
        environment["LUME_UEBERZUGPP"] = str(arguments.helper.resolve(strict=True))
    with (directory / "alacritty.log").open("w") as log:
        process = subprocess.Popen(
            ["alacritty", "--config-file", str(config), "--title",
             "Lume native image qualification", "-e", sys.executable,
             str(Path(__file__).resolve()), "--terminal-child",
             "--binary", str(arguments.binary.resolve()),
             "--artifacts", str(directory)],
            env=environment, stdout=log, stderr=log,
        )
        try:
            wait_for(
                "terminal startup",
                lambda: (directory / "terminal.json").exists(), process,
            )
            time.sleep(0.25)
            windows = run(
                "xdotool", "search", "--sync", "--pid", str(process.pid)
            )
            window = windows.splitlines()[-1]
            session = Session(directory, process, window)
            session.image_stress = arguments.stress
            session.database = directory / "data/lume/library.sqlite3"
            result = qualify(session, body)
            process.wait(timeout=10)
            cleanup = json.loads((directory / "exit.json").read_text())
            if cleanup != {"returncode": 0, "termios_restored": True}:
                raise RuntimeError(f"Terminal restoration failed: {cleanup}")
            return result
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


def nested_terminal():
    """Keep an older PTY with distinct dimensions above the Alacritty process."""
    arguments = [item for item in sys.argv[1:] if item != "--nested-terminal"]
    command = shlex.join([
        sys.executable, str(Path(__file__).resolve()),
        "--outer-pty", *arguments,
    ])
    return subprocess.call([
        "script", "--quiet", "--return", "--command", command, "/dev/null",
    ])


def size_outer_pty():
    import fcntl
    import struct
    import termios
    fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 25, 80, 800, 500))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/lume"))
    parser.add_argument("--helper", type=Path,
                        help="Optional legacy helper override; X11 uses persistent surfaces")
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--stress", action="store_true",
                        help="Run four times as many sustained scroll cycles")
    parser.add_argument("--nested-terminal", action="store_true",
                        help="Keep an ancestor PTY with different cell geometry")
    parser.add_argument("--outer-pty", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--terminal-child", action="store_true", help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    if arguments.nested_terminal:
        return nested_terminal()
    if arguments.outer_pty:
        size_outer_pty()
    directory = (
        arguments.artifacts
        or Path(tempfile.mkdtemp(prefix="lume-native-image-"))
    ).resolve()
    directory.mkdir(parents=True, exist_ok=True)
    if arguments.terminal_child:
        return terminal_child(
            arguments.binary, directory,
            ("--offline", "--data-dir", str(directory / "data/lume")),
        )
    if any(directory.iterdir()):
        parser.error("Use an empty artifact directory")
    try:
        result = launch(arguments, directory)
        write_json(directory / "result.json", dict(passed=True, **result))
        print(f"PASS: Native image pixels in Alacritty: {directory}")
        return 0
    except (OSError, RuntimeError, subprocess.SubprocessError, sqlite3.Error) as error:
        write_json(directory / "result.json", {"passed": False, "error": str(error)})
        print(f"FAIL: {error}; evidence: {directory}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
