"""Portable terminal configuration and bounded startup failure diagnostics."""

from pathlib import Path
import json
import re
import subprocess
import time


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
            terminal = Path(process.args[0]).name
            raise RuntimeError(
                f"{terminal} exited ({process.returncode}) while waiting for {description}"
            )
        time.sleep(0.025)
    raise RuntimeError(f"Timed out waiting for {description}")


def foot_configuration():
    version = subprocess.check_output(["foot", "--version"], text=True, timeout=5)
    match = re.search(r"version:\s*(\d+)\.(\d+)", version)
    if match is None:
        raise RuntimeError(f"Cannot identify Foot version: {version.strip()}")
    configuration = (
        "[main]\nfont=DejaVu Sans Mono:size=12\n"
        "initial-window-size-chars=120x40\npad=0x0\n"
    )
    # The distro Foot 1.16 has Sixel but predates security.osc52 (Foot 1.20).
    # CI uses a private compositor; modern local sessions disable host access.
    if tuple(map(int, match.groups())) >= (1, 20):
        configuration += "[security]\nosc52=disabled\n"
    return configuration


def startup_failure(error, directory, names):
    details = [str(error)]
    for name in names:
        path = Path(directory) / name
        if path.is_file():
            with path.open("rb") as stream:
                stream.seek(0, 2)
                stream.seek(max(0, stream.tell() - 4096))
                tail = stream.read().decode(errors="replace").strip()
            if tail:
                details.append(f"{name}:\n{tail}")
    return "\n".join(details)
