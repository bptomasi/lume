"""Qualify installation and legacy migration without touching user libraries."""

from contextlib import closing, redirect_stdout
import io
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from tools import install as installer


SQL = installer.PROJECT / "src/store/sql"


def seed_library(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(path)
    connection.executescript((SQL / "create_schema.sql").read_text())
    connection.executescript(
        (SQL / "test_setup_install_library.sql").read_text()
    )
    return connection


class InstallationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lume-install-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "legacy library.sqlite3"
        self.target = self.root / "data/library.sqlite3"

    def test_wal_snapshot_keeps_committed_notes_and_original_library(self):
        with closing(seed_library(self.source)) as writer:
            writer.execute("PRAGMA journal_mode = WAL")
            writer.execute("PRAGMA wal_autocheckpoint = 0")
            writer.execute("UPDATE articles SET title = 'Committed title'")
            writer.commit()
            writer.execute("UPDATE articles SET title = 'Uncommitted title'")
            original = self.source.read_bytes()
            wal = Path(str(self.source) + "-wal")
            committed_wal = wal.read_bytes()

            self.assertTrue(installer.migrate_library(self.source, self.target))

            with closing(sqlite3.connect(self.target)) as migrated:
                self.assertEqual(
                    migrated.execute(
                        "SELECT title, unread, starred FROM articles"
                    ).fetchall(),
                    [("Committed title", 0, 1)],
                )
                self.assertEqual(
                    migrated.execute("SELECT body FROM comments").fetchall(),
                    [("A personal annotation that must survive migration.",)],
                )
                self.assertEqual(
                    migrated.execute(
                        "SELECT collection_id, article_id FROM members"
                    ).fetchall(),
                    [(7, 42)],
                )
                self.assertEqual(
                    migrated.execute("PRAGMA integrity_check").fetchone(),
                    ("ok",),
                )
            self.assertEqual(self.source.read_bytes(), original)
            self.assertEqual(wal.read_bytes(), committed_wal)
            writer.rollback()
        self.assertTrue(self.source.is_file())
        self.assertEqual(self.target.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.target.parent.stat().st_mode & 0o777, 0o700)
        leftovers = list(self.target.parent.glob(".library-backup-*"))
        self.assertFalse(leftovers)

    def test_existing_library_is_never_overwritten(self):
        self.target.parent.mkdir()
        self.target.write_bytes(b"Existing user data")

        self.assertFalse(installer.migrate_library(self.source, self.target))

        self.assertEqual(self.target.read_bytes(), b"Existing user data")

    def test_corrupt_source_leaves_no_destination_or_temporary_files(self):
        self.source.write_bytes(b"This is not a SQLite database")

        with self.assertRaises(sqlite3.DatabaseError):
            installer.migrate_library(self.source, self.target)

        self.assertFalse(self.target.exists())
        self.assertEqual(list(self.target.parent.iterdir()), [])
        self.assertTrue(self.source.is_file())

    def test_concurrently_created_target_wins(self):
        with closing(seed_library(self.source)):
            pass

        def publish_competing_library(_temporary, target):
            target.write_bytes(b"Another process created this library")
            raise FileExistsError(target)

        with patch.object(installer.os, "link", publish_competing_library):
            migrated = installer.migrate_library(self.source, self.target)
            self.assertFalse(migrated)

        self.assertEqual(
            self.target.read_bytes(), b"Another process created this library"
        )
        self.assertFalse(list(self.target.parent.glob(".library-backup-*")))

    def test_failed_executable_copy_preserves_previous_installation(self):
        executable = self.root / "lume"
        executable.write_bytes(b"Previous executable")
        with patch.object(installer.shutil, "copyfile", side_effect=OSError):
            with self.assertRaises(OSError):
                installer.install_binary(self.source, executable)

        self.assertEqual(executable.read_bytes(), b"Previous executable")
        self.assertFalse(list(self.root.glob(".lume-*")))

    def test_legacy_detection_requires_a_choice_for_distinct_libraries(self):
        project = self.root / "project"
        home = self.root / "home"
        old_home = home / ".local/share/lume/library.sqlite3"
        old_project = project / ".local/library.sqlite3"
        self.assertIsNone(installer.legacy_library(project, home, {}))
        with closing(seed_library(old_home)):
            pass
        environment = {"XDG_DATA_HOME": str(home / ".local/share")}
        self.assertEqual(
            installer.legacy_library(project, home, environment), old_home
        )
        with closing(seed_library(old_project)):
            pass

        with self.assertRaisesRegex(ValueError, "MIGRATE_FROM"):
            installer.legacy_library(project, home, environment)

        self.assertEqual(
            installer.legacy_library(project, home, environment, old_project),
            old_project,
        )

    def test_backup_deadline_preserves_the_existing_source(self):
        with closing(seed_library(self.source)):
            pass

        with self.assertRaises(TimeoutError):
            installer.backup_library(self.source, self.root / "snapshot", 0)

        with closing(sqlite3.connect(self.source)) as original:
            self.assertEqual(
                original.execute("SELECT count(*) FROM articles").fetchone(),
                (1,),
            )

    def test_install_and_reinstall_keep_data_and_distribute_notices(self):
        binary = self.root / "release executable"
        binary.write_bytes(b"Fixture executable")
        prefix = self.root / "local prefix"
        with closing(seed_library(self.source)):
            pass
        arguments = (
            binary, prefix, self.target.parent, installer.PROJECT,
            self.root / "home", {}, self.source,
        )
        with redirect_stdout(io.StringIO()):
            self.assertTrue(installer.install(*arguments))
        installed = prefix / "bin/lume"
        self.assertEqual(installed.read_bytes(), b"Fixture executable")
        self.assertEqual(installed.stat().st_mode & 0o777, 0o755)
        licenses = prefix / "share/lume/licenses"
        self.assertEqual(
            (licenses / "LICENSE").read_bytes(),
            (installer.PROJECT / "LICENSE").read_bytes(),
        )
        self.assertTrue((licenses / "full-article/NOTICES.md").is_file())
        self.assertTrue((licenses / "images/NOTICES.md").is_file())
        self.assertEqual(
            (licenses / "images/THIRD_PARTY_LICENSES.txt").read_bytes(),
            (installer.PROJECT / "assets/licenses/images/THIRD_PARTY_LICENSES.txt")
            .read_bytes(),
        )
        for name in ("NOTICES.md", "THIRD_PARTY_LICENSES.txt"):
            self.assertEqual(
                (licenses / "configuration" / name).read_bytes(),
                (installer.PROJECT / "assets/licenses/configuration" / name)
                .read_bytes(),
            )
        self.assertTrue((licenses / "syntax/PROVENANCE.md").is_file())
        with closing(sqlite3.connect(self.target)) as library:
            library.execute("UPDATE comments SET body = 'Personal edit'")
            library.commit()

        with redirect_stdout(io.StringIO()):
            self.assertFalse(installer.install(*arguments))

        with closing(sqlite3.connect(self.target)) as library:
            self.assertEqual(
                library.execute("SELECT body FROM comments").fetchone(),
                ("Personal edit",),
            )

    def test_make_install_builds_locked_and_honors_environment_directory(self):
        cargo = self.root / "cargo-fixture"
        cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$CARGO_LOG"\n')
        cargo.chmod(0o755)
        binary = self.root / "release binary"
        binary.write_bytes(b"Fixture executable")
        with closing(seed_library(self.source)):
            pass
        home = self.root / "home"
        prefix = self.root / "prefix with spaces"
        log = self.root / "cargo-arguments"
        environment = dict(os.environ)
        environment.update(
            HOME=str(home), LUME_DATA_DIR=str(self.target.parent),
            CARGO_LOG=str(log),
        )
        environment.pop("MAKEFLAGS", None)
        environment.pop("MFLAGS", None)

        output = subprocess.run(
            ["make", "install", f"CARGO={cargo}", f"BINARY={binary}",
             f"PREFIX={prefix}", f"MIGRATE_FROM={self.source}"],
            cwd=installer.PROJECT, env=environment, capture_output=True,
            text=True, timeout=15, check=False,
        )

        self.assertEqual(output.returncode, 0, output.stderr + output.stdout)
        self.assertEqual(log.read_text().splitlines(), [
            "build", "--locked", "--release",
        ])
        self.assertEqual(
            (prefix / "bin/lume").read_bytes(), binary.read_bytes()
        )
        self.assertTrue(self.target.parent.is_dir())
        self.assertFalse((home / ".local/lume").exists())

    def test_empty_path_is_rejected_before_writing(self):
        output = subprocess.run(
            [sys.executable, str(installer.PROJECT / "tools/install.py"),
             "--binary", str(self.source), "--prefix", str(self.root),
             "--data-dir", ""],
            capture_output=True, text=True, timeout=5, check=False,
        )
        self.assertNotEqual(output.returncode, 0)
        self.assertIn("must not be empty", output.stderr)
        self.assertEqual(list(self.root.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
