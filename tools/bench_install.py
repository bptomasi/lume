"""Measure committed migration and executable upgrades with checked workloads.

Setup and verification are excluded from each wall-time sample. Peak RSS is
the process high-water mark, including fixture setup, not a backup allocation
delta. Upgrade fixtures represent 16/64 MiB executables and verify exact
binary, library and configuration bytes. Results qualify the installer only,
not application navigation.
"""

from contextlib import closing, redirect_stdout
import json
import hashlib
import io
from pathlib import Path
import platform
import resource
import sqlite3
import sys
import tempfile
import time

import install as installer


def create_library(path, count, body):
    schema = installer.PROJECT / "src/store/sql/create_schema.sql"
    connection = sqlite3.connect(path)
    connection.executescript(schema.read_text())
    connection.execute(
        "INSERT INTO feeds (id, url, title) VALUES (1, ?, ?)",
        ("https://journal.example/rss", "Benchmark source"),
    )
    connection.executemany(
        """
        INSERT INTO articles (
            feed_id, external_id, title, url, author, published, body
        )
        VALUES (1, ?, ?, ?, ?, ?, ?)
        """,
        ((str(index), f"Article {index}", "https://journal.example/article",
          "Author", index, body) for index in range(count)),
    )
    connection.execute(
        "INSERT INTO comments (article_id, body) VALUES (1, ?)",
        ("A personal note",),
    )
    connection.commit()
    connection.execute("PRAGMA journal_mode = WAL")
    connection.execute("UPDATE articles SET starred = 1 WHERE id = 1")
    connection.commit()
    return connection


def verify_snapshot(target, count, body):
    with closing(sqlite3.connect(target)) as library:
        actual = library.execute("SELECT count(*) FROM articles").fetchone()
        article = library.execute(
            "SELECT body, starred FROM articles WHERE id = 1"
        ).fetchone()
        note = library.execute("SELECT body FROM comments").fetchone()
        integrity = library.execute("PRAGMA integrity_check").fetchone()
    if (actual, article, note, integrity) != (
        (count,), (body, 1), ("A personal note",), ("ok",),
    ):
        raise AssertionError("Migration lost publisher or personal data")


def measure(root, name, count, body_bytes):
    directory = root / name
    directory.mkdir()
    source = directory / "source.sqlite3"
    body = "x" * body_bytes
    durations = []
    with closing(create_library(source, count, body)):
        for trial in range(3):
            target = directory / f"trial-{trial}/library.sqlite3"
            started = time.perf_counter()
            migrated = installer.migrate_library(source, target)
            durations.append(round((time.perf_counter() - started) * 1000, 3))
            if not migrated or not source.is_file():
                raise AssertionError("Migration did not preserve its source")
            verify_snapshot(target, count, body)
        size = source.stat().st_size
    return {
        "workload": name,
        "articles": count,
        "body_bytes_each": body_bytes,
        "database_bytes": size,
        "wall_ms": durations,
        "process_peak_rss_kib": (
            resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        ),
    }


def measure_upgrade(root, binary_bytes):
    directory = root / f"upgrade-{binary_bytes}"
    directory.mkdir()
    binary = directory / "release"
    binary.write_bytes(b"Lume executable fixture\n".ljust(binary_bytes, b"x"))
    expected = hashlib.sha256(binary.read_bytes()).digest()
    prefix = directory / "prefix"
    installed = prefix / "bin/lume"
    installed.parent.mkdir(parents=True)
    installed.write_bytes(b"Previous executable")
    data = directory / "data"
    data.mkdir()
    library = data / "library.sqlite3"
    library.write_bytes(b"Owned data: never open during executable upgrade")
    config = data / "config.toml"
    config.write_bytes(b"# Retain exact preference bytes\n")
    durations = []
    for _ in range(3):
        with redirect_stdout(io.StringIO()):
            started = time.perf_counter()
            installer.install(binary, prefix, data, installer.PROJECT,
                              directory, {}, upgrade=True)
            durations.append(round((time.perf_counter() - started) * 1000, 3))
        if hashlib.sha256(installed.read_bytes()).digest() != expected:
            raise AssertionError("Installed binary differs from release")
        if library.read_bytes() != b"Owned data: never open during executable upgrade":
            raise AssertionError("Upgrade modified library bytes")
        if config.read_bytes() != b"# Retain exact preference bytes\n":
            raise AssertionError("Upgrade modified configuration")
    return {"workload": "executable-upgrade", "binary_bytes": binary_bytes,
            "wall_ms": durations,
            "process_peak_rss_kib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss}


def main():
    if sys.platform != "linux":
        raise SystemExit("This RSS benchmark reports Linux KiB units")
    print(json.dumps({
        "environment": platform.platform(),
        "python": platform.python_version(),
        "sqlite": sqlite3.sqlite_version,
    }))
    with tempfile.TemporaryDirectory(prefix="lume-install-bench-") as name:
        root = Path(name)
        workloads = [
            ("representative", 1000, 2048),
            ("stress", 10000, 4096),
        ]
        for workload in workloads:
            print(json.dumps(measure(root, *workload)))
        for size in (16 * 1024 * 1024, 64 * 1024 * 1024):
            print(json.dumps(measure_upgrade(root, size)))


if __name__ == "__main__":
    main()
