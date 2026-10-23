"""Measure correlated input/frame/Foot-ack latency on an isolated Wayland window.

Every sample waits for unique expected publisher text in a synchronized frame,
then asks Foot for device status. This includes PTY and native VT processing;
it does not claim compositor presentation or physical key-to-photon latency.
"""

import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

from foot_terminal import relay
from latency_fixture import Publisher, body, seed
from terminal_support import wait_for, write_json
from qualify_foot import command, enter_library, frame, send


def percentile(values, quantile):
    ordered = sorted(values)
    if not ordered:
        raise ValueError("A percentile requires samples")
    return ordered[max(0, math.ceil(quantile * len(ordered)) - 1)]


def records(directory):
    path = directory / "latency.jsonl"
    if not path.exists():
        return []
    lines = path.read_text().splitlines(keepends=True)
    return [json.loads(line) for line in lines if line.endswith("\n")]


def sample(directory, process, scenario, index, payload, expected, native=False):
    request = {
        "id": index,
        "scenario": scenario,
        "sent_ns": time.monotonic_ns(),
        "input": base64.b64encode(payload).decode(),
        "expected": expected,
        "native": native,
    }
    packet = json.dumps(request).encode() + b"\n"
    descriptor = os.open(directory / "latency.fifo", os.O_WRONLY | os.O_NONBLOCK)
    try:
        if os.write(descriptor, packet) != len(packet):
            raise RuntimeError("Incomplete private latency request")
    finally:
        os.close(descriptor)

    def completed():
        results = records(directory)
        return results[-1] if results and results[-1]["id"] == index else None

    return wait_for(f"{scenario} sample {index}", completed, process, seconds=5)


def scroll_samples(directory, process, scenario, count, start, publisher=None):
    results = []
    for index in range(count):
        if publisher and publisher.release.is_set():
            raise RuntimeError("Publisher was released during collection samples")
        payload, expected = (
            (b"G", "%%%%%%%%%%%%") if index % 2 == 0
            else (b"gg", "@@@@@@@@@@@@")
        )
        result = sample(
            directory, process, scenario, start + index,
            payload, expected, native=True,
        )
        if publisher:
            if any(request.get("timed_out") for request in publisher.requests):
                raise RuntimeError("Loopback publisher exceeded its bounded hold")
            active = any(request["start_ns"] <= result["sent_ns"]
                         and "end_ns" not in request for request in publisher.requests)
            if not active:
                raise RuntimeError("Collection workload had no active HTTP request")
        results.append(result)
    return results


def switch_samples(directory, process, count, start):
    results = []
    for index in range(count):
        payload = b"j" if index % 2 == 0 else b"k"
        expected = "XXXXXXXXXXXX" if index % 2 == 0 else "QQQQQQQQQQQQ"
        results.append(sample(directory, process, "article-switch", start + index,
                              payload, expected))
    return results


def summarize(samples):
    groups = {}
    for sample_ in samples:
        groups.setdefault(sample_["scenario"], []).append(sample_)
    result = {}
    for scenario, items in groups.items():
        metrics = {}
        for metric in (
            "input_to_frame_ms", "input_to_terminal_ack_ms", "relay_admission_ms",
        ):
            values = [item[metric] for item in items]
            metrics[metric] = {
                "p50": percentile(values, 0.5),
                "p95": percentile(values, 0.95),
                "p99": percentile(values, 0.99),
                "max": max(values),
            }
        result[scenario] = {
            "samples": len(items),
            "metrics": metrics,
            "output_bytes": sum(item["output_bytes"] for item in items),
        }
    return result


def read_activation(directory, process):
    # Use the negotiated enhanced-key Enter packet, as real Foot does.
    send(directory, b"\x1b[13u")
    database = directory / "data/lume/library.sqlite3"

    def saved():
        with sqlite3.connect(database) as connection:
            row = connection.execute(
                "SELECT unread FROM articles WHERE external_id = ?",
                ("ALPHA",),
            ).fetchone()
        return row == (0,)

    wait_for("automatic read acknowledgement", saved, process)
    time.sleep(0.35)
    expected = "ALPHA HEADMARK @@@@@@@@@@@@"
    request = b"\x1b]52;c;" + base64.b64encode(expected.encode()) + b"\x1b\\"
    path = directory / "terminal-output.bin"
    before = path.stat().st_size
    send(directory, b"ggIVvAy")
    wait_for(
        "read activation retains the exact source",
        lambda: request in path.read_bytes()[before:], process,
    )
    return {"read_saved": True, "retained_heading": expected,
            "host_clipboard": "disabled"}


def process_resources(pid):
    status = (Path("/proc") / str(pid) / "status").read_text().splitlines()
    memory = {}
    for line in status:
        name, _, value = line.partition(":")
        if name in (
            "VmRSS", "VmHWM", "voluntary_ctxt_switches",
            "nonvoluntary_ctxt_switches",
        ):
            memory[name] = value.strip()
    io = (Path("/proc") / str(pid) / "io").read_text().splitlines()
    return {"memory": memory, "io": dict(line.split(": ", 1) for line in io)}


def exercise(directory, process, publisher, count):
    wait_for("Foot startup", lambda: (directory / "terminal.json").exists(), process)
    enter_library(directory, process, "ALPHA HEADMARK @@@@@@@@@@@@")
    wait_for("active collector", publisher.started.is_set, process)
    command(directory, "fullscreen")
    wait_for(
        "initial image",
        lambda: (current := frame(directory)) and current["strips"], process,
    )
    results = scroll_samples(
        directory, process, "scroll-during-collection", count, 0, publisher,
    )
    publisher.release.set()
    wait_for(
        "publisher completion",
        lambda: len(publisher.requests) == 16
        and all("end_ns" in request for request in publisher.requests), process,
    )
    time.sleep(0.3)
    results.extend(scroll_samples(
        directory, process, "cached-image-scroll", count, count,
    ))
    # Restore the normal workspace and measure source previews from titles.
    send(directory, b"\x1b")
    command(directory, "panel-focus articles")
    time.sleep(0.15)
    results.extend(switch_samples(directory, process, count, 2 * count))
    activation = read_activation(directory, process)
    geometry = json.loads((directory / "terminal.json").read_text())
    resources = {
        "lume": process_resources(geometry["pid"]),
        "foot": process_resources(process.pid),
    }
    command(directory, "quit")
    process.wait(timeout=10)
    cleanup = json.loads((directory / "exit.json").read_text())
    if cleanup != {"returncode": 0, "termios_restored": True}:
        raise RuntimeError(f"Terminal cleanup failed: {cleanup}")
    return results, cleanup, resources, activation


def launch(binary, directory, count, stress):
    if not shutil.which("foot") or not os.environ.get("WAYLAND_DISPLAY"):
        raise RuntimeError("Real Foot on native Wayland is required")
    publisher = Publisher()
    process = None
    try:
        corpus = seed(binary, directory, publisher, stress)
        config = directory / "foot.ini"
        config.write_text(
            "[main]\nfont=monospace:size=12\n"
            "initial-window-size-chars=120x40\npad=0x0\n"
            "[security]\nosc52=disabled\n"
        )
        environment = dict(os.environ, LUME_DATA_DIR=str(directory / "data/lume"),
                           LUME_CONFIG=str(directory / "config.toml"))
        with (directory / "foot.log").open("w") as log:
            process = subprocess.Popen([
                "foot", "--config", str(config), "--title",
                "Lume latency qualification",
                sys.executable, str(Path(__file__).resolve()), "--terminal-child",
                "--binary", str(binary), "--artifacts", str(directory),
            ], env=environment, stdout=log, stderr=log)
            samples, cleanup, resources, activation = exercise(directory, process, publisher, count)
        with sqlite3.connect(directory / "data/lume/library.sqlite3") as db:
            for name in ("ALPHA", "BRAVO"):
                actual = db.execute(
                    "SELECT body FROM articles WHERE external_id = ?", (name,),
                ).fetchone()
                if actual != (body(name),):
                    raise RuntimeError("Latency input modified publisher content")
        return {
            "corpus": corpus,
            "geometry": json.loads((directory / "terminal.json").read_text()),
            "summary": summarize(samples),
            "samples": samples,
            "resources": resources,
            "activation": activation,
            "publisher_requests": publisher.requests,
            "cleanup": cleanup,
        }
    finally:
        publisher.close()
        if process and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/lume"))
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--samples", type=int, default=100)
    parser.add_argument("--stress", action="store_true")
    parser.add_argument("--terminal-child", action="store_true", help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    binary = arguments.binary.resolve(strict=True)
    directory = arguments.artifacts or Path(
        tempfile.mkdtemp(prefix="lume-latency-")
    )
    directory = directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    if arguments.terminal_child:
        return relay(binary, directory, arguments=(), measure=True)
    if arguments.samples < 100:
        raise ValueError("At least 100 samples per workload are required for p99")
    if any(directory.iterdir()):
        raise RuntimeError("Use an empty owned artifact directory")
    report = launch(binary, directory, arguments.samples, arguments.stress)
    report.update(
        passed=True,
        measured_at=datetime.now(timezone.utc).isoformat(),
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        platform=platform.platform(),
        foot_version=subprocess.check_output(
            ["foot", "--version"], text=True,
        ).strip(),
        scope=(
            "Private input injection to a matching frame and Foot DSR 5 "
            "acknowledgement; excludes compositor/physical presentation."
        ),
    )
    write_json(directory / "result.json", report)
    print(f"PASS: real Foot latency; evidence: {directory / 'result.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
