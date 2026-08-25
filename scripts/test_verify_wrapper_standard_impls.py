#!/usr/bin/env python3
"""Fail-closed tests for the P04 Task 4 wrapper-standard verifier."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from verify_wrapper_standard_impls import (
    AdapterRow,
    VerificationError,
    extract_impl_bodies,
    verify_adapter_source,
    verify_sqlx_ownership,
)


class WrapperStandardVerifierTest(unittest.TestCase):
    """Reject Physical-only, duplicated-core, and borrowed SQLx ownership fixtures."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.source_path = Path("crates/druid-wrapper/src/demo.rs")
        (self.root / self.source_path).parent.mkdir(parents=True)
        self.row = AdapterRow(
            source_path=self.source_path,
            current_trait="druid_rdbc::spi::PhysicalConnection",
            implementation_type="DemoConnection",
            target_trait="druid_rdbc::connection::Connection",
            state="STANDARD_IMPL_LIVE",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_source(self, source: str) -> None:
        (self.root / self.source_path).write_text(source, encoding="utf-8")

    def test_rejects_adapter_that_only_implements_physical(self) -> None:
        self.write_source("impl PhysicalConnection for DemoConnection {}\n")
        with self.assertRaisesRegex(VerificationError, "implements only Physical"):
            verify_adapter_source(self.root, self.row)

    def test_rejects_duplicated_sql_in_standard_impl(self) -> None:
        self.write_source(
            "impl PhysicalConnection for DemoConnection {}\n"
            "impl Connection for DemoConnection {\n"
            "  async fn create_statement(&mut self) { sqlx::query(\"select 1\").execute(); }\n"
            "}\n"
        )
        with self.assertRaisesRegex(VerificationError, "duplicates SQL"):
            verify_adapter_source(self.root, self.row)

    def test_accepts_parallel_impls_that_delegate_one_core(self) -> None:
        self.write_source(
            "impl PhysicalConnection for DemoConnection { fn run(&mut self) { self.run_core(); } }\n"
            "impl Connection for DemoConnection { fn run(&mut self) { self.run_core(); } }\n"
        )
        verify_adapter_source(self.root, self.row)

    def test_impl_parser_handles_generic_metadata(self) -> None:
        source = "impl<'a> DatabaseMetaData for DemoMetaData<'a> { fn x(&self) {} }"
        self.assertEqual(
            len(extract_impl_bodies(source, "druid_rdbc::metadata::DatabaseMetaData", "DemoMetaData<'_>")),
            1,
        )

    def test_rejects_missing_sqlx_actor(self) -> None:
        with self.assertRaisesRegex(VerificationError, "statement-owned executor"):
            verify_sqlx_ownership(self.root)


if __name__ == "__main__":
    unittest.main()
