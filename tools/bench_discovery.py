"""Measure first-source commits while a different Miniflux discovery is slow."""
import argparse
from contextlib import ExitStack, contextmanager
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]


@contextmanager
def endpoint(host, role, feeds, delay):
    requests = []
    replied = threading.Event()

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            if role == "publisher":
                body = (ROOT / "tests/fixtures/rss.xml").read_bytes()
            elif self.headers.get("X-Auth-Token") != "bench-token":
                self.send_error(401)
                return
            elif role == "slow":
                time.sleep(delay)
                body = b"[]"
            elif self.path == "/v1/feeds":
                body = json.dumps([dict(id=index, title=f"Feed {index}",
                    feed_url=f"https://publisher.example/{index}")
                    for index in range(1, feeds + 1)]).encode()
            elif self.path.startswith("/v1/feeds/"):
                body = b'{"total":0,"entries":[]}'
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
            replied.set()

        def log_message(self, *_args):
            pass

    server = HTTPServer((host, 0), Handler)
    worker = threading.Thread(target=server.serve_forever,
                              kwargs={"poll_interval": 0.01}, daemon=True)
    worker.start()
    try:
        yield f"http://{host}:{server.server_port}", requests, replied
    finally:
        server.shutdown()
        server.server_close()
        worker.join(timeout=5)


def prepare(binary, directory, addresses):
    data = directory / "data"
    config = directory / "config.toml"
    slow, fast, publisher = addresses
    config.write_text(f'''schema_version = 1
[[servers]]
id = "slow"
url = "{slow}"
token = "bench-token"
[[servers]]
id = "fast"
url = "{fast}"
token = "bench-token"
[[feeds]]
id = "publisher"
url = "{publisher}"
''')
    command = [str(binary), "--config", str(config), "--data-dir", str(data)]
    subprocess.run(command + ["feeds"], check=True,
                   capture_output=True, text=True, timeout=15)
    return command, data / "library.sqlite3"


def observe(process, database, count, slow_replied):
    query = (ROOT / "src/store/sql/select_source_health.sql").read_text()
    first = complete = None
    before_reply = False
    started = time.perf_counter()
    with sqlite3.connect(database, isolation_level=None) as connection:
        while process.poll() is None:
            healthy = sum(row[3] > 0 for row in connection.execute(query))
            elapsed = (time.perf_counter() - started) * 1000
            if healthy and first is None:
                first = elapsed
            if healthy == count and complete is None:
                complete = elapsed
                before_reply = not slow_replied.is_set()
            if elapsed > 15_000:
                process.kill()
                raise RuntimeError("Collection exceeded fixture deadline")
            time.sleep(0.002)
        healthy = sum(row[3] > 0 for row in connection.execute(query))
    output, error = process.communicate(timeout=5)
    if process.returncode or healthy != count:
        raise RuntimeError(f"Collection failed: {output!r} {error!r}")
    finished = (time.perf_counter() - started) * 1000
    return dict(first_source_ms=first if first is not None else finished,
                fast_sources_ms=complete if complete is not None else finished,
                fast_complete_before_slow_reply=before_reply,
                total_ms=finished, healthy_sources=healthy)


def sample(binary, feeds, delay):
    with tempfile.TemporaryDirectory(prefix="lume-discovery-bench-") as temp:
        with ExitStack() as stack:
            endpoints = [stack.enter_context(endpoint(host, role, feeds, delay))
                         for host, role in [("127.0.0.1", "slow"),
                                            ("127.0.0.2", "fast"),
                                            ("127.0.0.3", "publisher")]]
            command, database = prepare(binary, Path(temp),
                                        [entry[0] for entry in endpoints])
            process = subprocess.Popen(command + ["sync"], stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE, text=True)
            try:
                result = observe(process, database, feeds + 1, endpoints[0][2])
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate(timeout=5)
            counts = [len(entry[1]) for entry in endpoints]
            if counts != [1, feeds + 1, 1]:
                raise RuntimeError(f"Unexpected request coverage: {counts}")
            return result | {"http_requests": sum(counts)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--feeds", type=int, default=100)
    parser.add_argument("--delay-ms", type=int, default=800)
    arguments = parser.parse_args()
    if not 1 <= arguments.feeds <= 5000:
        parser.error("--feeds must be 1–5000")
    if not 0 <= arguments.delay_ms <= 5000:
        parser.error("--delay-ms must be 0–5000")
    records = []
    for repeat in range(3):
        order = ["before", "after"] if repeat != 1 else ["after", "before"]
        for version in order:
            result = sample(getattr(arguments, version).resolve(),
                            arguments.feeds, arguments.delay_ms / 1000)
            records.append({"version": version, **result})
            arguments.output.write_text(json.dumps({"records": records}, indent=2) + "\n")
            print(version, json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
