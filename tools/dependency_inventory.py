"""Verify locked license evidence; cargo-about owns SPDX interpretation."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "assets/licenses"
ABOUT_VERSION = "0.9.2"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def locked_packages():
    return {
        (package["name"], package["version"]): package
        for package in tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    }


def verify():
    report = json.loads((DESTINATION / "inventory.json").read_text())
    if report["lock_sha256"] != digest(ROOT / "Cargo.lock"):
        raise ValueError("License inventory is stale; run make licenses")
    if report["manifest_sha256"] != digest(ROOT / "Cargo.toml"):
        raise ValueError("Dependency features or manifest changed; run make licenses")
    if report["policy_sha256"] != digest(ROOT / "about.toml"):
        raise ValueError("License policy changed; run make licenses")
    covered = {(item["name"], item["version"]) for item in report["packages"]}
    inactive = {(item["name"], item["version"]) for item in report["inactive"]}
    if covered & inactive or covered | inactive != set(locked_packages()):
        raise ValueError("License inventory does not account for every locked package")
    for name, expected in report["files"].items():
        if digest(DESTINATION / name) != expected:
            raise ValueError(f"License evidence changed: {name}")
    return report


def generate(cargo_about):
    version = subprocess.check_output([cargo_about, "--version"], text=True)
    if version.strip() != f"cargo-about {ABOUT_VERSION}":
        raise ValueError(f"Use cargo-about {ABOUT_VERSION}; found {version.strip()}")
    command = [cargo_about, "generate", "--locked"]
    result = subprocess.check_output(command + ["--format", "json"], cwd=ROOT)
    gathered = json.loads(result)
    text = subprocess.check_output(command + ["tools/licenses.hbs"], cwd=ROOT)
    packages = sorted(
        ({"name": item["package"]["name"],
          "version": item["package"]["version"],
          "license": item["license"]} for item in gathered["crates"]),
        key=lambda item: (item["name"], item["version"]),
    )
    covered = {(item["name"], item["version"]) for item in packages}
    inactive = [{"name": name, "version": version}
                for name, version in sorted(set(locked_packages()) - covered)]
    # cargo-about resolves feature-active normal/build edges. The lock can also
    # retain optional, inactive packages; list those explicitly, never as licensed.
    notice = DESTINATION / "THIRD_PARTY_LICENSES.txt"
    notice.write_bytes(text)
    files = {notice.name: digest(notice)}
    files.update(copy_mpl_sources(packages))
    report = {
        "generator": f"cargo-about {ABOUT_VERSION}",
        "lock_sha256": digest(ROOT / "Cargo.lock"),
        "manifest_sha256": digest(ROOT / "Cargo.toml"),
        "policy_sha256": digest(ROOT / "about.toml"),
        "packages": packages,
        "inactive": inactive,
        "files": files,
    }
    (DESTINATION / "inventory.json").write_text(json.dumps(report, indent=2) + "\n")
    verify()


def copy_mpl_sources(packages):
    home = Path.home() / ".cargo/registry/cache"
    destination = DESTINATION / "sources"
    destination.mkdir(exist_ok=True)
    files = {}
    required = {"selectors", "cssparser", "cssparser-macros", "dtoa-short"}
    for package in packages:
        if package["name"] not in required:
            continue
        key = (package["name"], package["version"])
        filename = f"{key[0]}-{key[1]}.crate"
        candidates = list(home.glob(f"*/{filename}"))
        if len(candidates) != 1:
            raise ValueError(f"Expected exactly one cached source archive: {filename}")
        source = candidates[0]
        if digest(source) != locked_packages()[key]["checksum"]:
            raise ValueError(f"Source archive checksum mismatch: {filename}")
        target = destination / filename
        shutil.copyfile(source, target)
        files[f"sources/{filename}"] = digest(target)
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--cargo-about", default="cargo-about")
    arguments = parser.parse_args()
    if arguments.generate:
        generate(arguments.cargo_about)
    else:
        report = verify()
        print(f"Verified licenses: {len(report['packages'])} active packages, "
              f"{len(report['inactive'])} explicitly inactive locked packages")


if __name__ == "__main__":
    main()
