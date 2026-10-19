"""Install Lume and migrate a legacy library with SQLite's backup API.

The installer copies a committed snapshot, including WAL content, and keeps
the source. An existing destination always wins. A same-directory hard link
publishes the completed snapshot without overwriting a concurrently created
library. Runtime reads only the configured data directory.
"""

import argparse
from contextlib import closing
import os
from pathlib import Path
import shutil
import sqlite3
import sys
import tempfile
import time


PROJECT = Path(__file__).resolve().parents[1]


def legacy_library(project, home, environment, requested=None):
    if requested is not None:
        if not requested.is_file():
            raise ValueError(f"Migration source is not a file: {requested}")
        return requested.resolve()
    candidates = [
        project / ".local/library.sqlite3",
        home / ".local/share/lume/library.sqlite3",
    ]
    configured = environment.get("XDG_DATA_HOME")
    if configured:
        candidates.append(Path(configured) / "lume/library.sqlite3")
    found = sorted({path.resolve() for path in candidates if path.is_file()})
    if len(found) > 1:
        names = ", ".join(str(path) for path in found)
        raise ValueError(
            f"Multiple legacy libraries found: {names}. "
            "Choose one with make install MIGRATE_FROM=/absolute/path"
        )
    return found[0] if found else None


def backup_library(source, temporary, timeout=10):
    deadline = time.monotonic() + timeout

    def check_deadline(_status, _remaining, _total):
        if time.monotonic() >= deadline:
            raise TimeoutError("Library backup timed out; close Lume and retry")

    uri = source.resolve().as_uri() + "?mode=ro"
    with closing(sqlite3.connect(uri, uri=True)) as origin:
        with closing(sqlite3.connect(temporary)) as destination:
            origin.backup(
                destination, pages=256, progress=check_deadline, sleep=0.05,
            )


def migrate_library(source, target):
    if target.exists() or target.is_symlink() or source is None:
        return False
    target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(
        prefix=".library-backup-", dir=target.parent,
    )
    os.close(descriptor)
    temporary = Path(name)
    try:
        backup_library(source, temporary)
        try:
            os.link(temporary, target)
        except FileExistsError:
            return False
        return True
    finally:
        temporary.unlink(missing_ok=True)


def install_binary(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix=".lume-", dir=target.parent)
    os.close(descriptor)
    temporary = Path(name)
    try:
        shutil.copyfile(source, temporary)
        temporary.chmod(0o755)
        os.replace(temporary, target)
    finally:
        temporary.unlink(missing_ok=True)


def install_notices(project, prefix):
    destination = prefix / "share/lume/licenses"
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(project / "LICENSE", destination / "LICENSE")
    shutil.copytree(
        project / "assets/licenses/full-article",
        destination / "full-article",
        dirs_exist_ok=True,
    )
    syntax = destination / "syntax"
    syntax.mkdir(exist_ok=True)
    for source in (project / "assets/syntax").iterdir():
        if "license" in source.name.lower() or source.name == "PROVENANCE.md":
            shutil.copyfile(source, syntax / source.name)


def install(
    binary, prefix, data_dir, project, home, environment, requested=None,
):
    if not binary.is_file():
        raise ValueError(f"Build the release binary first: {binary}")
    target = data_dir / "library.sqlite3"
    migrated = False
    if not target.exists() and not target.is_symlink():
        source = legacy_library(project, home, environment, requested)
        migrated = migrate_library(source, target)
    data_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    install_notices(project, prefix)
    executable = prefix / "bin/lume"
    install_binary(binary, executable)
    print(f"Installed executable: {executable}")
    print(f"Library: {target}")
    if migrated:
        print("Copied the legacy library; its original remains unchanged.")
    return migrated


def path_argument(value):
    if not value:
        raise argparse.ArgumentTypeError("The path must not be empty")
    return Path(value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=path_argument)
    parser.add_argument("--prefix", required=True, type=path_argument)
    parser.add_argument("--data-dir", required=True, type=path_argument)
    parser.add_argument("--migrate-from", type=path_argument)
    arguments = parser.parse_args()
    try:
        install(
            arguments.binary.resolve(),
            arguments.prefix.resolve(),
            arguments.data_dir.resolve(),
            PROJECT,
            Path.home(),
            os.environ,
            arguments.migrate_from,
        )
    except (OSError, ValueError, sqlite3.Error) as error:
        print(f"Installation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
