"""Qualify release identity, bytes and publication failure boundaries offline."""

import hashlib
import importlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch


with patch.object(sys, "path", [str(Path(__file__).resolve().parents[1]), *sys.path]):
    release = importlib.import_module("publish_release")

REVISION = "a" * 40
TAG = "v0.2.0"
REPOSITORY = "example/lume"


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="lume-release-")
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        (self.directory / "Cargo.toml").write_text(
            '[package]\nname = "lume"\nversion = "0.2.0"\n')
        (self.directory / "Cargo.lock").write_text(
            '[[package]]\nname = "lume"\nversion = "0.2.0"\n')

    def package(self, **changes):
        binary = b"qualified executable"
        metadata = dict(version="0.2.0", revision=REVISION,
                        target=release.TARGET, source_dirty=False,
                        binary_sha256=hashlib.sha256(binary).hexdigest())
        metadata.update(changes)
        name = f"lume-0.2.0-{release.TARGET}"
        archive = self.directory / f"{name}.tar.gz"
        with tarfile.open(archive, "w:gz") as bundle:
            for suffix, data in [("BUILD.json", json.dumps(metadata).encode()),
                                 ("bin/lume", binary)]:
                member = tarfile.TarInfo(f"{name}/{suffix}")
                member.size = len(data)
                bundle.addfile(member, io.BytesIO(data))
        checksum = archive.with_suffix(".gz.sha256")
        checksum.write_text(f"{release.digest(archive)}  {archive.name}\n")
        return [archive, checksum]

    def assets(self, files, draft=True):
        return dict(draft=draft, assets=[dict(
            name=file.name, digest=f"sha256:{release.digest(file)}"
        ) for file in files])

    def test_tags_require_exact_semantic_version(self):
        for version in ("0.2.0", "1.0.0", "0.3.0-rc.1", "1.2.3-beta-2"):
            release.validate_tag(f"v{version}", version)
        for tag in ("0.2.0", "v00.2.0", "v0.02.0", "v0.2.0-01",
                    "v0.2.0-rc..1", "v0.2.0-", "v0.2.0+build", "v0.3.0"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate_tag(tag, "0.2.0")

    def test_package_requires_clean_matching_revision_and_binary(self):
        files = self.package()
        self.assertEqual(release.verify_package(self.directory, TAG, REVISION), files)
        for change in (dict(source_dirty=True), dict(revision="b" * 40),
                       dict(version="0.1.0"), dict(target="other"),
                       dict(binary_sha256="0" * 64)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.package(**change)
                release.verify_package(self.directory, TAG, REVISION)

    def test_corrupt_archive_and_incomplete_assets_are_rejected(self):
        files = self.package()
        files[0].write_bytes(b"corrupted archive")
        with self.assertRaisesRegex(ValueError, "checksum"):
            release.verify_package(self.directory, TAG, REVISION)
        files = self.package()
        for metadata in (None, dict(assets=[]),
                         dict(assets=[dict(name=file.name) for file in files])):
            with self.subTest(metadata=metadata), self.assertRaises(ValueError):
                release.verify_assets(metadata, files)

    def test_draft_is_published_only_after_uploaded_digests_are_verified(self):
        files = self.package()
        metadata = [None, self.assets(files)]
        with patch.object(release, "release_metadata", side_effect=metadata):
            with patch.object(release, "github") as github:
                github.return_value.stdout = REVISION + "\n"
                release.publish(REPOSITORY, TAG, REVISION, files)
        commands = [call.args for call in github.call_args_list]
        self.assertEqual(commands[0][0], "api")
        self.assertEqual([command[1] for command in commands[1:]],
                         ["create", "upload", "edit"])
        self.assertIn("--draft", commands[1])
        self.assertIn("--draft=false", commands[-1])
        self.assertIn("--latest=true", commands[-1])

    def test_moved_tag_or_incomplete_upload_never_publishes(self):
        files = self.package()
        for revision, metadata in [("b" * 40, []),
                                   (REVISION, [None, dict(assets=[])])]:
            with patch.object(release, "release_metadata", side_effect=metadata):
                with patch.object(release, "github") as github:
                    github.return_value.stdout = revision
                    with self.assertRaises(ValueError):
                        release.publish(REPOSITORY, TAG, REVISION, files)
            self.assertFalse(any(call.args[:2] == ("release", "edit")
                                 for call in github.call_args_list))

    def test_existing_release_accepts_identical_bytes_without_writes(self):
        files = self.package()
        with patch.object(release, "release_metadata",
                          return_value=self.assets(files, draft=False)):
            with patch.object(release, "github") as github:
                github.return_value.stdout = REVISION
                release.publish(REPOSITORY, TAG, REVISION, files)
                github.assert_called_once()
        metadata = self.assets(files)
        files[0].write_bytes(b"different executable")
        with self.assertRaises(ValueError):
            release.verify_assets(metadata, files)

    def test_api_errors_cannot_be_treated_as_missing_releases(self):
        result = subprocess.CompletedProcess([], 1, "", "HTTP 403 forbidden")
        with patch.object(release, "github", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "HTTP 403"):
                release.release_metadata(REPOSITORY, TAG)

    def test_validation_does_not_contact_github(self):
        self.package()
        arguments = ["publish_release.py", "--tag", TAG, "--revision", REVISION,
                     "--artifacts", str(self.directory)]
        with (patch.object(sys, "argv", arguments),
              patch.object(release, "ROOT", self.directory),
              patch.object(release, "github") as github):
            release.main()
            github.assert_not_called()

    def test_lockfile_version_disagreement_stops_before_network(self):
        (self.directory / "Cargo.lock").write_text(
            '[[package]]\nname = "lume"\nversion = "0.1.0"\n')
        with (patch.object(sys, "argv", ["release", "--tag", TAG, "--check-tag-only"]),
              patch.object(release, "ROOT", self.directory),
              patch.object(sys, "stderr", io.StringIO()),
              patch.object(release, "github") as github,
              self.assertRaises(SystemExit)):
            release.main()
        github.assert_not_called()


if __name__ == "__main__":
    unittest.main()
