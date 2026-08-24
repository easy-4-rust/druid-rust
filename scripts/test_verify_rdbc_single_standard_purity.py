#!/usr/bin/env python3
"""Fail-closed self-tests for the single-standard RDBC purity gate."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

from verify_rdbc_single_standard_purity import (  # noqa: E402
    REQUIRED_CANONICAL_MODULES,
    VerificationError,
    verify_tree,
)


class RdbcSingleStandardPurityTest(unittest.TestCase):
    """Verify exact standard files and reject product/runtime identities."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        for module in REQUIRED_CANONICAL_MODULES:
            directory = self.root / module
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "mod.rs").write_text(
                "mod contract;\npub use contract::Contract;\n",
                encoding="utf-8",
            )
            (directory / "contract.rs").write_text(
                "pub trait Contract: Send {}\n",
                encoding="utf-8",
            )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_accepts_complete_pure_standard_tree(self) -> None:
        """The positive control contains only standard declarations."""

        verify_tree(self.root, phase="transitional")

    def test_rejects_physical_and_product_runtime_identities(self) -> None:
        """Canonical files cannot name Physical, Druid, holder, lease, cache, or filter types."""

        polluted = self.root / "connection/contract.rs"
        for source in (
            "pub trait PhysicalConnection {}\n",
            "use druid::DruidPooledConnection;\n",
            "pub struct ConnectionHolder;\n",
            "pub struct ConnectionLease;\n",
            "pub struct PreparedCache;\n",
            "pub struct ConnectionFilter;\n",
            "pub struct ResourceOwner;\n",
            "pub struct PooledConnectionCache;\n",
            "pub use crate::spi::PhysicalConnection as Connection;\n",
        ):
            polluted.write_text(source, encoding="utf-8")
            with self.subTest(source=source), self.assertRaises(VerificationError):
                verify_tree(self.root, phase="transitional")

    def test_allows_exact_javax_sql_pool_names_only(self) -> None:
        """JDBC PooledConnection names survive while generic pool policy does not."""

        pooled = self.root / "data_source/contract.rs"
        pooled.write_text("pub trait PooledConnection: Send {}\n", encoding="utf-8")
        pool_source = self.root / "data_source/connection_pool_data_source.rs"
        pool_source.write_text(
            "pub trait ConnectionPoolDataSource: Send + Sync {\n"
            "    fn pooled_connection_with_credentials(&self);\n"
            "}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

        pool_source.write_text("pub struct NativePool;\n", encoding="utf-8")
        with self.assertRaises(VerificationError):
            verify_tree(self.root, phase="transitional")

    def test_rejects_type_definitions_in_module_roots(self) -> None:
        """mod.rs files remain declaration and re-export roots only."""

        module_root = self.root / "statement/mod.rs"
        module_root.write_text("pub trait Statement {}\n", encoding="utf-8")
        with self.assertRaises(VerificationError):
            verify_tree(self.root, phase="transitional")

    def test_rejects_missing_standard_file(self) -> None:
        """A partial scaffold cannot satisfy either purity phase."""

        (self.root / "wrapper/mod.rs").unlink()
        with self.assertRaises(VerificationError):
            verify_tree(self.root, phase="scaffold")

    def test_rejects_pollution_in_an_extra_canonical_file(self) -> None:
        """A newly added file cannot escape purity verification by missing a fixed list."""

        extra = self.root / "connection/hidden_backend.rs"
        extra.write_text("use backend_driver::Executor;\npub struct HiddenBackend;\n", encoding="utf-8")
        with self.assertRaises(VerificationError):
            verify_tree(self.root, phase="transitional")

    def test_rejects_non_allowlisted_backend_import(self) -> None:
        """Canonical files accept only std and declared druid-rdbc module imports."""

        source = self.root / "connection/contract.rs"
        source.write_text("use sqlx::Executor;\npub trait Contract: Send {}\n", encoding="utf-8")
        with self.assertRaises(VerificationError):
            verify_tree(self.root, phase="transitional")


if __name__ == "__main__":
    unittest.main(verbosity=2)
