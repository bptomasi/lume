"""Gate current RustSec results against narrowly reviewed informational findings."""

import argparse
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.22.2"


def qualify(report, reviews, today):
    failures = []
    for finding in report["vulnerabilities"]["list"]:
        failures.append(finding["advisory"]["id"])
    actual = {}
    for category, findings in report.get("warnings", {}).items():
        for finding in findings:
            advisory = finding["advisory"]["id"]
            package = finding["package"]
            actual[advisory] = (category, package["name"], package["version"],
                                package["checksum"])
    expected = {review["id"]: review for review in reviews}
    for advisory, identity in actual.items():
        review = expected.get(advisory)
        if review is None:
            failures.append(f"Unreviewed warning: {advisory}")
            continue
        approved = tuple(review[key] for key in ("category", "package", "version", "checksum"))
        if identity != approved or today > review["expires"]:
            failures.append(f"Changed or expired review: {advisory}")
    for advisory in expected.keys() - actual.keys():
        failures.append(f"Remove obsolete review: {advisory}")
    if failures:
        raise ValueError("; ".join(failures))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-audit", default="cargo-audit")
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    version = subprocess.check_output([arguments.cargo_audit, "--version"], text=True)
    if version.strip() != f"cargo-audit {VERSION}":
        raise ValueError(f"Use cargo-audit {VERSION}")
    result = subprocess.run([arguments.cargo_audit, "audit", "--json"],
                            cwd=ROOT, capture_output=True, text=True, check=False)
    if result.returncode not in (0, 1):
        raise RuntimeError(result.stderr or result.stdout)
    report = json.loads(result.stdout)
    today = datetime.datetime.now(datetime.timezone.utc).date()
    report["qualification"] = {
        "date": today.isoformat(),
        "lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        "tool": f"cargo-audit {VERSION}",
    }
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    reviews = tomllib.loads((ROOT / "tools/advisory-reviews.toml").read_text())["review"]
    qualify(report, reviews, today)
    print(f"No reported vulnerabilities; {len(reviews)} scoped informational reviews. "
          f"Evidence: {arguments.output}")


if __name__ == "__main__":
    main()
