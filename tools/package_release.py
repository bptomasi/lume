"""Package the Linux executable, complete notices, source obligations and hashes.

Packaging has deterministic ordering and timestamps. It does not claim bitwise
reproducible compiler output across machines. No GitHub release is published.
"""

import argparse
import datetime
import gzip
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
import sys
import tempfile
import tomllib

from dependency_inventory import verify, digest
from advisories import qualify
from publish_release import verify_package

ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT, text=True).strip()


def standalone(binary, directory):
    environment = os.environ.copy()
    environment.update(HOME=str(directory / "home"),
                       LUME_DATA_DIR=str(directory / "data"),
                       LUME_CONFIG=str(directory / "config.toml"))
    for key in ("XDG_DATA_HOME", "XDG_CONFIG_HOME", "LD_PRELOAD", "LD_LIBRARY_PATH"):
        environment.pop(key, None)
    commands = [
        ["--help"], ["--version"], ["--offline", "feeds"],
        ["backup", str(directory / "snapshot.sqlite3")],
        ["restore", str(directory / "snapshot.sqlite3"), str(directory / "restored")],
    ]
    for arguments in commands:
        subprocess.run([str(binary), *arguments], cwd=directory,
                       env=environment, check=True, capture_output=True, timeout=30)
    if not (directory / "restored/config.toml").is_file():
        raise ValueError("Standalone recovery did not publish a configuration")

    installer = binary.parents[1] / "tools/install.py"
    prefix = directory / "installed"
    arguments = [sys.executable, str(installer), "--binary", str(binary),
                 "--prefix", str(prefix), "--data-dir", str(directory / "data")]
    subprocess.run(arguments, env=environment, cwd=directory, check=True,
                   capture_output=True, timeout=30)
    before = [(directory / name).read_bytes()
              for name in ("data/library.sqlite3", "config.toml")]
    subprocess.run(arguments + ["--upgrade"], env=environment, cwd=directory,
                   check=True, capture_output=True, timeout=30)
    after = [(directory / name).read_bytes()
             for name in ("data/library.sqlite3", "config.toml")]
    if before != after:
        raise ValueError("Standalone upgrade changed reading data or config")
    if not (prefix / "share/lume/licenses/THIRD_PARTY_LICENSES.txt").is_file():
        raise ValueError("Installation omitted complete license notices")


def required_glibc(binary):
    versions = subprocess.check_output(["readelf", "--version-info", str(binary)], text=True)
    values = [tuple(map(int, version.split(".")))
              for version in re.findall(r"GLIBC_([0-9.]+)", versions)]
    if not values or max(values) > (2, 39) or "GLIBC_PRIVATE" in versions:
        raise ValueError("Binary exceeds the qualified public glibc 2.39 ABI")
    return ".".join(map(str, max(values)))


def assemble(binary, staging, revision, inventory, audit):
    (staging / "bin").mkdir()
    shutil.copyfile(binary, staging / "bin/lume")
    (staging / "bin/lume").chmod(0o755)
    for name in ("LICENSE", "README.md"):
        shutil.copyfile(ROOT / name, staging / name)
    shutil.copytree(ROOT / "assets/licenses", staging / "assets/licenses")
    syntax = staging / "assets/syntax"
    syntax.mkdir(parents=True)
    for source in (ROOT / "assets/syntax").iterdir():
        if "license" in source.name.lower() or source.name == "PROVENANCE.md":
            shutil.copyfile(source, syntax / source.name)
    (staging / "tools").mkdir()
    shutil.copyfile(ROOT / "tools/install.py", staging / "tools/install.py")
    (staging / "INSTALL.txt").write_text(
        'Install: python3 tools/install.py --binary bin/lume --prefix "$HOME/.local" '
        '--data-dir "$HOME/.local/lume"\n'
        'Upgrade: add --upgrade to that command. Back up your library first.\n'
        'Runtime needs no Python or checkout. Linux x86_64, glibc >= 2.39.\n'
    )
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    manifest = {
        "version": version, "revision": revision, "target": TARGET,
        "source_dirty": bool(git("status", "--porcelain", "--untracked-files=normal")),
        "lock_sha256": inventory["lock_sha256"], "binary_sha256": digest(binary),
        "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "required_glibc": required_glibc(binary),
    }
    (staging / "DEPENDENCY_AUDIT.json").write_text(json.dumps(audit, indent=2) + "\n")
    (staging / "BUILD.json").write_text(json.dumps(manifest, indent=2) + "\n")
    paths = sorted(path for path in staging.rglob("*") if path.is_file())
    checksums = "".join(f"{digest(path)}  {path.relative_to(staging)}\n" for path in paths)
    (staging / "SHA256SUMS").write_text(checksums)
    return version


def archive(staging, output, name, epoch):
    destination = output / f"{name}.tar.gz"
    if destination.exists():
        raise ValueError(f"Package already exists: {destination}")
    with destination.open("xb") as raw:
        with gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as bundle:
                for path in [staging, *sorted(staging.rglob("*"))]:
                    relative = path.relative_to(staging)
                    info = bundle.gettarinfo(str(path), str(Path(name) / relative))
                    info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    info.mtime = epoch
                    if path.is_file():
                        with path.open("rb") as content:
                            bundle.addfile(info, content)
                    else:
                        bundle.addfile(info)
    destination.with_suffix(".gz.sha256").write_text(f"{digest(destination)}  {destination.name}\n")
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--require-clean", action="store_true")
    parser.add_argument("--security-report", type=Path, default=ROOT / "reports/advisories.json")
    arguments = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() not in ("x86_64", "AMD64"):
        raise ValueError(f"Qualified package target is {TARGET}")
    if arguments.require_clean and git("status", "--porcelain"):
        raise ValueError("Release packaging requires a clean source tree")
    inventory = verify()
    audit = json.loads(arguments.security_report.read_text())
    qualification = audit["qualification"]
    today = datetime.datetime.now(datetime.timezone.utc).date()
    if qualification["lock_sha256"] != inventory["lock_sha256"] or qualification["date"] != today.isoformat():
        raise ValueError("Run make check-security on the current lockfile before packaging")
    reviews = tomllib.loads((ROOT / "tools/advisory-reviews.toml").read_text())["review"]
    qualify(audit, reviews, today)
    revision = git("rev-parse", "HEAD")
    binary = arguments.binary.resolve(strict=True)
    version_text = subprocess.check_output([str(binary), "--version"], text=True)
    if revision not in version_text:
        raise ValueError("Rebuild the executable from the current revision")
    arguments.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="lume-package-") as temporary:
        root = Path(temporary)
        staging = root / "package"
        staging.mkdir()
        version = assemble(binary, staging, revision, inventory, audit)
        verification = root / "verification"
        verification.mkdir()
        standalone(staging / "bin/lume", verification)
        epoch = int(git("show", "-s", "--format=%ct", "HEAD"))
        destination = archive(staging, arguments.output, f"lume-{version}-{TARGET}", epoch)
    files = verify_package(arguments.output, f"v{version}", revision)
    print(f"Verified standalone package: {destination}")
    print(f"Verified direct executable: {files[2]}")


if __name__ == "__main__":
    main()
