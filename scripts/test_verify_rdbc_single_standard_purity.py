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
        root_modules = (*REQUIRED_CANONICAL_MODULES, "compat", "error", "resource", "spi", "types")
        (self.root / "lib.rs").write_text(
            "".join(f"pub mod {module};\n" for module in root_modules),
            encoding="utf-8",
        )
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
        for module, source in {
            "compat": "pub struct Compatibility;\n",
            "error": "pub struct RdbcError;\n",
            "resource": "pub struct Blob;\n",
            "spi": "pub struct TransitionalSpi;\n",
            "types": "pub struct Value;\n",
        }.items():
            directory = self.root / module
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "mod.rs").write_text(source, encoding="utf-8")

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
        pooled.write_text(
            "pub trait Contract: Send {}\n"
            "pub trait PooledConnection: Send {}\n",
            encoding="utf-8",
        )
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

    def test_accepts_inline_module_bound_from_its_declaring_scope(self) -> None:
        """An inline module is reachable without a sibling source file."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "mod inline { pub struct Visible; }\n"
            "use self::inline::Visible;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_nested_inline_module_path(self) -> None:
        """Nested inline module identities retain their complete module stack."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "mod outer { pub mod inner { pub struct Visible; } }\n"
            "use self::outer::inner::Visible;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_private_parent_item_from_descendant_module(self) -> None:
        """Rust privacy permits descendants to resolve private items from an ancestor module."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "struct ParentPrivate;\n"
            "mod child { pub type Visible = super::ParentPrivate; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_conventional_child_of_foo_rs(self) -> None:
        """A foo.rs module resolves its child from foo/child.rs."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "mod child;\n"
            "use self::child::Child;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        child = self.root / "connection/contract/child.rs"
        child.parent.mkdir(parents=True, exist_ok=True)
        child.write_text("pub struct Child;\n", encoding="utf-8")
        verify_tree(self.root, phase="transitional")

    def test_accepts_explicit_reexport_from_reachable_module(self) -> None:
        """A sibling may consume a public binding re-exported by a reachable module."""

        source = self.root / "statement/contract.rs"
        source.write_text(
            "use crate::connection::Contract as ConnectionContract;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_rejects_missing_reexport_even_when_a_physical_file_exists(self) -> None:
        """A physical file does not create a public binding without a declaration/re-export."""

        (self.root / "connection/hidden.rs").write_text(
            "pub struct Hidden;\n", encoding="utf-8"
        )
        source = self.root / "statement/contract.rs"
        source.write_text(
            "use crate::connection::Hidden;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "absent|missing|reachable|binding"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_ambiguous_reexport_binding(self) -> None:
        """Two re-exports cannot bind the same name to different reachable items."""

        module_root = self.root / "connection/mod.rs"
        module_root.write_text(
            "mod contract;\n"
            "mod left;\n"
            "mod right;\n"
            "pub use contract::Contract;\n"
            "pub use left::Thing;\n"
            "pub use right::Thing;\n",
            encoding="utf-8",
        )
        (self.root / "connection/left.rs").write_text(
            "pub struct Thing;\n", encoding="utf-8"
        )
        (self.root / "connection/right.rs").write_text(
            "pub struct Thing;\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(VerificationError, "ambiguous|duplicate|binding"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_hidden_physical_sibling_as_relative_module(self) -> None:
        """A physical sibling is not a module until its parent declares it."""

        (self.root / "connection/hidden.rs").write_text(
            "pub struct Hidden;\n", encoding="utf-8"
        )
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use super::hidden::Hidden;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "absent|missing|reachable|child"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_parent_traversal_to_transitional_resource(self) -> None:
        """Relative traversal cannot reach the transitional resource hierarchy."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "use super::super::resource::Blob;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "resource|transitional"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_super_traversal_above_crate_root(self) -> None:
        """Any number of super segments is parsed, and over-root traversal fails closed."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "use super::super::super::Contract as OverRootContract;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "above|over-root|super"):
            verify_tree(self.root, phase="transitional")

    def test_accepts_ordinary_path_assignment_outside_attribute(self) -> None:
        """An ordinary variable named path is not a Rust module path attribute."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Contract: Send {}\n"
            "pub fn ordinary_assignment() { let path = 1; let _ = path; }\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_rejects_direct_path_attribute(self) -> None:
        """A direct path attribute bypasses conventional module discovery."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            '#[path = "alternate.rs"] mod alternate;\n'
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "path"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_nested_cfg_attr_path_attribute(self) -> None:
        """Nested cfg_attr token ranges cannot hide a path attribute."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            '#[cfg_attr(any(), cfg_attr(all(), path = "alternate.rs"))] mod alternate;\n'
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "path"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_backend_attribute_path(self) -> None:
        """A procedural attribute path follows the same external-root policy as code paths."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "#[sqlx::test]\n"
            "pub fn backend_attribute() {}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "sqlx|external|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_qualified_include_macro(self) -> None:
        """Qualified include forms cannot bypass the reachable source denominator."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            'std::include!("hidden.rs");\n'
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "include"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_missing_external_module_file(self) -> None:
        """A semicolon module declaration must resolve to exactly one conventional file."""

        module_root = self.root / "connection/mod.rs"
        module_root.write_text(
            "mod contract;\nmod absent;\npub use contract::Contract;\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "absent|missing|module"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_simultaneous_rs_and_mod_rs_candidates(self) -> None:
        """Both conventional candidates for one module are ambiguous."""

        module_root = self.root / "connection/mod.rs"
        module_root.write_text(
            "mod contract;\nmod dual;\npub use contract::Contract;\n",
            encoding="utf-8",
        )
        (self.root / "connection/dual.rs").write_text(
            "pub struct Dual;\n", encoding="utf-8"
        )
        dual_directory = self.root / "connection/dual"
        dual_directory.mkdir(parents=True, exist_ok=True)
        (dual_directory / "mod.rs").write_text("pub struct Dual;\n", encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "ambiguous|both|module"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_unknown_qualified_root(self) -> None:
        """Capitalization cannot turn an unknown path root into a local item."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub type Alias = Mystery::Value;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "Mystery|unknown|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_unknown_module_scope_macro_edge(self) -> None:
        """An unknown module-scope macro may generate hidden module edges and fails closed."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "make_module!();\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "macro|unknown|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_non_allowlisted_backend_import(self) -> None:
        """Canonical files accept only std and declared druid-rdbc module imports."""

        source = self.root / "connection/contract.rs"
        source.write_text("use sqlx::Executor;\npub trait Contract: Send {}\n", encoding="utf-8")
        with self.assertRaises(VerificationError):
            verify_tree(self.root, phase="transitional")

    def test_rejects_backend_paths_macros_extern_and_tree_escape_edges(self) -> None:
        """Non-use paths and source-tree escape mechanisms fail closed."""

        source = self.root / "connection/contract.rs"
        for payload in (
            "pub type X = sqlx::Executor;\n",
            "pub type X = ::sqlx::Executor;\n",
            "pub type X = crate::resource::Blob;\n",
            "extern crate sqlx as backend;\n",
            'fn query() { let _ = sqlx::query!("select 1"); }\n',
            "mod backend { pub struct Executor; }\nuse self::backend::Executor;\n",
            '#[path = "../hidden.rs"] mod backend;\n',
            '#[cfg_attr(feature = "x", path = "../hidden.rs")] mod backend;\n',
            'include!("../hidden.rs");\n',
            "use sqlx::{Executor as BackendExecutor};\n",
            "use ::sqlx::Executor;\n",
        ):
            source.write_text(payload, encoding="utf-8")
            with self.subTest(payload=payload), self.assertRaises(VerificationError):
                verify_tree(self.root, phase="transitional")

    def test_allows_documentation_strings_and_grouped_standard_imports(self) -> None:
        """Token analysis ignores prose/literals and accepts grouped std/crate imports."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "/// sqlx::Executor is prose only.\n"
            "// backend_driver::Pool is a comment.\n"
            'const NOTE: &str = "sqlx::query!(backend)";\n'
            "use std::{any::TypeId, sync::Arc};\n"
            "use core::fmt::Debug;\n"
            "use alloc::vec::Vec;\n"
            "use crate::{error::RdbcError, types::Value};\n"
            "pub const BYTE_MAX: u8 = u8::MAX;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

        source.write_text(
            "use super::Contract as ParentContract;\n"
            "use super::super::statement::Contract as SiblingContract;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")


if __name__ == "__main__":
    unittest.main(verbosity=2)
