"""Run real Wayland clients with a private Weston X11 seat inside Xvfb.

Weston's headless backend creates no input seat on Ubuntu 24.04. Its X11
backend provides a real wl_seat while software rendering avoids a GPU
requirement. Foot still communicates over native Wayland, without Xwayland.
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from terminal_support import startup_failure


def wait_for_compositor(process, socket, environment):
    deadline = time.monotonic() + 10
    while not socket.is_socket():
        if process.poll() is not None:
            raise RuntimeError(f"Weston exited ({process.returncode}) before startup")
        if time.monotonic() >= deadline:
            raise RuntimeError("Weston did not create its Wayland socket")
        time.sleep(0.05)
    result = subprocess.run(["wayland-info"], env=environment, text=True,
                            capture_output=True, timeout=10, check=True)
    if "wl_seat" not in result.stdout:
        raise RuntimeError("Weston supplied no wl_seat for terminal input")
    return result.stdout


def qualify(directory, command):
    if not os.environ.get("DISPLAY"):
        raise RuntimeError("Run this qualification through xvfb-run -a")
    for name in ("weston", "wayland-info"):
        if shutil.which(name) is None:
            raise RuntimeError(f"Missing qualification dependency: {name}")
    with tempfile.TemporaryDirectory(prefix="lume-wayland-") as runtime:
        environment = dict(os.environ, XDG_RUNTIME_DIR=runtime,
                           WAYLAND_DISPLAY="wayland-lume")
        arguments = ["weston", "--no-config", "--backend=x11-backend.so",
                     "--renderer=pixman", "--width=1920", "--height=1440",
                     "--socket=wayland-lume", "--idle-time=0"]
        with (directory / "weston.log").open("w") as log:
            process = subprocess.Popen(arguments, env=environment,
                                       stdout=log, stderr=log)
            try:
                info = wait_for_compositor(
                    process, Path(runtime) / "wayland-lume", environment)
                (directory / "wayland-info.txt").write_text(info)
                return subprocess.run(command, env=environment, check=False).returncode
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        parser.error("Provide the qualification command after --")
    directory = arguments.artifacts.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    try:
        return qualify(directory, command)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(startup_failure(error, directory, ["weston.log"]), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
