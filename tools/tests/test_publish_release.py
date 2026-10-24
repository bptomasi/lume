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
        executable = self.directory / name
        executable.write_bytes(binary)
        executable.chmod(0o755)
        executable_checksum = executable.with_name(name + ".sha256")
        executable_checksum.write_text(f"{release.digest(executable)}  {name}\n")
        return [archive, checksum, executable, executable_checksum]

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

    def test_missing_direct_binary_is_derived_from_the_verified_archive(self):
        files = self.package()
        for path in files[2:]:
            path.unlink()
        actual = release.verify_package(self.directory, TAG, REVISION)
        self.assertEqual(actual, files)
        self.assertEqual(files[2].read_bytes(), b"qualified executable")
        self.assertEqual(files[2].stat().st_mode & 0o777, 0o755)
        self.assertEqual(files[3].read_text(),
                         f"{release.digest(files[2])}  {files[2].name}\n")
        self.assertEqual(release.verify_package(self.directory, TAG, REVISION), files)
        self.assertFalse(list(self.directory.glob(".lume-binary-*")))

    def test_invalid_archive_never_creates_a_direct_binary(self):
        files = self.package(binary_sha256="0" * 64)
        for path in files[2:]:
            path.unlink()
        with self.assertRaisesRegex(ValueError, "manifest"):
            release.verify_package(self.directory, TAG, REVISION)
        self.assertFalse(any(path.exists() for path in files[2:]))
        self.assertFalse(list(self.directory.glob(".lume-binary-*")))

    def test_existing_direct_assets_are_verified_and_never_overwritten(self):
        files = self.package()
        files[2].write_bytes(b"keep this existing file")
        with self.assertRaisesRegex(ValueError, "direct binary"):
            release.verify_package(self.directory, TAG, REVISION)
        self.assertEqual(files[2].read_bytes(), b"keep this existing file")
        files = self.package()
        files[3].write_text("keep this checksum\n")
        with self.assertRaisesRegex(ValueError, "checksum"):
            release.verify_package(self.directory, TAG, REVISION)
        self.assertEqual(files[3].read_text(), "keep this checksum\n")

    def test_direct_asset_symlinks_are_rejected_without_following_them(self):
        files = self.package()
        outside = self.directory / "private-file"
        outside.write_bytes(b"preserve private bytes")
        for path in files[2:]:
            path.unlink()
            path.symlink_to(outside)
            with self.assertRaisesRegex(ValueError, "regular file"):
                release.verify_package(self.directory, TAG, REVISION)
            path.unlink()
        self.assertEqual(outside.read_bytes(), b"preserve private bytes")

    def test_failed_materialization_cleans_temporary_files(self):
        files = self.package()
        for path in files[2:]:
            path.unlink()
        with patch.object(release.shutil, "copyfileobj", side_effect=OSError("disk full")):
            with self.assertRaisesRegex(OSError, "disk full"):
                release.verify_package(self.directory, TAG, REVISION)
        self.assertFalse(any(path.exists() for path in files[2:]))
        self.assertFalse(list(self.directory.glob(".lume-binary-*")))

    def test_qualified_direct_binary_can_be_added_to_an_existing_release(self):
        files = self.package()
        states = [self.assets(files[:2], draft=False), self.assets(files, draft=False)]
        with (patch.object(release, "release_metadata", side_effect=states),
              patch.object(release, "github") as github):
            github.return_value.stdout = REVISION
            release.publish(REPOSITORY, TAG, REVISION, files)
        calls = [call.args for call in github.call_args_list]
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[1][:3], ("release", "upload", TAG))
        self.assertEqual(calls[1][3:5], tuple(map(str, files[2:])))
        self.assertNotIn("--clobber", calls[1])

    def test_missing_or_corrupt_appended_assets_do_not_report_success(self):
        files = self.package()
        incomplete = self.assets(files[:3], draft=False)
        with (patch.object(release, "release_metadata",
                           side_effect=[self.assets(files[:2], draft=False), incomplete]),
              patch.object(release, "github") as github):
            github.return_value.stdout = REVISION
            with self.assertRaises(ValueError):
                release.publish(REPOSITORY, TAG, REVISION, files)
        changed = self.assets(files[:2], draft=False)
        changed["assets"][0]["digest"] = "sha256:" + "0" * 64
        with (patch.object(release, "release_metadata", return_value=changed),
              patch.object(release, "github") as github):
            github.return_value.stdout = REVISION
            with self.assertRaises(ValueError):
                release.publish(REPOSITORY, TAG, REVISION, files)
            github.assert_called_once()



if __name__ == "__main__":
    unittest.main()
