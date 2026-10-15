#!/usr/bin/env python3
"""Fail-closed self-tests for the single-standard RDBC purity gate."""

from __future__ import annotations

import sys
import subprocess
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

    def assert_rustc(
        self,
        source: str,
        *,
        succeeds: bool,
        diagnostic: str | None = None,
        externs: dict[str, Path] | None = None,
    ) -> None:
        """Compile an independent minimal Rust source and assert its exact success class."""

        rustc_root = self.root / "_rustc_fixture"
        rustc_root.mkdir(exist_ok=True)
        source_path = rustc_root / "lib.rs"
        output_path = rustc_root / "libfixture.rlib"
        source_path.write_text(source, encoding="utf-8")
        command = [
            "rustc",
            "--edition=2021",
            "--crate-type=lib",
            str(source_path),
            "-o",
            str(output_path),
        ]
        for crate_name, crate_path in sorted((externs or {}).items()):
            command.extend(("--extern", f"{crate_name}={crate_path}"))
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
        )
        stderr = completed.stderr
        if succeeds:
            self.assertEqual(completed.returncode, 0, stderr)
        else:
            self.assertNotEqual(completed.returncode, 0, "rustc unexpectedly accepted fixture")
            if diagnostic is not None:
                self.assertIn(diagnostic, stderr)

    def install_canonical_wrapper_macro(self) -> None:
        """Install the checked canonical crate-root export in its real module identity."""

        wrapper_root = self.root / "wrapper/mod.rs"
        wrapper_root.write_text(
            wrapper_root.read_text(encoding="utf-8") + "pub mod wrapper;\n",
            encoding="utf-8",
        )
        (self.root / "wrapper/wrapper.rs").write_text(
            "#[macro_export]\n"
            "macro_rules! impl_standard_wrapper {\n"
            "    ($($tokens:tt)*) => {};\n"
            "}\n",
            encoding="utf-8",
        )

    def install_private_macro_alias_module(self) -> str:
        """Install a private helper macro re-exported through `crate::evil::assert`."""

        source = (
            "macro_rules! private_assert { ($($tokens:tt)*) => {}; }\n"
            "pub(crate) use private_assert as helper_assert;\n"
            "mod evil {\n"
            "    pub(crate) use crate::helper_assert as assert;\n"
            "}\n"
        )
        lib = self.root / "lib.rs"
        lib.write_text(
            lib.read_text(encoding="utf-8") + source,
            encoding="utf-8",
        )
        return source

    def install_transitional_type_alias_module(self, exported_name: str = "String") -> str:
        """Install a private module that re-exports transitional Blob as one path member."""

        source = (
            "mod evil {\n"
            f"    pub use crate::resource::Blob as {exported_name};\n"
            "}\n"
        )
        lib = self.root / "lib.rs"
        lib.write_text(
            lib.read_text(encoding="utf-8") + source,
            encoding="utf-8",
        )
        return source

    def build_test_proc_macro(self) -> Path:
        """Compile a minimal thiserror-shaped proc-macro oracle with rustc."""

        rustc_root = self.root / "_rustc_fixture"
        rustc_root.mkdir(exist_ok=True)
        source_path = rustc_root / "thiserror.rs"
        library_suffix = {"win32": ".dll", "darwin": ".dylib"}.get(
            sys.platform,
            ".so",
        )
        output_path = rustc_root / f"libthiserror{library_suffix}"
        source_path.write_text(
            "extern crate proc_macro;\n"
            "use proc_macro::TokenStream;\n"
            "#[proc_macro_derive(Error, attributes(error))]\n"
            "pub fn derive_error(_: TokenStream) -> TokenStream { TokenStream::new() }\n"
            "#[proc_macro_attribute]\n"
            "pub fn marker(_: TokenStream, item: TokenStream) -> TokenStream { item }\n"
            "#[proc_macro]\n"
            "pub fn assert(_: TokenStream) -> TokenStream { \"()\".parse().unwrap() }\n",
            encoding="utf-8",
        )
        completed = subprocess.run(
            [
                "rustc",
                "--edition=2021",
                "--crate-name=thiserror",
                "--crate-type=proc-macro",
                str(source_path),
                "-o",
                str(output_path),
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return output_path

    def install_proc_macro_alias_module(self) -> str:
        """Install private derive, attribute, and function-like proc-macro re-exports."""

        source = (
            "mod evil {\n"
            "    pub use thiserror::{assert, marker, Error as Debug};\n"
            "}\n"
        )
        lib = self.root / "lib.rs"
        lib.write_text(
            lib.read_text(encoding="utf-8") + source,
            encoding="utf-8",
        )
        return source

    def assert_rejects_ordinary_standard_alias(self, root_name: str) -> None:
        """Prove one relative standard-looking type root resolves to the local alias first."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "mod connection {\n"
            f"    use crate::evil as {root_name};\n"
            f"    pub type Leaked = {root_name}::String;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            f"use crate::evil as {root_name};\n"
            f"pub type Leaked = {root_name}::String;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            VerificationError,
            f"{root_name}|transitional|binding|provenance",
        ):
            verify_tree(self.root, phase="transitional")

    def assert_rejects_derive_standard_alias(self, root_name: str) -> None:
        """Prove a derive root resolves through a local proc-macro re-export first."""

        proc_macro = self.build_test_proc_macro()
        helper_source = self.install_proc_macro_alias_module()
        rust_source = (
            helper_source
            + "mod connection {\n"
            f"    use crate::evil as {root_name};\n"
            f"    #[derive(::std::fmt::Debug, {root_name}::Debug)]\n"
            "    #[error(\"x\")]\n"
            "    pub struct Subject;\n"
            "}\n"
        )
        self.assert_rustc(
            rust_source,
            succeeds=True,
            externs={"thiserror": proc_macro},
        )
        source = self.root / "connection/contract.rs"
        source.write_text(
            f"use crate::evil as {root_name};\n"
            f"#[derive(::std::fmt::Debug, {root_name}::Debug)]\n"
            "#[error(\"x\")]\n"
            "pub struct Subject;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            VerificationError,
            f"{root_name}|Debug|binding|provenance",
        ):
            verify_tree(self.root, phase="transitional")

    def test_accepts_complete_pure_standard_tree(self) -> None:
        """The positive control contains only standard declarations."""

        verify_tree(self.root, phase="transitional")

    def test_final_phase_includes_canonical_resource_contracts(self) -> None:
        """Final purity counts JDBC resource contracts and permits their canonical paths."""

        (self.root / "resource/mod.rs").write_text(
            "mod blob;\npub use blob::Blob;\n", encoding="utf-8"
        )
        (self.root / "resource/blob.rs").write_text(
            "pub trait Blob: Send {}\n", encoding="utf-8"
        )
        (self.root / "connection/contract.rs").write_text(
            "use crate::resource::Blob;\npub trait Contract: Blob {}\n",
            encoding="utf-8",
        )

        verified = verify_tree(self.root, phase="final")
        self.assertIn(Path("resource/blob.rs"), verified)

        (self.root / "resource/blob.rs").write_text(
            "pub trait Blob: Send {}\npub struct ConnectionHolder;\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(VerificationError, r"resource[/\\]blob.rs"):
            verify_tree(self.root, phase="final")

    def test_accepts_reachable_canonical_file_module(self) -> None:
        """A single-object file module is a complete canonical root."""

        self.install_final_resource_module()
        with (self.root / "lib.rs").open("a", encoding="utf-8") as root:
            root.write("pub mod url;\n")
        (self.root / "url.rs").write_text(
            "pub struct RdbcConnectionUrl;\n", encoding="utf-8"
        )

        verified = verify_tree(self.root, phase="final")
        self.assertIn(Path("url.rs"), verified)

    def test_file_module_root_and_hidden_child_are_checked(self) -> None:
        """File-module layout cannot hide forbidden production identities."""

        self.install_final_resource_module()
        with (self.root / "lib.rs").open("a", encoding="utf-8") as root:
            root.write("pub mod url;\n")
        module = self.root / "url.rs"
        module.write_text("pub struct DruidPool;\n", encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "DruidPool"):
            verify_tree(self.root, phase="final")

        module.write_text("pub struct RdbcConnectionUrl;\n", encoding="utf-8")
        (self.root / "url").mkdir()
        (self.root / "url/hidden.rs").write_text(
            "pub struct DruidPool;\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(VerificationError, "unreachable|DruidPool"):
            verify_tree(self.root, phase="final")

    def test_rejects_ambiguous_canonical_file_and_directory_roots(self) -> None:
        """The gate must not choose a root when Rust has two candidates."""

        self.install_final_resource_module()
        with (self.root / "lib.rs").open("a", encoding="utf-8") as root:
            root.write("pub mod url;\n")
        (self.root / "url.rs").write_text(
            "pub struct RdbcConnectionUrl;\n", encoding="utf-8"
        )
        (self.root / "url").mkdir()
        (self.root / "url/mod.rs").write_text(
            "pub struct OtherUrl;\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(VerificationError, "ambiguous|both|module"):
            verify_tree(self.root, phase="final")

    def install_final_resource_module(self) -> None:
        """Give a final-phase fixture a canonical resource object."""

        (self.root / "resource/mod.rs").write_text(
            "mod blob;\npub use blob::Blob;\n", encoding="utf-8"
        )
        (self.root / "resource/blob.rs").write_text(
            "pub trait Blob: Send {}\n", encoding="utf-8"
        )

    def test_final_phase_accepts_external_bitflags_macro_only(self) -> None:
        """Resource capability flags may use the external bitflags macro, never a local alias."""

        (self.root / "resource/mod.rs").write_text(
            "mod flags;\npub use flags::Flags;\n", encoding="utf-8"
        )
        flags = self.root / "resource/flags.rs"
        flags.write_text(
            "bitflags::bitflags! { pub struct Flags: u8 { const READ = 1; } }\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="final")

        flags.write_text(
            "use crate::compat as bitflags;\n"
            "bitflags::bitflags! { pub struct Flags: u8 { const READ = 1; } }\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "bitflags|provenance"):
            verify_tree(self.root, phase="final")

    def test_accepts_unshadowed_standard_format_args_macro(self) -> None:
        """The standard formatting macro keeps the same provenance check as format!."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            'pub fn args() { let _ = format_args!("x"); }\n'
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

        source.write_text(
            'macro_rules! format_args { ($value:literal) => { $value }; }\n'
            'pub fn args() { let _ = format_args!("x"); }\n'
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "format_args|shadow"):
            verify_tree(self.root, phase="transitional")

    def test_accepts_unshadowed_standard_matches_macro(self) -> None:
        """The standard matches! prelude macro is accepted only without a local shadow."""

        source = self.root / "data_source/contract.rs"
        source.write_text(
            'pub fn classify(value: Option<u8>) -> bool { matches!(value, Some(1)) }\n'
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

        source.write_text(
            "macro_rules! matches { ($value:expr, $pattern:pat) => { true }; }\n"
            'pub fn classify(value: Option<u8>) -> bool { matches!(value, Some(1)) }\n'
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "matches|shadow"):
            verify_tree(self.root, phase="final")

    def test_allows_tokio_dependency_without_local_alias_spoofing(self) -> None:
        """Async standard contracts may use Tokio only through real external provenance."""

        source = self.root / "data_source/contract.rs"
        source.write_text(
            "use parking_lot::Mutex;\n"
            "use tokio::sync::Mutex as TokioMutex;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

        source.write_text(
            "use crate::compat as tokio;\n"
            "use tokio::sync::Mutex;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "tokio|compat|sync"):
            verify_tree(self.root, phase="transitional")

        source.write_text(
            "use crate::compat as parking_lot;\n"
            "use parking_lot::Mutex;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "parking_lot|compat|Mutex"):
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

    def test_release_savepoint_is_not_a_lease_runtime_identity(self) -> None:
        """Identifier components distinguish JDBC release from a Druid lease helper."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Contract: Send { fn release_savepoint(&mut self); }\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

        source.write_text(
            "pub trait Contract: Send { fn lease_handle(&mut self); }\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "lease_handle|runtime"):
            verify_tree(self.root, phase="transitional")

    def test_allows_iterator_filter_method_but_rejects_filter_type(self) -> None:
        """The standard iterator method is not a Druid filter contract."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn values() -> Vec<i32> { [1, 2].into_iter().filter(|v| *v > 1).filter_map(Some).collect() }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

        source.write_text("pub struct Filter;\npub trait Contract: Send {}\n", encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "Filter"):
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

    def test_rejects_unreachable_physical_standard_file(self) -> None:
        """Dead source cannot masquerade as implemented canonical API."""

        extra = self.root / "resource/dead_contract.rs"
        extra.write_text("pub trait DeadContract: Send {}\n", encoding="utf-8")
        (self.root / "resource/mod.rs").write_text(
            "mod blob;\npub use blob::Blob;\n", encoding="utf-8"
        )
        (self.root / "resource/blob.rs").write_text(
            "pub trait Blob: Send {}\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(VerificationError, r"resource[/\\]dead_contract.rs"):
            verify_tree(self.root, phase="final")

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

    def test_accepts_function_type_generic_associated_path(self) -> None:
        """A function type parameter is a legal associated-path root inside its declaration."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub fn value<T: Assoc>() { let _: Option<T::Item> = None; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_impl_type_generic_associated_path(self) -> None:
        """An impl type parameter remains bound throughout the impl body."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub struct Subject<T>(T);\n"
            "impl<T: Assoc> Subject<T> { pub fn value(_: T::Item) {} }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_associated_method_type_generic_path(self) -> None:
        """A method's own type parameter is bound inside an otherwise non-generic impl."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub struct Subject;\n"
            "impl Subject { pub fn value<T: Assoc>(_: T::Item) {} }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_trait_type_generic_associated_path(self) -> None:
        """A trait type parameter remains bound throughout its item body."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub trait GenericTrait<T: Assoc> { fn value(_: Option<T::Item>); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_struct_type_generic_associated_path(self) -> None:
        """A struct type parameter remains bound throughout its field body."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub struct GenericStruct<T: Assoc> { value: Option<T::Item> }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_type_alias_generic_associated_path(self) -> None:
        """A type-alias parameter remains bound through the alias expression."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub type GenericAlias<T: Assoc> = Option<T::Item>;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_where_clause_generic_associated_path(self) -> None:
        """A type parameter remains bound in its declaration's where clause and body."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub fn value<T>() where T: Assoc, T::Item: Sized { let _: Option<T::Item> = None; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_lifetime_with_type_generic_associated_path(self) -> None:
        """Lifetime parameters are retained syntactically but never treated as path roots."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub fn borrow<'a, T: Assoc>(value: &'a T::Item) -> &'a T::Item { value }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_const_generic_in_associated_path_arguments(self) -> None:
        """A const parameter is legal inside a type-rooted associated path in its function range."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub struct ConstCarrier<const WIDTH: usize>;\n"
            "impl<const WIDTH: usize> ConstCarrier<WIDTH> { pub const VALUE: usize = WIDTH; }\n"
            "pub fn width<const WIDTH: usize>() { let _ = ConstCarrier::<WIDTH>::VALUE; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_rejects_const_generic_as_associated_path_root(self) -> None:
        """A const generic binding is a value and cannot masquerade as a type path root."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn invalid<const WIDTH: usize>() { let _ = WIDTH::Item; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "WIDTH|const|unknown|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_undeclared_generic_like_associated_root(self) -> None:
        """An uppercase identifier is not a generic binding without a lexical declaration."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn value() { let _: Option<T::Item> = None; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "unknown|root|T"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_external_trait_in_generic_projection(self) -> None:
        """A legal T binding cannot hide a backend trait in a qualified projection."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn value<T>() { let _: Option<<T as sqlx::ExternalTrait>::Item> = None; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "sqlx|external|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_unknown_root_in_generic_bound(self) -> None:
        """Generic binding collection never turns an unknown bound path into a legal root."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn value<T: Unknown::Trait>() { let _: Option<T::Item> = None; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "Unknown|unknown|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_unknown_bare_generic_bound(self) -> None:
        """A bare bound name must resolve; capitalization is not source proof."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn value<T: MissingBound>() { let _: Option<T::Item> = None; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "MissingBound|unknown|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_unknown_bare_where_bound(self) -> None:
        """A where-clause bound follows the same canonical root policy as inline bounds."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn value<T>() where T: MissingBound { let _: Option<T::Item> = None; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "MissingBound|unknown|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_transitional_root_in_generic_bound(self) -> None:
        """A generic bound cannot enter the transitional resource hierarchy."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn value<T: crate::resource::Blob>() { let _: Option<T::Item> = None; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "resource|transitional"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_function_generic_binding_after_its_range(self) -> None:
        """A function generic cannot leak into a following module item."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub fn declares<T: Assoc>() {}\n"
            "pub type Escaped = T::Item;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "unknown|root|T"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_function_generic_inside_nested_item(self) -> None:
        """A nested item cannot capture a generic declared by its enclosing function."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub fn outer<T: Assoc>() { fn nested() { let _: Option<T::Item> = None; } }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "unknown|root|T|nested"):
            verify_tree(self.root, phase="transitional")

    def test_accepts_nested_generic_item_shadow_and_outer_call(self) -> None:
        """A nested function owns its shadow generic and is callable from its enclosing block."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub fn outer<T: Assoc>() {\n"
            "    fn inner<T: Assoc>() { let _: Option<T::Item> = None; }\n"
            "    inner::<T>();\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_nested_local_function_struct_and_type_bindings(self) -> None:
        """Function, struct, and type items bind throughout their containing block."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn outer() {\n"
            "    fn helper<T>() {}\n"
            "    struct Local;\n"
            "    impl Local { const VALUE: usize = 1; }\n"
            "    type Alias = Local;\n"
            "    helper::<u8>();\n"
            "    let _ = Local::VALUE;\n"
            "    let _ = Alias::VALUE;\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_nested_local_module_type_namespace(self) -> None:
        """A nested module remains a type-namespace path root inside its containing block."""

        rust_source = (
            "pub fn outer() {\n"
            "    mod local { pub struct External; }\n"
            "    let _: Option<local::External> = None;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_rejects_nested_item_binding_after_block_range(self) -> None:
        """A block-local item binding cannot leak into a following function."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn outer() { fn inner<T>() {} inner::<u8>(); }\n"
            "pub fn escape() { inner::<u8>(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "inner|unknown|root|range"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_impl_generic_binding_after_its_range(self) -> None:
        """An impl generic cannot leak into a following module item."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub trait Assoc { type Item; }\n"
            "pub struct Subject<T>(T);\n"
            "impl<T: Assoc> Subject<T> {}\n"
            "pub type Escaped = T::Item;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "unknown|root|T"):
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

    def test_rejects_unknown_crate_root_macro_edge(self) -> None:
        """A crate-root invocation cannot generate unindexed canonical module edges."""

        attack = (
            "macro_rules! make_standard { () => {}; }\n"
            "make_standard!();\n"
        )
        self.assert_rustc(attack, succeeds=True)
        lib = self.root / "lib.rs"
        lib.write_text(lib.read_text(encoding="utf-8") + attack, encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "make_standard|macro|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_accepts_uninvoked_macro_rules_generic_looking_body(self) -> None:
        """Unexpanded macro arms and nested delimiters do not create runtime path roots."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "macro_rules! generated_items {\n"
            "    ($name:ident) => {{\n"
            "        fn $name<T>() {\n"
            "            if true { { let _: Option<T::Item> = None; } }\n"
            "        }\n"
            "    }};\n"
            "    ($($token:tt)*) => {{{ $($token)* }}};\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_rejects_module_macro_invoked_in_function_with_transitional_body(self) -> None:
        """A compilable module-defined macro invocation cannot hide a transitional path."""

        rust_source = (
            "mod resource { pub struct Blob; }\n"
            "macro_rules! leak {\n"
            "    () => {{ let _: Option<crate::resource::Blob> = None; }};\n"
            "}\n"
            "pub fn run() { leak!(); }\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "macro_rules! leak {\n"
            "    () => {{ let _: Option<crate::resource::Blob> = None; }};\n"
            "}\n"
            "pub fn run() { leak!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "leak|macro|allow"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_function_local_macro_invoked_in_nested_block(self) -> None:
        """Function-local definitions and nested blocks cannot bypass invocation policy."""

        rust_source = (
            "mod resource { pub struct Blob; }\n"
            "pub fn run() {\n"
            "    macro_rules! leak {\n"
            "        () => {{ let _: Option<crate::resource::Blob> = None; }};\n"
            "    }\n"
            "    if true { { leak!(); } }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn run() {\n"
            "    macro_rules! leak {\n"
            "        () => {{ let _: Option<crate::resource::Blob> = None; }};\n"
            "    }\n"
            "    if true { { leak!(); } }\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "leak|macro|allow"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_unknown_bare_macro_invocation_in_nested_block(self) -> None:
        """An unproven bare invocation fails closed at any function/block depth."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn run() { if true { { mystery!(); } } }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "mystery|macro|allow"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_local_impl_standard_wrapper_shadow(self) -> None:
        """A local textual macro cannot impersonate the approved crate-root export."""

        rust_source = (
            "macro_rules! impl_standard_wrapper { () => {}; }\n"
            "pub fn run() { impl_standard_wrapper!(); }\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "impl_standard_wrapper|macro|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_local_format_shadow(self) -> None:
        """Bare format resolves to an active textual definition before the standard prelude."""

        rust_source = (
            "macro_rules! format { ($($tokens:tt)*) => { String::new() }; }\n"
            "pub fn run() { let _ = format!(\"{}\", 1); }\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "format|macro|shadow|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_imported_bare_format_shadow(self) -> None:
        """A path-imported macro named format shadows the standard-prelude candidate."""

        attack = (
            "macro_rules! private_format { ($($tokens:tt)*) => { String::new() }; }\n"
            "pub(crate) use private_format as helper_format;\n"
        )
        lib = self.root / "lib.rs"
        lib.write_text(lib.read_text(encoding="utf-8") + attack, encoding="utf-8")
        rust_source = (
            attack
            + "mod connection {\n"
            "    use crate::helper_format as format;\n"
            "    pub fn run() { let _ = format!(\"{}\", 1); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::helper_format as format;\n"
            "pub fn run() { let _ = format!(\"{}\", 1); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "format|import|shadow|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_imported_bare_impl_standard_wrapper_shadow(self) -> None:
        """A path-imported same-name macro cannot satisfy the canonical wrapper contract."""

        attack = (
            "macro_rules! private_wrapper { () => {}; }\n"
            "pub(crate) use private_wrapper as helper_wrapper;\n"
        )
        lib = self.root / "lib.rs"
        lib.write_text(lib.read_text(encoding="utf-8") + attack, encoding="utf-8")
        rust_source = (
            attack
            + "mod connection {\n"
            "    use crate::helper_wrapper as impl_standard_wrapper;\n"
            "    pub fn run() { impl_standard_wrapper!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::helper_wrapper as impl_standard_wrapper;\n"
            "pub fn run() { impl_standard_wrapper!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            VerificationError,
            "impl_standard_wrapper|import|canonical|provenance",
        ):
            verify_tree(self.root, phase="transitional")

    def test_rejects_macro_use_scope_injection(self) -> None:
        """macro_use injects untracked textual bindings and therefore fails closed."""

        rust_source = (
            "#[macro_use]\n"
            "mod evil {\n"
            "    macro_rules! format { ($($tokens:tt)*) => { String::new() }; }\n"
            "}\n"
            "pub fn run() { let _ = format!(\"{}\", 1); }\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "macro_use|macro|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_raw_approved_macro_name(self) -> None:
        """A raw identifier cannot bypass provenance by canonicalizing to an approved name."""

        self.install_canonical_wrapper_macro()
        rust_source = (
            "pub mod wrapper {\n"
            "    pub mod wrapper {\n"
            "        #[macro_export]\n"
            "        macro_rules! impl_standard_wrapper { ($($tokens:tt)*) => {}; }\n"
            "    }\n"
            "}\n"
            "mod connection {\n"
            "    use crate::impl_standard_wrapper;\n"
            "    pub fn run() { r#impl_standard_wrapper!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::impl_standard_wrapper;\n"
            "pub fn run() { r#impl_standard_wrapper!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "raw|impl_standard_wrapper|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_private_helper_reexport_shadowing_std(self) -> None:
        """A grouped alias to a private macro re-export cannot impersonate std."""

        helper_source = self.install_private_macro_alias_module()
        rust_source = (
            helper_source
            + "mod connection {\n"
            "    use crate::{evil as std};\n"
            "    pub fn run() { std::assert!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::{evil as std};\n"
            "pub fn run() { std::assert!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|macro|shadow|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_private_helper_reexport_shadowing_core(self) -> None:
        """A local alias named core does not have core-macro provenance."""

        helper_source = self.install_private_macro_alias_module()
        rust_source = (
            helper_source
            + "mod connection {\n"
            "    use crate::evil as core;\n"
            "    pub fn run() { core::assert!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as core;\n"
            "pub fn run() { core::assert!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "core|macro|shadow|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_private_helper_reexport_shadowing_alloc(self) -> None:
        """A local alias named alloc does not have alloc-macro provenance."""

        helper_source = self.install_private_macro_alias_module()
        rust_source = (
            helper_source
            + "mod connection {\n"
            "    use crate::evil as alloc;\n"
            "    pub fn run() { alloc::assert!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as alloc;\n"
            "pub fn run() { alloc::assert!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "alloc|macro|shadow|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_same_name_pub_use_chain_as_canonical_macro(self) -> None:
        """A same-name macro re-export chain is not the canonical wrapper definition."""

        attack = (
            "macro_rules! private_wrapper { () => {}; }\n"
            "pub(crate) use private_wrapper as first_export;\n"
            "mod first {\n"
            "    pub(crate) use crate::first_export as second_export;\n"
            "}\n"
            "mod second {\n"
            "    pub(crate) use crate::first::second_export as impl_standard_wrapper;\n"
            "}\n"
            "pub(crate) use second::impl_standard_wrapper;\n"
        )
        lib = self.root / "lib.rs"
        lib.write_text(lib.read_text(encoding="utf-8") + attack, encoding="utf-8")
        rust_source = (
            attack
            + "mod connection {\n"
            "    pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "impl_standard_wrapper|canonical|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_conditional_macro_export_provenance(self) -> None:
        """cfg_attr macro_export is conditional and cannot prove the canonical root binding."""

        wrapper_root = self.root / "wrapper/mod.rs"
        wrapper_root.write_text(
            wrapper_root.read_text(encoding="utf-8") + "pub mod wrapper;\n",
            encoding="utf-8",
        )
        conditional_macro = (
            "#[cfg_attr(all(), macro_export)]\n"
            "macro_rules! impl_standard_wrapper { ($($tokens:tt)*) => {}; }\n"
        )
        (self.root / "wrapper/wrapper.rs").write_text(
            conditional_macro,
            encoding="utf-8",
        )
        rust_source = (
            "pub mod wrapper { pub mod wrapper {\n"
            + conditional_macro
            + "} }\n"
            "mod connection {\n"
            "    pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "impl_standard_wrapper|canonical|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_right_scope_macro_from_wrong_source(self) -> None:
        """The canonical module identity alone cannot replace its checked source origin."""

        wrapper_root = self.root / "wrapper/mod.rs"
        inline_macro = (
            "pub mod wrapper {\n"
            "    #[macro_export]\n"
            "    macro_rules! impl_standard_wrapper { ($($tokens:tt)*) => {}; }\n"
            "}\n"
        )
        wrapper_root.write_text(
            wrapper_root.read_text(encoding="utf-8") + inline_macro,
            encoding="utf-8",
        )
        rust_source = (
            "pub mod wrapper {\n"
            + inline_macro
            + "}\n"
            "mod connection {\n"
            "    pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "impl_standard_wrapper|source|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_non_module_canonical_macro_export(self) -> None:
        """A non-local macro_export cannot impersonate the module-level canonical definition."""

        wrapper_root = self.root / "wrapper/mod.rs"
        wrapper_root.write_text(
            wrapper_root.read_text(encoding="utf-8") + "pub mod wrapper;\n",
            encoding="utf-8",
        )
        nested_export = (
            "pub fn define() {\n"
            "    #[macro_export]\n"
            "    macro_rules! impl_standard_wrapper { ($($tokens:tt)*) => {}; }\n"
            "}\n"
        )
        (self.root / "wrapper/wrapper.rs").write_text(
            nested_export,
            encoding="utf-8",
        )
        rust_source = (
            "pub mod wrapper { pub mod wrapper {\n"
            + nested_export
            + "} }\n"
            "pub fn run() { crate::impl_standard_wrapper!(); }\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "impl_standard_wrapper|module|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_lexical_use_alias_shadowing_std(self) -> None:
        """A function/block use alias shadows std only inside its lexical range."""

        helper_source = self.install_private_macro_alias_module()
        rust_source = (
            helper_source
            + "pub fn run() {\n"
            "    if true { use crate::evil as std; std::assert!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn run() {\n"
            "    if true { use crate::evil as std; std::assert!(); }\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|lexical|shadow|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_parent_textual_macro_inside_nested_inline_module(self) -> None:
        """Textual macro scope propagates from a parent into later inline descendants."""

        rust_source = (
            "macro_rules! format { () => { String::new() }; }\n"
            "pub mod outer {\n"
            "    pub mod inner { pub fn run() { let _ = format!(); } }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "format|textual|shadow|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_parent_textual_macro_inside_external_child_module(self) -> None:
        """Textual macro scope propagates into an external child declared after it."""

        rust_source = (
            "macro_rules! format { () => { String::new() }; }\n"
            "pub mod child { pub fn run() { let _ = format!(); } }\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "macro_rules! format { () => { String::new() }; }\n"
            "pub mod child;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        child = self.root / "connection/contract/child.rs"
        child.parent.mkdir(parents=True, exist_ok=True)
        child.write_text(
            "pub fn run() { let _ = format!(); }\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "format|textual|shadow|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_shadowed_approved_macro_across_source_positions(self) -> None:
        """Definition, type, impl, closure, nested, and cfg sites all receive provenance checks."""

        sources = (
            (
                "definition",
                "macro_rules! format { () => { () }; }\n"
                "pub const GENERATED: () = format!();\n",
            ),
            (
                "type",
                "macro_rules! format { () => { u8 }; }\n"
                "pub type Generated = format!();\n",
            ),
            (
                "impl",
                "pub struct Subject;\n"
                "macro_rules! format { () => { pub fn generated() {} }; }\n"
                "impl Subject { format!(); }\n",
            ),
            (
                "closure",
                "macro_rules! format { () => { () }; }\n"
                "pub fn run() { let generated = || format!(); generated(); }\n",
            ),
            (
                "nested",
                "macro_rules! format { () => { () }; }\n"
                "pub fn run() { if true { { format!(); } } }\n",
            ),
            (
                "cfg",
                "#[cfg(any())]\n"
                "pub fn hidden() { impl_standard_wrapper!(); }\n",
            ),
        )
        source = self.root / "connection/contract.rs"
        for position, rust_source in sources:
            with self.subTest(position=position):
                self.assert_rustc(rust_source, succeeds=True)
                source.write_text(
                    rust_source + "pub trait Contract: Send {}\n",
                    encoding="utf-8",
                )
                with self.assertRaisesRegex(
                    VerificationError,
                    "format|impl_standard_wrapper|macro|shadow|provenance",
                ):
                    verify_tree(self.root, phase="transitional")

    def test_accepts_proven_canonical_macro_despite_local_textual_shadow(self) -> None:
        """crate:: qualification ignores textual shadow and resolves the reviewed export."""

        self.install_canonical_wrapper_macro()
        rust_source = (
            "pub mod wrapper {\n"
            "    pub mod wrapper {\n"
            "        #[macro_export]\n"
            "        macro_rules! impl_standard_wrapper { ($($tokens:tt)*) => {}; }\n"
            "    }\n"
            "}\n"
            "mod connection {\n"
            "    macro_rules! impl_standard_wrapper { () => {}; }\n"
            "    pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "macro_rules! impl_standard_wrapper { () => {}; }\n"
            "pub fn run() { crate::impl_standard_wrapper!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_unshadowed_std_core_alloc_and_format_macros(self) -> None:
        """Proven standard roots and the unshadowed format prelude remain available."""

        lib = self.root / "lib.rs"
        lib.write_text("extern crate alloc;\n" + lib.read_text(encoding="utf-8"), encoding="utf-8")
        rust_source = (
            "extern crate alloc;\n"
            "pub fn allowed() {\n"
            "    std::assert!(true);\n"
            "    core::assert!(true);\n"
            "    let _: alloc::vec::Vec<u8> = alloc::vec![1];\n"
            "    let _ = format!(\"{}\", 1);\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source.removeprefix("extern crate alloc;\n")
            + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_macro_after_textual_and_alias_shadow_ranges_end(self) -> None:
        """Textual and use shadows stop at block end and do not poison later invocations."""

        helper_source = self.install_private_macro_alias_module()
        rust_source = (
            helper_source
            + "pub fn allowed() {\n"
            "    { macro_rules! format { () => { String::new() }; } }\n"
            "    { use crate::evil as std; }\n"
            "    let _ = format!(\"{}\", 1);\n"
            "    std::assert!(true);\n"
            "}\n"
            "pub fn before_shadow() {\n"
            "    let _ = format!(\"{}\", 1);\n"
            "    macro_rules! format { () => { String::new() }; }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source.removeprefix(helper_source) + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_absolute_std_macro_despite_relative_alias(self) -> None:
        """Leading absolute standard roots ignore same-name relative aliases."""

        helper_source = self.install_private_macro_alias_module()
        lib = self.root / "lib.rs"
        lib.write_text("extern crate alloc;\n" + lib.read_text(encoding="utf-8"), encoding="utf-8")
        rust_source = (
            "extern crate alloc;\n"
            + helper_source
            + "mod connection {\n"
            "    use crate::evil as std;\n"
            "    use crate::evil as core;\n"
            "    use crate::evil as alloc;\n"
            "    pub fn run() {\n"
            "        ::std::assert!(true);\n"
            "        ::core::assert!(true);\n"
            "        let _: ::alloc::vec::Vec<u8> = ::alloc::vec![1];\n"
            "    }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as std;\n"
            "use crate::evil as core;\n"
            "use crate::evil as alloc;\n"
            "pub fn run() {\n"
            "    ::std::assert!(true);\n"
            "    ::core::assert!(true);\n"
            "    let _: ::alloc::vec::Vec<u8> = ::alloc::vec![1];\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_explicitly_allowlisted_macro_invocations(self) -> None:
        """Qualified std/core and the reviewed bare format macro remain available."""

        rust_source = (
            "pub fn allowed() {\n"
            "    std::assert!(true);\n"
            "    core::assert!(true);\n"
            '    let _ = format!("{}", 1);\n'
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_inequality_without_macro_misclassification(self) -> None:
        """A compilable != comparison is punctuation, not a value! invocation."""

        rust_source = (
            "pub fn different(value: i32) -> bool {\n"
            "    value != 0\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(rust_source + "pub trait Contract: Send {}\n", encoding="utf-8")
        verify_tree(self.root, phase="transitional")

    def test_accepts_boolean_negation_without_macro_misclassification(self) -> None:
        """Rust keyword plus unary ! is not a named macro invocation."""

        rust_source = (
            "pub fn negate(mut value: bool) {\n"
            "    if !value { value = true; }\n"
            "    while !value { break; }\n"
            "    let _ = !value;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_rejects_actual_external_macro_path(self) -> None:
        """Skipping definitions never permits an actual backend macro invocation."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            'pub fn query() { let _ = sqlx::query!("select 1"); }\n'
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "sqlx|external|root"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_nested_function_as_type_or_module_namespace(self) -> None:
        """A value-namespace function binding cannot legalize a multi-segment type path."""

        rust_source = (
            "pub fn outer() {\n"
            "    fn sqlx() {}\n"
            "    let _ = sqlx::External;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=False, diagnostic="E0433")
        source = self.root / "connection/contract.rs"
        source.write_text(
            rust_source + "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "sqlx|function|namespace"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_lifetime_as_associated_path_root(self) -> None:
        """A lifetime token followed by :: is never a type or module path root."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn invalid<'a>() { let _ = 'a::Item; }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "lifetime|'a|root"):
            verify_tree(self.root, phase="transitional")

    def test_accepts_char_literal_and_loop_label_boundaries(self) -> None:
        """Character literals and single-colon loop labels are not lifetime path roots."""

        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn labels<'a>(value: &'a str) -> &'a str {\n"
            "    'outer: loop { break 'outer; }\n"
            "    let _character = 'a';\n"
            "    value\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_rejects_relative_std_alias_in_ordinary_type_path(self) -> None:
        """A local std alias cannot hide a transitional type behind std::String."""

        self.assert_rejects_ordinary_standard_alias("std")

    def test_rejects_relative_core_alias_in_ordinary_type_path(self) -> None:
        """A local core alias cannot hide a transitional type behind core::String."""

        self.assert_rejects_ordinary_standard_alias("core")

    def test_rejects_relative_alloc_alias_in_ordinary_type_path(self) -> None:
        """A local alloc alias cannot hide a transitional type behind alloc::String."""

        self.assert_rejects_ordinary_standard_alias("alloc")

    def test_rejects_relative_std_alias_in_derive_path(self) -> None:
        """A relative std::Debug derive resolves the local proc-macro binding first."""

        self.assert_rejects_derive_standard_alias("std")

    def test_rejects_relative_core_alias_in_derive_path(self) -> None:
        """A relative core::Debug derive resolves the local proc-macro binding first."""

        self.assert_rejects_derive_standard_alias("core")

    def test_rejects_relative_alloc_alias_in_derive_path(self) -> None:
        """A relative alloc::Debug derive resolves the local proc-macro binding first."""

        self.assert_rejects_derive_standard_alias("alloc")

    def test_rejects_relative_std_alias_in_proc_attribute_path(self) -> None:
        """A qualified procedural attribute uses the same local-first root resolution."""

        proc_macro = self.build_test_proc_macro()
        helper_source = self.install_proc_macro_alias_module()
        rust_source = (
            helper_source
            + "mod connection {\n"
            "    use crate::evil as std;\n"
            "    #[std::marker]\n"
            "    pub struct Subject;\n"
            "}\n"
        )
        self.assert_rustc(
            rust_source,
            succeeds=True,
            externs={"thiserror": proc_macro},
        )
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as std;\n"
            "#[std::marker]\n"
            "pub struct Subject;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|marker|binding|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_relative_std_alias_in_function_like_proc_macro_path(self) -> None:
        """A qualified function-like macro retains the same local-root provenance policy."""

        proc_macro = self.build_test_proc_macro()
        helper_source = self.install_proc_macro_alias_module()
        rust_source = (
            helper_source
            + "mod connection {\n"
            "    use crate::evil as std;\n"
            "    pub fn run() { std::assert!(); }\n"
            "}\n"
        )
        self.assert_rustc(
            rust_source,
            succeeds=True,
            externs={"thiserror": proc_macro},
        )
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as std;\n"
            "pub fn run() { std::assert!(); }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|assert|binding|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_relative_std_alias_inside_nested_inline_child(self) -> None:
        """An inline child resolves its own std use before the external prelude."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "mod connection {\n"
            "    mod child {\n"
            "        use crate::evil as std;\n"
            "        pub type Leaked = std::String;\n"
            "    }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "mod child {\n"
            "    use crate::evil as std;\n"
            "    pub type Leaked = std::String;\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|transitional|binding"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_relative_std_alias_inside_external_child(self) -> None:
        """An external child resolves its own std use before the external prelude."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "mod connection {\n"
            "    mod child {\n"
            "        use crate::evil as std;\n"
            "        pub type Leaked = std::String;\n"
            "    }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "mod child;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        child = self.root / "connection/contract/child.rs"
        child.parent.mkdir(parents=True, exist_ok=True)
        child.write_text(
            "use crate::evil as std;\n"
            "pub type Leaked = std::String;\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|transitional|binding"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_block_std_alias_in_ordinary_type_path(self) -> None:
        """A block use binding shadows std throughout its lexical block."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "pub fn leak() {\n"
            "    use crate::evil as std;\n"
            "    let _: std::String;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn leak() {\n"
            "    use crate::evil as std;\n"
            "    let _: std::String;\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|lexical|transitional|binding"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_block_std_alias_before_use_declaration(self) -> None:
        """A block use item binds for the whole block, including preceding expressions."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "pub fn leak() {\n"
            "    let _: std::String;\n"
            "    use crate::evil as std;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn leak() {\n"
            "    let _: std::String;\n"
            "    use crate::evil as std;\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|lexical|transitional|binding"):
            verify_tree(self.root, phase="transitional")

    def test_accepts_std_after_block_alias_scope_ends(self) -> None:
        """A block-local std alias does not poison a later external-prelude path."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "pub fn clean() {\n"
            "    { use crate::evil as std; }\n"
            "    let _: std::string::String;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub fn clean() {\n"
            "    { use crate::evil as std; }\n"
            "    let _: std::string::String;\n"
            "}\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_parent_std_alias_not_inherited_by_inline_child(self) -> None:
        """An ordinary parent use binding is not in an inline child's name scope."""

        helper_source = self.install_private_macro_alias_module()
        rust_source = (
            helper_source
            + "mod connection {\n"
            "    use crate::evil as std;\n"
            "    mod child { pub fn run() { std::assert!(true); } }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as std;\n"
            "mod child { pub fn run() { std::assert!(true); } }\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_accepts_parent_std_alias_not_inherited_by_external_child(self) -> None:
        """An ordinary parent use binding is not in an external child's name scope."""

        helper_source = self.install_private_macro_alias_module()
        rust_source = (
            helper_source
            + "mod connection {\n"
            "    use crate::evil as std;\n"
            "    mod child { pub fn run() { std::assert!(true); } }\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as std;\n"
            "mod child;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        child = self.root / "connection/contract/child.rs"
        child.parent.mkdir(parents=True, exist_ok=True)
        child.write_text(
            "pub fn run() { std::assert!(true); }\n",
            encoding="utf-8",
        )
        verify_tree(self.root, phase="transitional")

    def test_rejects_grouped_std_alias_in_ordinary_type_path(self) -> None:
        """Grouped use syntax cannot bypass local std binding provenance."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "mod connection {\n"
            "    use crate::{evil as std};\n"
            "    pub type Leaked = std::String;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::{evil as std};\n"
            "pub type Leaked = std::String;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|transitional|binding"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_pub_use_chain_std_alias_in_ordinary_type_path(self) -> None:
        """A pub-use chain named std retains its local target provenance."""

        helper_source = self.install_transitional_type_alias_module()
        bridge_source = (
            "mod bridge { pub(crate) use crate::evil as std; }\n"
        )
        lib = self.root / "lib.rs"
        lib.write_text(
            lib.read_text(encoding="utf-8") + bridge_source,
            encoding="utf-8",
        )
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + bridge_source
            + "mod connection {\n"
            "    use crate::bridge::std;\n"
            "    pub type Leaked = std::String;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::bridge::std;\n"
            "pub type Leaked = std::String;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|transitional|binding"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_raw_std_alias_in_ordinary_type_path(self) -> None:
        """Raw identifier spelling cannot turn a local std alias into the external prelude."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "mod connection {\n"
            "    use crate::evil as r#std;\n"
            "    pub type Leaked = r#std::String;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as r#std;\n"
            "pub type Leaked = r#std::String;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "std|transitional|binding"):
            verify_tree(self.root, phase="transitional")

    def test_rejects_extern_crate_alias_named_std(self) -> None:
        """An extern-crate alias named std cannot impersonate the absolute std provenance."""

        rust_source = (
            "#![no_std]\n"
            "extern crate self as std;\n"
            "pub struct String;\n"
            "mod connection { pub type Leaked = std::String; }\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        lib = self.root / "lib.rs"
        lib.write_text(
            "#![no_std]\n"
            "extern crate self as std;\n"
            "pub use resource::Blob as String;\n"
            + lib.read_text(encoding="utf-8"),
            encoding="utf-8",
        )
        source = self.root / "connection/contract.rs"
        source.write_text(
            "pub type Leaked = std::String;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "extern crate|alias|std|provenance"):
            verify_tree(self.root, phase="transitional")

    def test_accepts_absolute_std_type_despite_relative_alias(self) -> None:
        """Leading ::std keeps absolute external provenance despite a relative alias."""

        helper_source = self.install_transitional_type_alias_module()
        rust_source = (
            "pub mod resource { pub struct Blob; }\n"
            + helper_source
            + "mod connection {\n"
            "    use crate::evil as std;\n"
            "    pub type Clean = ::std::string::String;\n"
            "}\n"
        )
        self.assert_rustc(rust_source, succeeds=True)
        source = self.root / "connection/contract.rs"
        source.write_text(
            "use crate::evil as std;\n"
            "pub type Clean = ::std::string::String;\n"
            "pub trait Contract: Send {}\n",
            encoding="utf-8",
        )
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
