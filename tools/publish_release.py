"""Validate tagged packages and publish verified assets without replacing bytes.

Validation is the default. GitHub writes require --publish and run only after
the exact tag, archive bytes, build manifest and executable have been checked.
Validation also stages the direct binary and its checksum from the archive.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"
NUMBER = r"(?:0|[1-9][0-9]*)"
PRERELEASE = rf"(?:{NUMBER}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
TAG = rf"v{NUMBER}\.{NUMBER}\.{NUMBER}(?:-{PRERELEASE}(?:\.{PRERELEASE})*)?"


def validate_tag(tag, version):
    if not re.fullmatch(TAG, tag):
        raise ValueError("Release tags must be vMAJOR.MINOR.PATCH or a prerelease")
    if tag != f"v{version}":
        raise ValueError(f"Tag {tag} does not match Cargo.toml version {version}")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_package(directory, tag, revision):
    name = f"lume-{tag[1:]}-{TARGET}"
    package = directory / f"{name}.tar.gz"
    checksum = directory / f"{name}.tar.gz.sha256"
    if checksum.read_text() != f"{digest(package)}  {package.name}\n":
        raise ValueError("Release archive checksum does not match its bytes")
    with tarfile.open(package, "r:gz") as bundle:
        metadata = bundle.getmember(f"{name}/BUILD.json")
        if not metadata.isfile() or metadata.size > 64 * 1024:
            raise ValueError("Invalid release build manifest")
        with bundle.extractfile(metadata) as stream:
            build = json.load(stream)
        if (build["revision"] != revision or build["version"] != tag[1:]
                or build["target"] != TARGET or build["source_dirty"]):
            raise ValueError("Package does not match the clean tagged revision")
        binary = bundle.getmember(f"{name}/bin/lume")
        if not binary.isfile() or binary.size > 128 * 1024 * 1024:
            raise ValueError("Invalid release executable")
        with bundle.extractfile(binary) as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != build["binary_sha256"]:
                raise ValueError("Release executable does not match its manifest")
        exported = stage_binary(bundle, binary, directory / name, build["binary_sha256"])
    return [package, checksum, *exported]


def stage_binary(bundle, member, binary, expected):
    checksum = binary.with_name(binary.name + ".sha256")
    for path in (binary, checksum):
        if path.is_symlink() or (path.exists() and not path.is_file()):
            raise ValueError(f"Release asset must be a regular file: {path.name}")
    text = f"{expected}  {binary.name}\n"
    if checksum.exists() and checksum.read_text() != text:
        raise ValueError("Direct binary checksum does not match its bytes")
    if binary.exists():
        if binary.stat().st_size != member.size or digest(binary) != expected:
            raise ValueError("Existing direct binary differs from the qualified archive")
    else:
        materialize_binary(bundle, member, binary, expected)
    binary.chmod(0o755)
    if not checksum.exists():
        with checksum.open("x") as stream:
            stream.write(text)
    return [binary, checksum]


def materialize_binary(bundle, member, binary, expected):
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=binary.parent, prefix=".lume-binary-", delete=False) as target:
            temporary = Path(target.name)
            with bundle.extractfile(member) as source:
                shutil.copyfileobj(source, target, length=1024 * 1024)
        if digest(temporary) != expected:
            raise ValueError("Extracted binary differs from its build manifest")
        temporary.chmod(0o755)
        # Same-directory hard linking publishes atomically without replacing a
        # competing file. Only the validated executable member is extracted.
        os.link(temporary, binary)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def github(*arguments, check=True):
    return subprocess.run(["gh", *arguments], capture_output=True, text=True,
                          check=check, timeout=120)


def release_metadata(repository, tag):
    result = github("api", f"repos/{repository}/releases/tags/{tag}", check=False)
    if result.returncode == 0:
        return json.loads(result.stdout)
    if "HTTP 404" not in result.stderr:
        raise RuntimeError(f"Cannot inspect release: {result.stderr.strip()}")
    # The tag endpoint returns published releases. Authenticated listing also
    # exposes drafts, including the one just created by this publication job.
    page = 1
    while True:
        result = github("api", f"repos/{repository}/releases?per_page=100&page={page}", check=False)
        if result.returncode:
            raise RuntimeError(f"Cannot inspect release drafts: {result.stderr.strip()}")
        releases = json.loads(result.stdout)
        matching = [release for release in releases if release["tag_name"] == tag]
        if len(matching) > 1:
            raise ValueError("Multiple releases match the requested tag")
        if matching:
            return matching[0]
        if len(releases) < 100:
            return None
        page += 1


def missing_assets(release, files):
    if release is None:
        raise ValueError("Release draft disappeared before publication")
    actual = {item["name"]: item.get("digest") for item in release["assets"]}
    expected = {file.name: file for file in files}
    if len(actual) != len(release["assets"]) or not set(actual) <= set(expected):
        raise ValueError("Release asset names differ from the qualified package")
    for name, recorded in actual.items():
        if recorded != f"sha256:{digest(expected[name])}":
            raise ValueError(f"GitHub asset does not match local bytes: {name}")
    return [file for file in files if file.name not in actual]


def verify_assets(release, files):
    if missing_assets(release, files):
        raise ValueError("Release asset names differ from the qualified package")


def publish(repository, tag, revision, files):
    remote = github("api", f"repos/{repository}/commits/{tag}", "--jq", ".sha")
    if remote.stdout.strip() != revision:
        raise ValueError("Remote tag moved after qualification; refusing to publish")
    release = release_metadata(repository, tag)
    if release and not release["draft"]:
        missing = missing_assets(release, files)
        if missing:
            # Published bytes are immutable. Append only missing, independently
            # verified assets and never send --clobber for a public release.
            github("release", "upload", tag, *map(str, missing), "--repo", repository)
            verify_assets(release_metadata(repository, tag), files)
        print(f"Release {tag} has complete, verified published assets")
        return
    prerelease = "-" in tag
    if release is None:
        github("release", "create", tag, "--repo", repository, "--verify-tag",
               "--target", revision, "--draft", "--generate-notes",
               "--title", f"Lume {tag}", f"--prerelease={str(prerelease).lower()}")
    github("release", "upload", tag, *map(str, files),
           "--repo", repository, "--clobber")
    verify_assets(release_metadata(repository, tag), files)
    github("release", "edit", tag, "--repo", repository, "--draft=false",
           f"--prerelease={str(prerelease).lower()}",
           f"--latest={str(not prerelease).lower()}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--check-tag-only", action="store_true")
    parser.add_argument("--revision", default=os.environ.get("GITHUB_SHA"))
    parser.add_argument("--artifacts", type=Path, default=ROOT / "dist")
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY"))
    parser.add_argument("--publish", action="store_true")
    arguments = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    packages = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    if [package["version"] for package in packages
            if package["name"] == "lume"] != [version]:
        parser.error("Cargo.lock and Cargo.toml versions disagree")
    validate_tag(arguments.tag, version)
    if arguments.check_tag_only:
        return
    if not arguments.revision or not re.fullmatch(r"[0-9a-f]{40}", arguments.revision):
        parser.error("Provide the complete qualified commit SHA")
    files = verify_package(arguments.artifacts, arguments.tag, arguments.revision)
    if arguments.publish:
        if not arguments.repository or not re.fullmatch(r"[\w.-]+/[\w.-]+", arguments.repository):
            parser.error("Provide the GitHub repository as owner/name")
        publish(arguments.repository, arguments.tag, arguments.revision, files)
    print(f"Verified release artifacts for {arguments.tag}: {arguments.revision}")


if __name__ == "__main__":
    main()
