#!/usr/bin/env python3
"""Adversarial tests for the exact Task 6 public-contract allowlist."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from render_task6_public_contract_allowlist import (
    VerificationError,
    affected_items,
    candidate_target,
    known_internal_or_deleted,
    render_rows,
)


class PublicContractAllowlistTest(unittest.TestCase):
    def test_selects_identifier_and_filter_signature_breaks_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            snapshot = Path(temporary) / "api.txt"
            snapshot.write_text(
                "pub fn druid::Pool::physical_connection_timeout(self) -> Self\n"
                "pub fn druid::Filter::connection_connect(&self, "
                "druid::PhysicalConnectionConnectFilterChain)\n"
                "pub fn druid::Pool::unrelated(&self)\n",
                encoding="utf-8",
            )
            self.assertEqual(
                set(affected_items(snapshot)),
                {
                    "druid::Filter::connection_connect",
                    "druid::Pool::physical_connection_timeout",
                },
            )

    def test_reviewed_identifier_mapping_is_exact(self) -> None:
        self.assertEqual(
            candidate_target(
                "druid_wrapper::sqlx::SqlxConnectionAdapter::prepare_physical_statement"
            ),
            "druid_wrapper::sqlx::SqlxConnectionAdapter::prepare_statement",
        )
        self.assertEqual(
            candidate_target(
                "druid::core::PhysicalConnection::create_physical_statement"
            ),
            "druid_rdbc::connection::Connection::create_statement",
        )

    def test_unknown_removal_and_tsv_controls_fail_closed(self) -> None:
        with self.assertRaisesRegex(VerificationError, "unreviewed"):
            known_internal_or_deleted("druid::Type::physical_surprise")
        row = {
            "contract_kind": "PUBLIC_API",
            "package": "druid",
            "old_path": "druid::bad\tpath",
            "disposition": "APPROVED_BREAKING_DELETE",
            "target_path": "-",
            "baseline_signature_sha256": "a" * 64,
            "evidence_test": "test.rs#test",
            "notes": "bad",
        }
        with self.assertRaisesRegex(VerificationError, "delimiter"):
            render_rows((row,))


if __name__ == "__main__":
    unittest.main()
