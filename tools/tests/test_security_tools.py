"""License evidence, visual preflight and scoped advisory failures."""

import datetime
import unittest
from unittest.mock import patch

from tools import advisories, dependency_inventory, visual_dependencies


class AdvisoryTests(unittest.TestCase):
    def test_new_vulnerabilities_and_warnings_always_fail(self):
        for report in [
            {"vulnerabilities": {"list": [{"advisory": {"id": "NEW"}}]}},
            {"vulnerabilities": {"list": []}, "warnings": {"notice": [
                {"advisory": {"id": "NEW"}, "package": {
                    "name": "dependency", "version": "1.0", "checksum": "changed"
                }}
            ]}},
        ]:
            with self.assertRaises(ValueError):
                advisories.qualify(report, [], datetime.date(2026, 10, 21))

    def test_removed_reviews_cannot_accumulate_silent_exceptions(self):
        report = {"vulnerabilities": {"list": []}, "warnings": {}}
        with self.assertRaisesRegex(ValueError, "obsolete"):
            advisories.qualify(report, [{"id": "OBSOLETE"}], datetime.date(2026, 10, 21))

    def test_missing_pillow_reports_the_exact_installation_command(self):
        with patch("importlib.import_module", side_effect=ImportError):
            with self.assertRaisesRegex(RuntimeError, "requirements-visual.txt"):
                visual_dependencies.pillow()

    def test_inventory_accounts_for_every_locked_package_and_notice_hash(self):
        report = dependency_inventory.verify()
        self.assertGreater(len(report["packages"]), 300)
        self.assertIn("sources/selectors-0.38.0.crate", report["files"])


if __name__ == "__main__":
    unittest.main()
