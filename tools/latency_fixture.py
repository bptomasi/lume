"""Owned library and loopback publisher for Foot latency qualification."""

import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sqlite3
import subprocess
import threading
import time

ADDRESS = "https://example.org/request-path.svg"
SQL = Path(__file__).resolve().parents[1] / "src/store/sql"


class Publisher:
    def __init__(self):
        self.release = threading.Event()
        self.started = threading.Event()
        self.requests = []
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                record = {"start_ns": time.monotonic_ns(), "path": self.path}
                owner.requests.append(record)
                owner.started.set()
                if not owner.release.wait(10):
                    record["timed_out"] = True
                body = (
                    '<?xml version="1.0"?><rss version="2.0"><channel>'
                    '<title>Latency publisher</title><link>https://example.org</link>'
                    '<description>Owned fixture</description><item>'
                    f'<guid>collected-{self.path}</guid><title>Collected arrival</title>'
                    '<link>https://example.org/collected</link>'
                    '<description>A collected systems article.</description>'
                    '</item></channel></rss>'
                ).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/rss+xml")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                record["end_ns"] = time.monotonic_ns()

            def log_message(self, *_arguments):
                return

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def close(self):
        self.release.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)

    @property
    def address(self):
        return f"http://127.0.0.1:{self.server.server_port}"


def body(name):
    return (
        f"# {name} HEADMARK @@@@@@@@@@@@\n\n"
        + ("QQQQQQQQQQQQ" if name == "ALPHA" else "XXXXXXXXXXXX")
        + " CONTENTREADY\n\n"
        + f"![Request path]({ADDRESS})\n\n"
        + "A system has boundaries, ownership and failure modes.\n\n" * 120
        + f"{name} TAILMARK %%%%%%%%%%%%\n\n![Request path]({ADDRESS})\n"
    )


def seed(binary, directory, publisher, stress):
    library = directory / "data/lume"
    library.mkdir(parents=True)
    subprocess.run([str(binary), "--data-dir", str(library), "feeds"],
                   check=True, capture_output=True, timeout=30)
    count = 50_000 if stress else 10_000
    with sqlite3.connect(library / "library.sqlite3") as connection:
        connection.executemany(
            "INSERT INTO feeds (url, title) VALUES (?, ?)",
            [
                (f"{publisher.address}/feed/{index}", f"Source {index}")
                for index in range(16)
            ],
        )
        connection.executemany(
            (SQL / "test_insert_latency_article.sql").read_text(),
            [(1, f"old-{index}", f"Archive {index}", "https://example.org/archive",
              "Fixture", 1_700_000_000 + index, "Systems and resource ownership.")
             for index in range(count)],
        )
        connection.executemany(
            (SQL / "test_insert_latency_article.sql").read_text(),
            [(1, name, name, f"https://example.org/{name}", "Fixture",
              9_000_000_001 - index, body(name))
             for index, name in enumerate(("ALPHA", "BRAVO"))],
        )
        connection.executemany(
            "INSERT INTO smart_lists (name, definition) VALUES (?, ?)",
            [(f"Systems {index}", '{"text":"systems","unread":true}')
             for index in range(64)],
        )
    cache = library / "images"
    cache.mkdir()
    name = hashlib.sha256(ADDRESS.encode()).hexdigest() + ".img"
    (cache / name).write_bytes(
        (SQL.parents[2] / "tests/fixtures/images/request-path.svg").read_bytes()
    )
    return {"articles": count + 2, "feeds": 16, "smart_lists": 64}
