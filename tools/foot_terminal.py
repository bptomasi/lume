"""Relay an owned Foot PTY for deterministic input and output observation.

The parent driver writes raw input to a private FIFO. Terminal replies and
human input use the same relay; stdout is copied to Foot unchanged. Only the
fresh qualification child and its terminal settings are observed.
"""

import errno
import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import tty

from qualify_alacritty import write_json


def relay(binary, directory):
    before = termios.tcgetattr(sys.stdin.fileno())
    master, slave = pty.openpty()
    inner_before = termios.tcgetattr(slave)
    control = directory / "input.fifo"
    os.mkfifo(control, 0o600)
    commands = os.open(control, os.O_RDWR | os.O_NONBLOCK)
    size = fcntl.ioctl(sys.stdin.fileno(), termios.TIOCGWINSZ, bytes(8))
    fcntl.ioctl(slave, termios.TIOCSWINSZ, size)
    process = subprocess.Popen(
        [str(binary), "--offline"], stdin=slave, stdout=slave, stderr=slave,
        start_new_session=True,
    )
    rows, columns, width, height = struct.unpack("HHHH", size)
    write_json(directory / "terminal.json", {
        "pid": process.pid, "columns": columns, "rows": rows,
        "cell_pixels": [width // columns, height // rows],
    })
    try:
        tty.setraw(sys.stdin.fileno())
        with (directory / "terminal-output.bin").open("wb", buffering=0) as capture:
            pump(process, master, commands, capture)
        returncode = process.wait(timeout=10)
        restored = termios.tcgetattr(slave) == inner_before
    finally:
        termios.tcsetattr(sys.stdin.fileno(), termios.TCSANOW, before)
        for descriptor in (master, slave, commands):
            os.close(descriptor)
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
    write_json(directory / "exit.json", {
        "returncode": returncode, "termios_restored": restored,
    })
    return 0 if returncode == 0 and restored else 1


def pump(process, master, commands, capture):
    while True:
        ready, _, _ = select.select([sys.stdin.fileno(), master, commands], [], [], 0.1)
        for descriptor in ready:
            try:
                data = os.read(descriptor, 65536)
            except OSError as error:
                if descriptor == master and error.errno == errno.EIO:
                    return
                raise
            if not data and descriptor == master:
                return
            if descriptor == master:
                capture.write(data)
                write_all(sys.stdout.fileno(), data)
            elif data:
                write_all(master, data)
        if process.poll() is not None and master not in ready:
            return


def write_all(descriptor, data):
    view = memoryview(data)
    while view:
        count = os.write(descriptor, view)
        if count == 0:
            raise OSError("Terminal relay made no write progress")
        view = view[count:]
