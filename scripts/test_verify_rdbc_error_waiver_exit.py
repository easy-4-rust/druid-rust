#!/usr/bin/env python3
"""Adversarial tests for the frozen error-disposition migration gate."""

from __future__ import annotations

import unittest
from pathlib import Path
from unittest.mock import patch

from verify_rdbc_error_waiver_exit import (
    DispositionRow,
    gate_baseline_disposed,
    repository_root,
)


def frozen_row(path: str) -> DispositionRow:
    return DispositionRow(Path(path), "RDBC_STANDARD", 1, "0" * 64, "USE_RDBC_ERROR")


class ErrorWaiverMigrationTest(unittest.TestCase):
    def test_closed_authority_allows_deleted_standard_source(self) -> None:
        result = gate_baseline_disposed(
            repository_root(),
            [frozen_row("crates/druid-rdbc/src/spi/physical_connection.rs")],
        )
        self.assertTrue(result.passed, result.detail)

    def test_pending_authority_still_rejects_deleted_standard_source(self) -> None:
        result = gate_baseline_disposed(
            repository_root(),
            [frozen_row("crates/druid-rdbc/src/resource/rdbc_array_access.rs")],
        )
        self.assertFalse(result.passed)
        self.assertIn("rdbc_array_access.rs", result.detail)

    def test_unmapped_deleted_adapter_remains_rejected(self) -> None:
        result = gate_baseline_disposed(
            repository_root(),
            [frozen_row("crates/druid-wrapper/src/duckdb/duckdb_prepared_statement.rs")],
        )
        self.assertFalse(result.passed)
        self.assertIn("duckdb_prepared_statement.rs", result.detail)

    def test_mixed_closed_and_pending_authority_stays_rejected(self) -> None:
        result = gate_baseline_disposed(
            repository_root(),
            [frozen_row("crates/druid-rdbc/src/spi/rdbc_result_set.rs")],
        )
        self.assertFalse(result.passed)
        self.assertIn("rdbc_result_set.rs", result.detail)

    def test_unverified_authority_cannot_release_deleted_source(self) -> None:
        with patch(
            "verify_rdbc_error_waiver_exit.verify_task6_migration_authority",
            side_effect=RuntimeError("invalid handoff"),
        ):
            result = gate_baseline_disposed(
                repository_root(),
                [frozen_row("crates/druid-rdbc/src/spi/physical_connection.rs")],
            )
        self.assertFalse(result.passed)
        self.assertIn("invalid handoff", result.detail)


if __name__ == "__main__":
    unittest.main()
