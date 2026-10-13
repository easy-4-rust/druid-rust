#!/usr/bin/env python3
"""Fail-closed checks for the compact SemVer breaking allowlist."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("verify_rdbc_single_standard_breaking_api.py")
sys.path.insert(0, str(SCRIPT.parent))

from verify_rdbc_single_standard_breaking_api import findings_by_lint  # noqa: E402
import verify_rdbc_single_standard_breaking_api as verifier  # noqa: E402


def report(item: str, line: int = 4) -> str:
    return (
        "Checked 1 checks: 0 pass, 1 fail\n"
        "--- failure enum_missing: pub enum removed or renamed ---\n"
        "Failed in:\n"
        f"  enum druid::{item}, previously in file "
        f"/home/runner/work/druid-rust/druid-rust/crates/druid/src/item.rs:{line}\n"
        "Summary semver requires new major version: 1 major and 0 minor checks failed\n"
    )


class BreakingApiAllowlistTest(unittest.TestCase):
    def test_module_migration_map_requires_every_reported_path_and_real_evidence(self) -> None:
        module_report = (
            "--- failure module_missing: pub module removed or renamed ---\n"
            "Failed in:\n"
            "  mod druid::core::physical_connection, previously in file old.rs:1\n"
            "  mod druid::spi, previously in file old_spi.rs:1\n"
            "Summary semver requires new major version: 1 major and 0 minor checks failed\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target.rs"
            test = root / "contract_test.rs"
            target.write_text("// standard target\n", encoding="utf-8")
            test.write_text("// regression contract\n", encoding="utf-8")
            mapping = root / "migration.tsv"
            header = "old_module\ttarget_file\ttransition\ttest_file\n"
            rows = (
                "druid::core::physical_connection\ttarget.rs\tSTANDARD_PORT\tcontract_test.rs\n"
                "druid::spi\ttarget.rs\tSPLIT_STANDARD\tcontract_test.rs\n"
            )
            mapping.write_text(header + rows, encoding="utf-8")
            verifier.verify_module_migration_map(module_report, mapping, root)
            mapping.write_text(header + rows.splitlines(keepends=True)[0], encoding="utf-8")
            with self.assertRaises(ValueError):
                verifier.verify_module_migration_map(module_report, mapping, root)
            mapping.write_text(header + rows.replace("contract_test.rs", "missing.rs"), encoding="utf-8")
            with self.assertRaises(ValueError):
                verifier.verify_module_migration_map(module_report, mapping, root)

    def test_fingerprint_keeps_item_but_ignores_source_line(self) -> None:
        self.assertEqual(
            findings_by_lint(report("Removed"), 100),
            findings_by_lint(report("Removed", 99), 100),
        )
        self.assertNotEqual(
            findings_by_lint(report("Removed"), 100),
            findings_by_lint(report("Different"), 100),
        )

    def test_incomplete_report_and_non_semver_exit_fail_closed(self) -> None:
        with self.assertRaises(ValueError):
            findings_by_lint(report("Removed"), 101)
        with self.assertRaises(ValueError):
            findings_by_lint("Summary no semver update required", 100)
        self.assertEqual(findings_by_lint("Summary no semver update required", 0), {})

    def test_pending_changed_and_exactly_approved_rows(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = [root / f"{name}.txt" for name in ("rdbc", "druid", "wrapper")]
            for path in paths:
                path.write_text(report("Removed"), encoding="utf-8")
            allowlist = root / "allowlist.tsv"
            arguments = [
                sys.executable,
                str(SCRIPT),
                "--allowlist",
                str(allowlist),
            ]
            for short, path in zip(("rdbc", "druid", "wrapper"), paths, strict=True):
                arguments.extend((f"--{short}-report", str(path), f"--{short}-exit-code", "100"))

            candidate = subprocess.run([*arguments, "--write-candidate"], capture_output=True)
            self.assertEqual(candidate.returncode, 0, candidate.stderr)
            self.assertEqual(subprocess.run(arguments, capture_output=True).returncode, 1)

            rows = allowlist.read_text(encoding="utf-8").replace(
                "pending\treview migration and consumers", "approved\tpre-release migration reviewed"
            )
            allowlist.write_text(rows, encoding="utf-8")
            self.assertEqual(subprocess.run(arguments, capture_output=True).returncode, 0)

            paths[1].write_text(report("Unexpected"), encoding="utf-8")
            self.assertEqual(subprocess.run(arguments, capture_output=True).returncode, 1)


if __name__ == "__main__":
    unittest.main()
