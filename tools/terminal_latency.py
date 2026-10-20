"""An isolated relay probe measures input to a matching frame parsed by Foot.

DSR 5 queries terminal status after synchronized output. The reply acknowledges
VT parsing, not Wayland presentation or physical display latency. Only this
probe requests DSR 5; unrelated replies remain on the application's PTY.
"""

import base64
import json
import os
import time

BEGIN = b"\x1b[?2026h"
END = b"\x1b[?2026l"
STATUS = b"\x1b[5n"
ACK = b"\x1b[0n"
MAX_FRAME_BYTES = 16 * 1024 * 1024


class Probe:
    def __init__(self, directory, clock=time.monotonic_ns):
        self.clock = clock
        self.control = directory / "latency.fifo"
        os.mkfifo(self.control, 0o600)
        self.descriptor = os.open(self.control, os.O_RDWR | os.O_NONBLOCK)
        self.log = (directory / "latency.jsonl").open("w", buffering=1)
        self.command = bytearray()
        self.frame = bytearray()
        self.reply = bytearray()
        self.pending = None
        self.last_id = -1

    def close(self):
        os.close(self.descriptor)
        self.log.close()

    def receive(self, data, send):
        self.command.extend(data)
        if len(self.command) > 4096:
            raise RuntimeError("Latency request exceeds 4 KiB")
        while b"\n" in self.command:
            line, _, tail = self.command.partition(b"\n")
            self.command = bytearray(tail)
            request = json.loads(line)
            if self.pending or request["id"] <= self.last_id:
                raise RuntimeError("Latency requests must be sequential")
            payload = base64.b64decode(request["input"], validate=True)
            expected = request["expected"].encode("ascii")
            if not payload or not expected:
                raise RuntimeError("Latency input and expected feedback are required")
            self.last_id = request["id"]
            self.pending = dict(request, admitted_ns=self.clock(), output_bytes=0)
            self.frame.clear()
            self.reply.clear()
            send(payload)

    def output(self, data, query):
        if not self.pending or "frame_ns" in self.pending:
            return
        self.pending["output_bytes"] += len(data)
        self.frame.extend(data)
        if len(self.frame) > MAX_FRAME_BYTES:
            raise RuntimeError("Latency frame exceeds 16 MiB")
        while END in self.frame:
            end = self.frame.index(END)
            start = self.frame.rfind(BEGIN, 0, end)
            content = self.frame[start + len(BEGIN):end] if start >= 0 else b""
            del self.frame[:end + len(END)]
            expected = self.pending["expected"].encode("ascii")
            native = b"\x1bP" in content
            if expected not in content or (self.pending.get("native") and not native):
                continue
            self.pending.update(frame_ns=self.clock(), sixel_in_frame=native)
            query(STATUS)
            return

    def response(self, data):
        if not self.pending or "frame_ns" not in self.pending:
            return data
        self.reply.extend(data)
        position = self.reply.find(ACK)
        if position < 0:
            keep = min(len(ACK) - 1, len(self.reply))
            forwarded = bytes(self.reply[:-keep]) if keep else bytes(self.reply)
            if keep:
                del self.reply[:-keep]
            return forwarded
        finished = self.clock()
        forwarded = bytes(self.reply[:position] + self.reply[position + len(ACK):])
        record = self.pending
        record.update(ack_ns=finished)
        record["input_to_frame_ms"] = (record["frame_ns"] - record["sent_ns"]) / 1e6
        record["input_to_terminal_ack_ms"] = (finished - record["sent_ns"]) / 1e6
        record["relay_admission_ms"] = (record["admitted_ns"] - record["sent_ns"]) / 1e6
        self.log.write(json.dumps(record) + "\n")
        self.pending = None
        self.reply.clear()
        return forwarded
