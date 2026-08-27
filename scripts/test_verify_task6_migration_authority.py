#!/usr/bin/env python3
"""Adversarial self-tests for the Task 6 migration authority verifier."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from verify_task6_migration_authority import (
    STANDARDIZATION_ONLY_TASK7,
    TASK7_EARLY_COMPONENTS,
    TASK7_PENDING_AUTHORITY_PATHS,
    TASK7_PENDING_COMPONENTS,
    VerificationError,
    module_declares_exact_target,
    verify_authority_schema,
    verify_standardization,
    verify_task7_scope_boundary,
)


class Task6MigrationAuthorityTest(unittest.TestCase):
    def test_bounded_task7_rows_cannot_be_falsely_closed(self) -> None:
        rows = []
        pending = sorted(TASK7_PENDING_AUTHORITY_PATHS)
        for index in range(79):
            old_path = pending[index] if index < len(pending) else f"rust::Legacy{index}"
            rows.append(
                {
                    "old_path": old_path,
                    "disposition": "DELETED",
                    "target_path": "-",
                    "target_task": "P04_TASK_7_SEMANTIC_HARDENING"
                    if index < len(pending)
                    else "P04_TASK_6_REMOVE_PHYSICAL",
                    "state": "TASK7_PENDING"
                    if index < len(pending)
                    else "TASK6_CLOSED",
                    "source_file": "old.rs",
                    "normalized_signature": f"struct Legacy{index}",
                    "method_handoff": "NOT_APPLICABLE",
                    "test_handoff": "task6-deleted-test-migration.tsv#exact-denominator=1283",
                    "evidence": "bounded scope",
                }
            )
        rows[0]["state"] = "TASK6_CLOSED"
        with self.assertRaisesRegex(VerificationError, "not pending"):
            verify_authority_schema(tuple(rows))

    def test_task7_scope_requires_exact_early_and_pending_denominators(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plan = root / "docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md"
            plan.parent.mkdir(parents=True)
            for _, source_path in TASK7_EARLY_COMPONENTS.values():
                source = root / source_path
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_text("// live\n", encoding="utf-8")
            early_rows = "\n".join(
                f"| `{component}` | `COMPLETED_EARLY_TASK6` | `{identity}` | `{source}` |"
                for component, (identity, source) in TASK7_EARLY_COMPONENTS.items()
            )
            pending_rows = "\n".join(
                f"| `{component}` | `TASK7_PENDING` | retained | retained |"
                for component in TASK7_PENDING_COMPONENTS
            )
            plan.write_text(
                "### Task 7 bounded pull-forward denominator\n"
                + early_rows
                + "\n"
                + pending_rows
                + "\nTask 7 remaining denominator: **7**\n"
                + "This is a sequencing change; it does not reduce Task 7 requirements.\n"
                + "### Task 8:\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(VerificationError, "denominator is not exact"):
                verify_task7_scope_boundary(root)

    def test_exact_module_target_does_not_accept_same_leaf_elsewhere(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/druid/src"
            (source / "core").mkdir(parents=True)
            (source / "lib.rs").write_text("pub mod core;\n", encoding="utf-8")
            (source / "core/mod.rs").write_text("mod actual;\n", encoding="utf-8")
            (source / "core/actual.rs").write_text(
                "pub(crate) struct Target;\n", encoding="utf-8"
            )
            self.assertFalse(
                module_declares_exact_target(root, "druid::core::expected::Target")
            )
            self.assertTrue(
                module_declares_exact_target(root, "druid::core::actual::Target")
            )

    def test_standardization_must_match_exact_target_not_only_old_path(self) -> None:
        authority = []
        physical = []
        for index in range(67):
            old_path = f"druid_rdbc::spi::Legacy{index}"
            target_path = f"druid_rdbc::connection::Target{index}"
            authority.append(
                {
                    "old_path": old_path,
                    "disposition": "MERGED_STANDARD",
                    "target_path": target_path,
                    "target_task": "P04_TASK_3_STANDARD_TRAITS",
                    "state": "TASK6_CLOSED",
                    "source_file": "old.rs",
                    "normalized_signature": f"struct Legacy{index}",
                }
            )
            physical.append(
                {
                    "old_path": old_path,
                    "classification": "MERGE_STANDARD",
                    "target_path": target_path,
                    "compatibility": "TASK6_CLOSED",
                    "source_file": "old.rs",
                    "normalized_signature": f"struct Legacy{index}",
                    "target_task": "P04_TASK_3_STANDARD_TRAITS",
                }
            )
        physical[0]["target_path"] = "druid_rdbc::wrong::Target0"
        for old_path in STANDARDIZATION_ONLY_TASK7:
            physical.append(
                {
                    "old_path": old_path,
                    "classification": "MERGE_STANDARD",
                    "target_path": old_path,
                    "compatibility": "TASK7_PENDING",
                    "source_file": "resource.rs",
                    "normalized_signature": "type Resource",
                    "target_task": "P04_TASK_7_SEMANTIC_HARDENING",
                }
            )
        with self.assertRaisesRegex(VerificationError, "disagrees"):
            verify_standardization(tuple(authority), tuple(physical))

    def test_pending_row_cannot_claim_task6_or_another_owner(self) -> None:
        rows = []
        for index in range(79):
            rows.append(
                {
                    "old_path": f"rust::Legacy{index}",
                    "disposition": "DELETED",
                    "target_path": "-",
                    "target_task": "P04_TASK_6_REMOVE_PHYSICAL",
                    "state": "TASK6_CLOSED",
                    "source_file": "old.rs",
                    "normalized_signature": f"struct Legacy{index}",
                    "method_handoff": "NOT_APPLICABLE",
                    "test_handoff": "task6-deleted-test-migration.tsv#exact-denominator=1283",
                    "evidence": "removed",
                }
            )
        rows[0] = {
            "old_path": "druid_rdbc::spi::RdbcResultSet",
            "disposition": "CONCRETE_MOVED",
            "target_path": "druid_rdbc::extensions::BufferedRowSet",
            "target_task": "P04_TASK_6_REMOVE_PHYSICAL",
            "state": "TASK7_PENDING",
            "source_file": "old.rs",
            "normalized_signature": "struct RdbcResultSet",
            "method_handoff": "NOT_APPLICABLE",
            "test_handoff": "task6-deleted-test-migration.tsv#exact-denominator=1283",
            "evidence": "future target",
        }
        with self.assertRaisesRegex(VerificationError, "another owner"):
            verify_authority_schema(tuple(rows))


if __name__ == "__main__":
    unittest.main()
