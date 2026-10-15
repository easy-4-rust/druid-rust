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
    def test_retired_method_ledger_preserves_exact_identities_and_duplicate_counts(self) -> None:
        method_report = (
            "--- failure inherent_method_missing: pub method removed or renamed ---\n"
            "\x1b[1mFailed in:\x1b[0m\n"
            "  DruidPooledConnection::close, previously in file "
            "/home/runner/work/druid-rust/druid-rust/target/semver-checks/git-base/rev/"
            "crates/druid/src/core/pooled_connection.rs:40\n"
            "  DruidPooledConnection::close, previously in file "
            "/home/runner/work/druid-rust/druid-rust/target/semver-checks/git-base/rev/"
            "crates/druid/src/core/pooled_connection.rs:40\n"
            "--- failure trait_method_missing: pub trait method removed ---\n"
            "Failed in:\n"
            "  method result_set_get_string of trait ResultSetFilter, previously in file "
            "/home/runner/work/druid-rust/druid-rust/crates/druid/src/core/result_filter.rs:80\n"
            "Summary semver requires new major version: 2 major and 0 minor checks failed\n"
        )
        rows = verifier.method_migration_rows(method_report)
        self.assertEqual(len(rows), 2)
        self.assertEqual(sum(int(row[5]) for row in rows), 3)
        self.assertEqual(rows[0][3], "crates/druid/src/core/pooled_connection.rs")
        self.assertEqual(rows[0][5], "2")
        self.assertEqual(rows[0][6], "APPROVED_PRE_RELEASE_STANDARD_MIGRATION")
        self.assertIn("druid_rdbc::connection::Connection", rows[0][7])
        self.assertEqual(rows[1][6], "APPROVED_PRE_RELEASE_INTERNAL_RETIREMENT")
        self.assertEqual(
            verifier.render_method_migration_ledger(method_report),
            verifier.render_method_migration_ledger(method_report),
        )

    def test_retired_method_ledger_rejects_unknown_finding_and_drift(self) -> None:
        malformed = (
            "--- failure inherent_method_missing: removed ---\n"
            "Failed in:\n  unexpected report format\n"
        )
        with self.assertRaisesRegex(ValueError, "unrecognized"):
            verifier.method_migration_rows(malformed)

        report_text = (
            "--- failure inherent_method_missing: removed ---\n"
            "Failed in:\n"
            "  DruidPooledConnection::close, previously in file old.rs:40\n"
            "Summary semver requires new major version: 1 major and 0 minor checks failed\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            ledger = Path(directory) / "migration.tsv"
            ledger.write_bytes(verifier.render_method_migration_ledger(report_text))
            verifier.verify_method_migration_ledger(report_text, ledger)
            ledger.write_text(ledger.read_text(encoding="utf-8").replace("close", "open"), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "differs from SemVer"):
                verifier.verify_method_migration_ledger(report_text, ledger)

    def test_method_arity_map_requires_exact_owner_and_shift_counts(self) -> None:
        arity_report = (
            "--- failure method_parameter_count_changed: pub method parameter count changed ---\n"
            "Failed in:\n"
            "  druid::core::Owner::read takes 1 parameters in old.rs:1, "
            "but now takes 0 parameters in new.rs:2\n"
            "  druid::core::alias::Owner::write takes 2 parameters in old.rs:3, "
            "but now takes 1 parameters in new.rs:4\n"
            "Summary semver requires new major version: 1 major and 0 minor checks failed\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "contract_test.rs").write_text("// contract\n", encoding="utf-8")
            mapping = root / "arity.tsv"
            header = "owner\tfindings\tshifts\ttest_files\n"
            row = "Owner\t2\t1>0:1;2>1:1\tcontract_test.rs\n"
            mapping.write_text(header + row, encoding="utf-8")
            verifier.verify_method_arity_migration_map(arity_report, mapping, root)
            for invalid in (
                header,
                header + row.replace("Owner", "Other"),
                header + row.replace("2>1:1", "2>1:2"),
                header + row.replace("contract_test.rs", "missing.rs"),
            ):
                mapping.write_text(invalid, encoding="utf-8")
                with self.assertRaises(ValueError):
                    verifier.verify_method_arity_migration_map(arity_report, mapping, root)

    def test_struct_migration_map_binds_each_removed_type_to_a_real_target(self) -> None:
        struct_report = (
            "--- failure struct_missing: pub struct removed or renamed ---\n"
            "Failed in:\n"
            "  struct druid::core::OldHandle, previously in file old.rs:1\n"
            "  struct druid::core::nested::OldHandle, previously in file old.rs:1\n"
            "Summary semver requires new major version: 1 major and 0 minor checks failed\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target.rs"
            test = root / "contract_test.rs"
            target.write_text("pub struct NewHandle;\n", encoding="utf-8")
            test.write_text("// regression contract\n", encoding="utf-8")
            mapping = root / "migration.tsv"
            header = "old_type\ttarget_file\ttarget_symbol\ttransition\ttest_file\n"
            row = "OldHandle\ttarget.rs\tNewHandle\tSTANDARD_PORT\tcontract_test.rs\n"
            mapping.write_text(header + row, encoding="utf-8")
            verifier.verify_struct_migration_map(struct_report, mapping, root)
            mapping.write_text(header, encoding="utf-8")
            with self.assertRaises(ValueError):
                verifier.verify_struct_migration_map(struct_report, mapping, root)
            mapping.write_text(header + row.replace("NewHandle", "MissingHandle"), encoding="utf-8")
            with self.assertRaises(ValueError):
                verifier.verify_struct_migration_map(struct_report, mapping, root)
            target.write_text("// pub struct NewHandle;\n", encoding="utf-8")
            mapping.write_text(header + row, encoding="utf-8")
            with self.assertRaises(ValueError):
                verifier.verify_struct_migration_map(struct_report, mapping, root)
            target.write_text("pub struct NewHandle;\n", encoding="utf-8")
            conflicting_identity = struct_report.replace(
                "nested::OldHandle, previously in file old.rs",
                "nested::OldHandle, previously in file another.rs",
            )
            with self.assertRaises(ValueError):
                verifier.verify_struct_migration_map(conflicting_identity, mapping, root)

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
