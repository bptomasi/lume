#!/usr/bin/env python3
"""Deterministic foreground helper fixture; no display server is opened."""

import json
import os
from pathlib import Path
import struct
import sys
import time

root = Path(__file__).parent
mode = (root / "mode").read_text().strip()


def record(value):
    with (root / "journal.jsonl").open("a") as journal:
        journal.write(json.dumps(value) + "\n")


record({"event": "started", "pid": os.getpid(), "arguments": sys.argv[1:]})
if mode == "exit":
    sys.exit(23)
if mode == "stall":
    time.sleep(10)
    sys.exit(0)

for line in sys.stdin:
    command = json.loads(line)
    if command["action"] == "add":
        path = Path(command["path"])
        with path.open("rb") as source:
            header = source.read(24)
        if header[:8] != b"\x89PNG\r\n\x1a\n":
            raise RuntimeError("Only prepared PNG files may reach the helper")
        command["pixel_dimensions"] = list(struct.unpack(">II", header[16:24]))
    record(command)
    if mode == "exit_after_frame":
        sys.exit(29)
