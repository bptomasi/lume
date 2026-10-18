"""Exercise the release binary through a real Alacritty X11 window.

Run with `make check-alacritty` in a disposable Xvfb display or an X11 session.
Only an isolated demo library is opened. XTest sends actual keyboard and mouse
events; database assertions, clipboard contents and terminal restoration are
checked independently. A screenshot is evidence for human paint review, not an
automatic pixel comparison. Missing display access fails rather than skips.
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import termios
import time


def run(*arguments, **options):
    return subprocess.run(
        arguments, check=True, timeout=10, text=True, capture_output=True,
        **options,
    ).stdout.strip()


def write_json(path, value):
    pending = path.with_suffix(".pending")
    pending.write_text(json.dumps(value))
    pending.replace(path)


def wait_for(description, predicate, process, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        if process.poll() is not None:
            raise RuntimeError(f"Alacritty exited while waiting for {description}")
        time.sleep(0.025)
    raise RuntimeError(f"Timed out waiting for {description}")


def terminal_child(binary, directory):
    """Observe tty restoration outside the application, inside Alacritty."""
    before = termios.tcgetattr(sys.stdin.fileno())
    process = subprocess.Popen([str(binary), "--demo", "--offline"])
    deadline = time.monotonic() + 10
    while process.poll() is None and time.monotonic() < deadline:
        if not termios.tcgetattr(sys.stdin.fileno())[3] & termios.ICANON:
            size = os.get_terminal_size()
            write_json(directory / "terminal.json", {
                "columns": size.columns,
                "rows": size.lines,
            })
            break
        time.sleep(0.025)
    returncode = process.wait()
    restored = termios.tcgetattr(sys.stdin.fileno()) == before
    write_json(directory / "exit.json", {
        "returncode": returncode,
        "termios_restored": restored,
    })
    return 0 if returncode == 0 and restored else 1


class Session:
    def __init__(self, directory, process, window):
        self.directory = directory
        self.process = process
        self.window = window
        self.database = directory / "data/lume/demo.sqlite3"
        size = json.loads((directory / "terminal.json").read_text())
        geometry = run("xdotool", "getwindowgeometry", "--shell", window)
        pixels = dict(line.split("=", 1) for line in geometry.splitlines())
        self.cell_width = int(pixels["WIDTH"]) / size["columns"]
        self.cell_height = int(pixels["HEIGHT"]) / size["rows"]
        if size["columns"] < 120 or size["rows"] < 30:
            raise RuntimeError(f"Terminal too small for qualification: {size}")
        run("xdotool", "windowfocus", "--sync", window)

    def scalar(self, sql):
        if not self.database.exists():
            return None
        with sqlite3.connect(self.database, timeout=1) as connection:
            row = connection.execute(sql).fetchone()
            return row[0] if row else None

    def workspace(self):
        value = self.scalar("SELECT value FROM settings WHERE key='workspace'")
        return json.loads(value) if value else None

    def wait(self, description, predicate):
        return wait_for(description, predicate, self.process)

    def keys(self, *keys):
        run("xdotool", "key", "--clearmodifiers", "--delay", "40", *keys)

    def command(self, text):
        run("xdotool", "type", "--clearmodifiers", "--delay", "2", ":" + text)
        self.keys("Return")

    def position(self, column, row):
        run(
            "xdotool", "mousemove", "--sync", "--window", self.window,
            str(round((column + 0.5) * self.cell_width)),
            str(round((row + 0.5) * self.cell_height)),
        )

    def drag(self, start, end):
        self.position(*start)
        run("xdotool", "mousedown", "1")
        try:
            for step in range(1, 9):
                fraction = step / 8
                self.position(
                    start[0] + (end[0] - start[0]) * fraction,
                    start[1] + (end[1] - start[1]) * fraction,
                )
                time.sleep(0.025)
        finally:
            run("xdotool", "mouseup", "1")

    def screenshot(self, name):
        run(
            "xwd", "-silent", "-id", self.window,
            "-out", str(self.directory / f"{name}.xwd"),
        )

    def clipboard(self):
        try:
            return run("xclip", "-selection", "clipboard", "-o")
        except subprocess.CalledProcessError:
            return None


def qualify(session):
    session.command("workspace load Browse")
    session.wait("Browse workspace", lambda: session.workspace() is not None)
    original = session.workspace()["panel_order"]
    session.command("panel-focus reader")
    session.keys("ctrl+w", "v")
    session.wait("keyboard split", lambda: session.workspace()["layout"])
    session.keys("ctrl+w", "u")
    session.wait("layout undo", lambda: session.workspace()["layout"] is None)

    session.drag((23, 12), (27, 12))
    session.wait("mouse resize", lambda: session.workspace()["library_width"] == 27)
    session.command("layout-undo")
    session.wait("resize undo", lambda: session.workspace()["library_width"] == 23)
    session.drag((16, 2), (42, 1))
    session.wait(
        "mouse reorder",
        lambda: session.workspace()["panel_order"] != original,
    )
    session.screenshot("reordered")
    session.command("layout-undo")
    session.wait("reorder undo", lambda: session.workspace()["panel_order"] == original)

    session.command("fullscreen")
    session.wait("fullscreen", lambda: session.workspace()["fullscreen"])
    session.keys("g", "g", "0", "v", "l", "y")
    session.wait(
        "OSC52 clipboard", lambda: session.clipboard() == "Th",
    )
    session.screenshot("fullscreen-80")
    qualify_scroll(session)
    session.command("fullscreen-width 100")
    session.wait(
        "fullscreen width",
        lambda: session.workspace()["fullscreen_width"] == 100,
    )
    session.screenshot("fullscreen-100")
    session.command("quit")


def qualify_scroll(session):
    """A physical wheel notch must equal Vim's explicit one-line scroll."""
    progress = "SELECT progress FROM articles ORDER BY published DESC, id DESC LIMIT 1"
    session.keys("g", "g")
    session.position(70, 15)
    run("xdotool", "click", "5")
    session.command("workspace save Wheel")
    session.wait("wheel progress", lambda: session.scalar(progress) > 0)
    wheel = session.scalar(progress)
    session.keys("g", "g", "ctrl+e")
    session.command("workspace save Key")
    session.wait(
        "one-line keyboard scroll",
        lambda: "Key" in json.loads(session.scalar(
            "SELECT value FROM settings WHERE key='layouts'",
        ))["profiles"],
    )
    if session.scalar(progress) != wheel:
        raise RuntimeError("A wheel notch did not match one line of scrolling")


def configure_terminal(directory):
    for command in ["alacritty", "xdotool", "xclip", "xwd"]:
        if shutil.which(command) is None:
            raise RuntimeError(f"Required qualification tool is missing: {command}")
    if not os.environ.get("DISPLAY"):
        raise RuntimeError("Set DISPLAY or run through xvfb-run -a")
    config = directory / "alacritty.toml"
    config.write_text(
        '[window]\ndimensions = { columns = 144, lines = 44 }\n'
        'padding = { x = 0, y = 0 }\ndynamic_padding = false\n'
        'decorations = "None"\n[font]\nsize = 11.0\n'
        '[scrolling]\nmultiplier = 1\n'
    )
    environment = os.environ.copy()
    environment.pop("WAYLAND_DISPLAY", None)
    environment.pop("NO_COLOR", None)
    environment.update(
        WINIT_UNIX_BACKEND="x11", XDG_DATA_HOME=str(directory / "data"),
    )
    return config, environment


def launch(binary, directory):
    config, environment = configure_terminal(directory)
    with (directory / "alacritty.log").open("w") as log:
        process = subprocess.Popen(
            [
                "alacritty", "--config-file", str(config), "--title",
                "Lume qualification", "-e", sys.executable,
                str(Path(__file__).resolve()),
                "--terminal-child", "--binary", str(binary),
                "--artifacts", str(directory),
            ], env=environment, stdout=log, stderr=log,
        )
        try:
            wait_for(
                "terminal startup",
                lambda: (directory / "terminal.json").exists(), process,
            )
            # Allow the terminal's initial capability replies to precede input.
            time.sleep(0.25)
            windows = run(
                "xdotool", "search", "--sync", "--pid", str(process.pid),
            )
            window = windows.splitlines()[-1]
            session = Session(directory, process, window)
            qualify(session)
            process.wait(timeout=10)
            result = json.loads((directory / "exit.json").read_text())
            successful_exit = {"returncode": 0, "termios_restored": True}
            if process.returncode != 0 or result != successful_exit:
                raise RuntimeError(f"Application/terminal cleanup failed: {result}")
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
    directory = arguments.artifacts
    if directory is None:
        directory = Path(tempfile.mkdtemp(prefix="lume-alacritty-"))
    directory = directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    binary = arguments.binary.resolve(strict=True)
    if arguments.terminal_child:
        return terminal_child(binary, directory)
    if any(directory.iterdir()):
        print("FAIL: Use an empty artifact directory for isolation", file=sys.stderr)
        return 1
    try:
        launch(binary, directory)
        write_json(directory / "result.json", {"passed": True})
        print(f"PASS: Alacritty input, clipboard and cleanup: {directory}")
        return 0
    except (OSError, RuntimeError, subprocess.SubprocessError, sqlite3.Error) as error:
        write_json(directory / "result.json", {
            "passed": False,
            "error": str(error),
        })
        print(f"FAIL: {error}; evidence: {directory}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
