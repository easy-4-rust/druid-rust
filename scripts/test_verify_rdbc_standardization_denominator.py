#!/usr/bin/env python3
"""Fail-closed self-tests for the live RDBC standardization denominator."""

from __future__ import annotations

import sys
import tempfile
import unittest
from collections import Counter
from dataclasses import replace
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parent
REPOSITORY_ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))

from verify_rdbc_standardization_denominator import (  # noqa: E402
    ADAPTER_HEADER,
    CONSUMER_HEADER,
    PHYSICAL_HEADER,
    VerificationError,
    ConsumerRow,
    derive_live_manifests,
    render_adapter_rows,
    render_consumer_rows,
    render_physical_rows,
    scan_source_text,
    verify_denominator,
)
import verify_rdbc_standardization_denominator as standardizer  # noqa: E402


class RdbcStandardizationDenominatorTest(unittest.TestCase):
    """Verify dynamic source enumeration and fail-closed manifest comparison."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.live = derive_live_manifests(
            REPOSITORY_ROOT,
            REPOSITORY_ROOT / "docs/verification/rdbc-first/rdbc-physical-standardization.tsv",
        )

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.physical = self.root / "physical.tsv"
        self.adapters = self.root / "adapters.tsv"
        self.consumers = self.root / "consumers.tsv"
        self.physical.write_bytes(render_physical_rows(self.live.physical))
        self.adapters.write_bytes(render_adapter_rows(self.live.adapters))
        self.consumers.write_bytes(render_consumer_rows(self.live.consumers))

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def verify(self) -> None:
        """Run the real live comparison against the temporary manifests."""

        verify_denominator(
            REPOSITORY_ROOT,
            self.physical,
            self.adapters,
            self.consumers,
            live=self.live,
        )

    @staticmethod
    def mutate_lines(path: Path, mutation) -> None:
        """Apply one controlled line-level mutation."""

        lines = path.read_text(encoding="utf-8").splitlines()
        path.write_text("\n".join(mutation(lines)) + "\n", encoding="utf-8")

    def test_accepts_exact_dynamic_manifests(self) -> None:
        """The positive control is the scanner's exact current production denominator."""

        self.verify()
        self.assertGreaterEqual(len(self.live.physical), 48)
        self.assertGreater(len(self.live.adapters), 0)
        self.assertGreater(len(self.live.consumers), 0)

    def test_accepts_only_task4_wrapper_live_state_projection(self) -> None:
        """Task 4 may advance its wrapper rows while every source-derived field stays exact."""

        adapters = tuple(
            replace(row, state="STANDARD_IMPL_LIVE")
            if row.target_task == "P04_TASK_4_ADAPTER_CUTOVER"
            and row.owner == "WRAPPER_ADAPTER"
            else row
            for row in self.live.adapters
        )
        consumers = tuple(
            replace(row, state="STANDARD_REFERENCE_LIVE")
            if row.target_task == "P04_TASK_4_ADAPTER_CUTOVER"
            and row.source_path.startswith("crates/druid-wrapper/")
            else row
            for row in self.live.consumers
        )
        self.adapters.write_bytes(render_adapter_rows(adapters))
        self.consumers.write_bytes(render_consumer_rows(consumers))
        self.verify()

    def test_rejects_live_state_on_wrong_target_or_unknown_state(self) -> None:
        """Task 5 rows and unknown future states cannot impersonate Task 4 LIVE evidence."""

        adapter_lines = self.adapters.read_text(encoding="utf-8").splitlines()
        wrong_adapter = next(
            index
            for index, line in enumerate(adapter_lines[1:], start=1)
            if "P04_TASK_4_ADAPTER_CUTOVER" not in line
        )
        fields = adapter_lines[wrong_adapter].split("\t")
        fields[-1] = "STANDARD_IMPL_LIVE"
        adapter_lines[wrong_adapter] = "\t".join(fields)
        self.adapters.write_text("\n".join(adapter_lines) + "\n", encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "only valid for Task 4"):
            self.verify()

        self.adapters.write_bytes(render_adapter_rows(self.live.adapters))
        consumer_lines = self.consumers.read_text(encoding="utf-8").splitlines()
        fields = consumer_lines[1].split("\t")
        fields[-1] = "UNKNOWN_LIVE"
        consumer_lines[1] = "\t".join(fields)
        self.consumers.write_text("\n".join(consumer_lines) + "\n", encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "unknown state"):
            self.verify()

    def test_rejects_missing_duplicate_extra_and_unknown_physical_rows(self) -> None:
        """No table edit may shrink, duplicate, invent, or declassify the P03/live surface."""

        for mutation in (
            lambda lines: lines[:-1],
            lambda lines: lines + [lines[1]],
            lambda lines: lines + [lines[1].replace("druid_rdbc::", "druid_rdbc::fake::", 1)],
            lambda lines: [lines[0], lines[1].replace("MERGE_STANDARD", "UNKNOWN"), *lines[2:]],
        ):
            self.physical.write_bytes(render_physical_rows(self.live.physical))
            self.mutate_lines(self.physical, mutation)
            with self.assertRaises(VerificationError):
                self.verify()

    def test_rejects_fake_source_paths_and_unresolved_old_paths(self) -> None:
        """Every physical row must resolve to the live declaration it claims."""

        lines = self.physical.read_text(encoding="utf-8").splitlines()
        fields = lines[1].split("\t")
        fields[0] = "druid_rdbc::spi::DoesNotExist"
        fields[4] = "crates/fake/src/does_not_exist.rs"
        lines[1] = "\t".join(fields)
        self.physical.write_text("\n".join(lines) + "\n", encoding="utf-8")
        with self.assertRaises(VerificationError):
            self.verify()

    def test_rejects_missing_duplicate_and_fake_adapter_rows(self) -> None:
        """All current impls need one real source-bound planned canonical target."""

        for mutation in (
            lambda lines: lines[:-1],
            lambda lines: lines + [lines[1]],
            lambda lines: [lines[0], lines[1].replace("crates/", "crates/fake/", 1), *lines[2:]],
        ):
            self.adapters.write_bytes(render_adapter_rows(self.live.adapters))
            self.mutate_lines(self.adapters, mutation)
            with self.assertRaises(VerificationError):
                self.verify()

    def test_rejects_missing_duplicate_and_extra_consumer_occurrences(self) -> None:
        """Every production occurrence is a row; neither omission nor invention is tolerated."""

        for mutation in (
            lambda lines: lines[:-1],
            lambda lines: lines + [lines[1]],
            lambda lines: lines + [lines[1].replace("crates/", "crates/fake/", 1)],
        ):
            self.consumers.write_bytes(render_consumer_rows(self.live.consumers))
            self.mutate_lines(self.consumers, mutation)
            with self.assertRaises(VerificationError):
                self.verify()

    def test_comments_strings_and_raw_strings_do_not_create_occurrences(self) -> None:
        """Rust literals and comments containing old names must not inflate the denominator."""

        mapping = {
            "druid_rdbc::spi::PhysicalConnection": "druid_rdbc::connection::Connection",
        }
        source = '''
// PhysicalConnection and druid_rdbc::spi::PhysicalConnection
const NORMAL: &str = "PhysicalConnection";
const RAW: &str = r###"druid_rdbc::spi::PhysicalConnection"###;
const BYTE_RAW: &[u8] = br#"PhysicalConnection"#;
'''
        occurrences, implementations = scan_source_text(Path("crates/druid/src/literal.rs"), source, mapping)
        self.assertEqual([], occurrences)
        self.assertEqual([], implementations)

    def test_grouped_raw_identifiers_and_aliases_cannot_escape(self) -> None:
        """Grouped imports, raw names, crate aliases, and renamed symbols remain traceable."""

        mapping = {
            "druid_rdbc::spi::PhysicalConnection": "druid_rdbc::connection::Connection",
            "druid_rdbc::spi::PhysicalStatement": "druid_rdbc::statement::Statement",
        }
        source = '''
extern crate druid_rdbc as rr;
use rr::spi::{r#PhysicalConnection as PC, PhysicalStatement as PS};
fn accepts(connection: Box<dyn PC>, statement: &PS) {
    let _ = PC::is_closed(connection.as_ref());
    let _ = statement;
}
'''
        occurrences, _ = scan_source_text(Path("crates/druid/src/aliases.rs"), source, mapping)
        kinds = [row.occurrence_kind for row in occurrences]
        self.assertEqual(2, kinds.count("IMPORT"))
        self.assertIn("TRAIT_OBJECT", kinds)
        self.assertIn("TYPE", kinds)
        self.assertIn("CALL", kinds)

    def test_live_scan_includes_module_relative_qualified_paths(self) -> None:
        """Current super-qualified UFCS and dyn paths must enter the denominator."""

        ufcs_path = "crates/druid-rdbc/src/spi/physical_prepared_statement.rs"
        ufcs = [
            row for row in self.live.consumers
            if row.source_path == ufcs_path
            and row.old_path == "druid_rdbc::spi::PhysicalStatement"
            and row.occurrence_kind == "CALL"
            and "@qualified-token=" in row.symbol
        ]
        self.assertEqual(19, len(ufcs))

        dyn_path = "crates/druid/src/core/druid_pooled_prepared_statement.rs"
        qualified = [
            row for row in self.live.consumers
            if row.source_path == dyn_path
            and row.old_path == "druid_rdbc::spi::PhysicalPreparedStatement"
            and "@qualified-token=" in row.symbol
        ]
        self.assertEqual(5, len(qualified))
        self.assertIn("TRAIT_OBJECT", {row.occurrence_kind for row in qualified})

    def test_live_scan_includes_proven_physical_mut_receiver_calls(self) -> None:
        """The typed physical_mut accessor must account for all 35 current trait calls."""

        source_path = "crates/druid/src/core/druid_pooled_connection.rs"
        receiver_calls = [
            row for row in self.live.consumers
            if row.source_path == source_path
            and row.old_path == "druid_rdbc::spi::PhysicalConnection"
            and row.occurrence_kind == "CALL"
            and "@receiver=PhysicalConnection@" in row.symbol
            and (
                "@binding=accessor:druid::core::druid_pooled_connection::"
                "DruidPooledConnection:physical_mut@"
            ) in row.symbol
        ]
        self.assertEqual(35, len(receiver_calls))
        names = [row.symbol.split("@", 1)[0] for row in receiver_calls]
        self.assertEqual(1, names.count("commit"))
        self.assertEqual(2, names.count("rollback"))

    def test_filter_chain_same_name_receivers_keep_lexical_trait_identity(self) -> None:
        """Each real filter-chain physical binding keeps its own Connection/Statement trait."""

        source_path = "crates/druid/src/core/filter_chain_impl.rs"
        rows = [
            row for row in self.live.consumers
            if row.source_path == source_path and "@receiver=" in row.symbol
        ]
        observed = {}
        for row in rows:
            method = row.symbol.split("@", 1)[0]
            observed[(row.old_path, method)] = observed.get((row.old_path, method), 0) + 1
        connection = "druid_rdbc::spi::PhysicalConnection"
        statement = "druid_rdbc::spi::PhysicalStatement"
        prepared = "druid_rdbc::spi::PhysicalPreparedStatement"
        self.assertEqual(1, observed.get((connection, "create_blob"), 0))
        self.assertEqual(1, observed.get((connection, "create_clob"), 0))
        self.assertEqual(1, observed.get((connection, "create_n_clob"), 0))
        self.assertEqual(1, observed.get((connection, "database_meta_data"), 0))
        self.assertEqual(1, observed.get((connection, "warnings"), 0))
        self.assertEqual(1, observed.get((connection, "clear_warnings"), 0))
        self.assertEqual(1, observed.get((statement, "warnings"), 0))
        self.assertEqual(1, observed.get((statement, "clear_warnings"), 0))
        self.assertEqual(1, observed.get((prepared, "warnings"), 0))
        self.assertEqual(1, observed.get((prepared, "clear_warnings"), 0))

    def test_physical_connection_lease_generic_impl_accessors_cover_all_calls(self) -> None:
        """Inherent and trait impls of PhysicalConnectionLease<L> share one normalized identity."""

        source_path = "crates/druid/src/core/physical_connection_lease.rs"
        rows = [
            row for row in self.live.consumers
            if row.source_path == source_path
            and row.old_path == "druid_rdbc::spi::PhysicalConnection"
            and row.occurrence_kind == "CALL"
            and (
                "@binding=accessor:druid::core::physical_connection_lease::"
                "PhysicalConnectionLease<_G0>:physical_connection"
            ) in row.symbol
        ]
        self.assertEqual(52, len(rows))

    def test_receiver_expression_distinguishes_self_field_from_parameter(self) -> None:
        """self.physical resolves the field; bare physical resolves the parameter."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        statement = "druid_rdbc::spi::PhysicalStatement"
        mapping = {
            connection: "druid_rdbc::connection::Connection",
            statement: "druid_rdbc::statement::Statement",
        }
        source = '''
use druid_rdbc::spi::{PhysicalConnection, PhysicalStatement};
struct Holder<'a> { physical: &'a dyn PhysicalConnection }
impl<'a> Holder<'a> {
    fn run(&self, physical: &dyn PhysicalStatement) {
        self.physical.commit();
        physical.warnings();
    }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/field_vs_parameter.rs"), source, mapping,
            trait_paths={connection, statement},
            trait_methods={connection: {"commit"}, statement: {"warnings"}},
        )
        calls = sorted(
            (row.old_path, row.symbol.split("@", 1)[0])
            for row in occurrences if "@receiver=" in row.symbol
        )
        self.assertEqual(sorted([(connection, "commit"), (statement, "warnings")]), calls)

    def test_destructuring_closure_and_control_flow_patterns_are_scoped(self) -> None:
        """Destructured params, closure params, if/while-let and match arms retain identities."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        statement = "druid_rdbc::spi::PhysicalStatement"
        mapping = {
            connection: "druid_rdbc::connection::Connection",
            statement: "druid_rdbc::statement::Statement",
        }
        source = '''
use druid_rdbc::spi::{PhysicalConnection, PhysicalStatement};
enum Target<'a> {
    Connection(&'a dyn PhysicalConnection),
    Statement(&'a dyn PhysicalStatement),
}
struct Unrelated;
enum UnrelatedTarget { Other(Unrelated) }
fn destructured((connection, statement): (&dyn PhysicalConnection, &dyn PhysicalStatement)) {
    connection.commit();
    statement.warnings();
    let closure = |physical: &dyn PhysicalConnection| physical.rollback();
    if let Target::Connection(physical) = next_target() { physical.commit(); }
    while let Target::Statement(physical) = next_target() { physical.clear_warnings(); }
    match next_target() {
        Target::Connection(physical) => physical.rollback(),
        Target::Statement(physical) => physical.warnings(),
    }
}
fn closure_shadow(physical: &dyn PhysicalConnection) {
    let closure = |physical| physical.commit();
    physical.rollback();
}
fn control_shadow(physical: &dyn PhysicalConnection) {
    if let UnrelatedTarget::Other(physical) = unrelated_target() { physical.commit(); }
    physical.rollback();
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/pattern_scopes.rs"), source, mapping,
            trait_paths={connection, statement},
            trait_methods={
                connection: {"commit", "rollback"},
                statement: {"warnings", "clear_warnings"},
            },
        )
        observed = Counter(
            (row.old_path, row.symbol.split("@", 1)[0])
            for row in occurrences if "@receiver=" in row.symbol
        )
        self.assertEqual(2, observed[(connection, "commit")])
        self.assertEqual(4, observed[(connection, "rollback")])
        self.assertEqual(2, observed[(statement, "warnings")])
        self.assertEqual(1, observed[(statement, "clear_warnings")])

    def test_two_generic_impls_keep_same_named_accessor_owners_distinct(self) -> None:
        """Generic declarations do not collapse Holder<T> and Other<T> to impl owner T."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        statement = "druid_rdbc::spi::PhysicalStatement"
        mapping = {
            connection: "druid_rdbc::connection::Connection",
            statement: "druid_rdbc::statement::Statement",
        }
        source = '''
use druid_rdbc::spi::{PhysicalConnection, PhysicalStatement};
struct Holder<T>(T);
struct Other<T>(T);
impl<T> self::Holder<T> {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
}
impl<T> Holder<T> {
    fn run(&self) { self.physical().commit(); }
}
impl<T> self::Other<T> {
    fn physical(&self) -> &dyn PhysicalStatement { todo!() }
}
impl<T> Other<T> {
    fn run(&self) { self.physical().warnings(); }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/generic_impl_accessors.rs"), source, mapping,
            trait_paths={connection, statement},
            trait_methods={connection: {"commit"}, statement: {"warnings"}},
        )
        calls = sorted(
            (row.old_path, row.symbol.split("@", 1)[0])
            for row in occurrences if "@receiver=" in row.symbol
        )
        self.assertEqual(sorted([(connection, "commit"), (statement, "warnings")]), calls)

    def test_qualified_same_base_impls_keep_distinct_accessor_owners(self) -> None:
        """a::Holder<T> and b::Holder<T> cannot collapse to one Holder<T> identity."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        statement = "druid_rdbc::spi::PhysicalStatement"
        mapping = {
            connection: "druid_rdbc::connection::Connection",
            statement: "druid_rdbc::statement::Statement",
        }
        source = '''
use druid_rdbc::spi::{PhysicalConnection, PhysicalStatement};
impl<T> a::Holder<T> {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
    fn run(&self) { self.physical().commit(); }
}
impl<T> b::Holder<T> {
    fn physical(&self) -> &dyn PhysicalStatement { todo!() }
    fn run(&self) { self.physical().warnings(); }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/qualified_same_base.rs"), source, mapping,
            trait_paths={connection, statement},
            trait_methods={connection: {"commit"}, statement: {"warnings"}},
        )
        calls = sorted(
            (row.old_path, row.symbol.split("@", 1)[0])
            for row in occurrences if "@receiver=" in row.symbol
        )
        self.assertEqual(sorted([(connection, "commit"), (statement, "warnings")]), calls)

    def test_qualified_associated_self_impls_keep_distinct_accessor_owners(self) -> None:
        """Qualified <T as Assoc>::Out self types retain their complete identity."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        statement = "druid_rdbc::spi::PhysicalStatement"
        mapping = {
            connection: "druid_rdbc::connection::Connection",
            statement: "druid_rdbc::statement::Statement",
        }
        source = '''
use druid_rdbc::spi::{PhysicalConnection, PhysicalStatement};
impl<T> Runner for <T as a::Assoc>::Out {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
    fn run(&self) { self.physical().commit(); }
}
impl<T> Runner for <T as b::Assoc>::Out {
    fn physical(&self) -> &dyn PhysicalStatement { todo!() }
    fn run(&self) { self.physical().warnings(); }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/qualified_associated_self.rs"), source, mapping,
            trait_paths={connection, statement},
            trait_methods={connection: {"commit"}, statement: {"warnings"}},
        )
        calls = sorted(
            (row.old_path, row.symbol.split("@", 1)[0])
            for row in occurrences if "@receiver=" in row.symbol
        )
        self.assertEqual(sorted([(connection, "commit"), (statement, "warnings")]), calls)

    def test_impl_self_paths_expand_from_source_module_context(self) -> None:
        """crate/self/super impl owners share their source-relative canonical identity."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        statement = "druid_rdbc::spi::PhysicalStatement"
        mapping = {
            connection: "druid_rdbc::connection::Connection",
            statement: "druid_rdbc::statement::Statement",
        }
        source = '''
use druid_rdbc::spi::{PhysicalConnection, PhysicalStatement};
impl<T> super::Holder<T> {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
}
impl<T> crate::Holder<T> {
    fn run(&self) { self.physical().commit(); }
}
impl<T> self::LocalHolder<T> {
    fn physical(&self) -> &dyn PhysicalStatement { todo!() }
}
impl<T> crate::child::LocalHolder<T> {
    fn run(&self) { self.physical().warnings(); }
}
impl<T> Accessor for <T as super::Assoc>::Out {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
}
impl<T> Runner for <T as crate::Assoc>::Out {
    fn run(&self) { self.physical().rollback(); }
}
impl<T> a::Other<T> {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
}
impl<T> b::Other<T> {
    fn run(&self) { self.physical().commit(); }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/child/mod.rs"), source, mapping,
            trait_paths={connection, statement},
            trait_methods={
                connection: {"commit", "rollback"},
                statement: {"warnings"},
            },
        )
        calls = sorted(
            (row.old_path, row.symbol.split("@", 1)[0])
            for row in occurrences if "@receiver=" in row.symbol
        )
        self.assertEqual(
            sorted([
                (connection, "commit"),
                (connection, "rollback"),
                (statement, "warnings"),
            ]),
            calls,
        )

        repeated_source = '''
use druid_rdbc::spi::{PhysicalConnection, PhysicalStatement};
impl<T> super::super::RepeatedHolder<T> {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
}
impl<T> crate::RepeatedHolder<T> {
    fn run(&self) { self.physical().commit(); }
}
impl<T> Accessor for <T as super::super::RepeatedAssoc>::Out {
    fn physical(&self) -> &dyn PhysicalStatement { todo!() }
}
impl<T> Runner for <T as crate::RepeatedAssoc>::Out {
    fn run(&self) { self.physical().warnings(); }
}
'''
        repeated_occurrences, _ = scan_source_text(
            Path("crates/druid/src/child/grandchild/mod.rs"),
            repeated_source,
            mapping,
            trait_paths={connection, statement},
            trait_methods={connection: {"commit"}, statement: {"warnings"}},
        )
        repeated_calls = sorted(
            (row.old_path, row.symbol.split("@", 1)[0])
            for row in repeated_occurrences if "@receiver=" in row.symbol
        )
        self.assertEqual(
            sorted([(connection, "commit"), (statement, "warnings")]),
            repeated_calls,
        )

    def test_child_local_self_type_does_not_merge_with_crate_root_type(self) -> None:
        """A child-local self::Holder cannot lend accessors to crate::Holder."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        mapping = {connection: "druid_rdbc::connection::Connection"}
        source = '''
use druid_rdbc::spi::PhysicalConnection;
impl<T> self::Holder<T> {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
}
impl<T> crate::Holder<T> {
    fn run(&self) { self.physical().commit(); }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/child/mod.rs"),
            source,
            mapping,
            trait_paths={connection},
            trait_methods={connection: {"commit"}},
        )
        receiver_calls = [
            row for row in occurrences if "@receiver=" in row.symbol
        ]
        self.assertEqual([], receiver_calls)

    def test_unresolved_external_type_does_not_merge_with_current_crate_type(self) -> None:
        """An unproven external::Holder cannot lend accessors to crate::external::Holder."""

        statement = "druid_rdbc::spi::PhysicalStatement"
        mapping = {statement: "druid_rdbc::statement::Statement"}
        source = '''
use druid_rdbc::spi::PhysicalStatement;
impl<T> external::Holder<T> {
    fn physical(&self) -> &dyn PhysicalStatement { todo!() }
}
impl<T> crate::external::Holder<T> {
    fn run(&self) { self.physical().warnings(); }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/consumer.rs"),
            source,
            mapping,
            trait_paths={statement},
            trait_methods={statement: {"warnings"}},
        )
        receiver_calls = [
            row for row in occurrences if "@receiver=" in row.symbol
        ]
        self.assertEqual([], receiver_calls)

    def test_source_proven_crate_import_keeps_absolute_self_type_identity(self) -> None:
        """An explicit crate import aliases the same absolute self type."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        mapping = {connection: "druid_rdbc::connection::Connection"}
        source = '''
use druid_rdbc::spi::PhysicalConnection;
use crate::Holder;
impl<T> Holder<T> {
    fn physical(&self) -> &dyn PhysicalConnection { todo!() }
}
impl<T> crate::Holder<T> {
    fn run(&self) { self.physical().commit(); }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/child/mod.rs"),
            source,
            mapping,
            trait_paths={connection},
            trait_methods={connection: {"commit"}},
        )
        receiver_calls = [
            row for row in occurrences if "@receiver=" in row.symbol
        ]
        self.assertEqual(1, len(receiver_calls))
        self.assertIn("accessor:druid::Holder<_G0>:physical", receiver_calls[0].symbol)

    def test_non_block_closure_scope_ignores_nested_call_comma(self) -> None:
        """A nested call comma cannot end an untyped closure's shadow scope early."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        mapping = {connection: "druid_rdbc::connection::Connection"}
        source = '''
use druid_rdbc::spi::PhysicalConnection;
fn run(physical: &dyn PhysicalConnection) {
    let closure = |physical| dispatch(unrelated(), physical.commit());
    physical.rollback();
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/non_block_closure.rs"), source, mapping,
            trait_paths={connection},
            trait_methods={connection: {"commit", "rollback"}},
        )
        methods = [
            row.symbol.split("@", 1)[0]
            for row in occurrences if "@receiver=" in row.symbol
        ]
        self.assertEqual(["rollback"], methods)

    def test_non_block_closure_scope_ignores_turbofish_and_nested_delimiters(self) -> None:
        """Turbofish, tuple, and macro commas remain inside an untyped closure shadow."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        mapping = {connection: "druid_rdbc::connection::Connection"}
        source = '''
use druid_rdbc::spi::PhysicalConnection;
fn run(physical: &dyn PhysicalConnection) {
    let closure = |physical| dispatch::<Left, Right>(
        (physical.commit(), nested!(physical.commit(), (left, right))),
    );
    physical.rollback();
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/non_block_closure_turbofish.rs"), source, mapping,
            trait_paths={connection},
            trait_methods={connection: {"commit", "rollback"}},
        )
        methods = [
            row.symbol.split("@", 1)[0]
            for row in occurrences if "@receiver=" in row.symbol
        ]
        self.assertEqual(["rollback"], methods)

    def test_match_pattern_binding_is_visible_in_guard_and_body(self) -> None:
        """A match payload binding starts at the guard and remains through the arm body."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        mapping = {connection: "druid_rdbc::connection::Connection"}
        source = '''
use druid_rdbc::spi::PhysicalConnection;
enum Target<'a> { Connection(&'a dyn PhysicalConnection) }
fn run(target: Target<'_>) {
    match target {
        Target::Connection(physical) if physical.commit() => physical.rollback(),
    }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/match_guard.rs"), source, mapping,
            trait_paths={connection},
            trait_methods={connection: {"commit", "rollback"}},
        )
        methods = sorted(
            row.symbol.split("@", 1)[0]
            for row in occurrences if "@receiver=" in row.symbol
        )
        self.assertEqual(["commit", "rollback"], methods)

    def test_unrelated_same_leaf_import_is_not_bound_to_rdbc(self) -> None:
        """An unrelated module exporting the same leaf name is not RDBC evidence."""

        mapping = {
            "druid_rdbc::spi::PhysicalConnection": "druid_rdbc::connection::Connection",
        }
        source = "use unrelated::PhysicalConnection; fn inspect(_: PhysicalConnection) {}"
        occurrences, implementations = scan_source_text(
            Path("crates/druid/src/unrelated.rs"), source, mapping
        )
        self.assertEqual([], occurrences)
        self.assertEqual([], implementations)

    def test_local_same_leaf_shadow_is_not_bound_to_rdbc(self) -> None:
        """A local declaration with an old leaf name cannot impersonate the real source item."""

        mapping = {
            "druid_rdbc::spi::PhysicalConnection": "druid_rdbc::connection::Connection",
        }
        source = "struct PhysicalConnection; impl PhysicalConnection { fn close(&self) {} }"
        occurrences, implementations = scan_source_text(
            Path("crates/druid/src/local_shadow.rs"), source, mapping
        )
        self.assertEqual([], occurrences)
        self.assertEqual([], implementations)

    def test_cfg_test_receiver_calls_are_not_production_occurrences(self) -> None:
        """Receiver scanning excludes cfg(test) bodies while retaining production calls."""

        old_path = "druid_rdbc::spi::PhysicalConnection"
        mapping = {old_path: "druid_rdbc::connection::Connection"}
        source = '''
use druid_rdbc::spi::PhysicalConnection;
fn physical_mut(&mut self) -> Result<&mut dyn PhysicalConnection, Error> { unreachable!() }
fn production(&mut self) { self.physical_mut()?.commit(); }
#[cfg(test)]
mod tests {
    use super::*;
    fn test_only(&mut self) { self.physical_mut()?.rollback(); }
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/cfg_fixture.rs"),
            source,
            mapping,
            trait_paths={old_path},
            trait_methods={old_path: {"commit", "rollback"}},
        )
        receiver_methods = [
            row.symbol.split("@", 1)[0]
            for row in occurrences
            if "@receiver=PhysicalConnection@" in row.symbol
        ]
        self.assertEqual(["commit"], receiver_methods)

    def test_receiver_bindings_follow_method_and_nested_shadow_scopes(self) -> None:
        """Same-name parameters and a nested unrelated local cannot overwrite each other."""

        connection = "druid_rdbc::spi::PhysicalConnection"
        statement = "druid_rdbc::spi::PhysicalStatement"
        mapping = {
            connection: "druid_rdbc::connection::Connection",
            statement: "druid_rdbc::statement::Statement",
        }
        source = '''
use druid_rdbc::spi::{PhysicalConnection, PhysicalStatement};
struct Other;
fn connection_path(physical: &dyn PhysicalConnection) {
    physical.commit();
    {
        let physical: &Other = unrelated();
        physical.commit();
    }
    physical.rollback();
}
fn statement_path(physical: &dyn PhysicalStatement) {
    physical.warnings();
}
'''
        occurrences, _ = scan_source_text(
            Path("crates/druid/src/scoped_receivers.rs"),
            source,
            mapping,
            trait_paths={connection, statement},
            trait_methods={
                connection: {"commit", "rollback"},
                statement: {"warnings"},
            },
        )
        calls = [
            (row.old_path, row.symbol.split("@", 1)[0])
            for row in occurrences
            if "@receiver=" in row.symbol
        ]
        self.assertEqual(
            sorted(
                [
                    (connection, "commit"),
                    (connection, "rollback"),
                    (statement, "warnings"),
                ]
            ),
            sorted(calls),
        )

    def test_cfg_predicates_share_exact_all_any_production_model(self) -> None:
        """test and all(test, ...) are excluded; any(test, feature) remains production-capable."""

        source = '''
#[cfg(test)] pub struct ExactOnly;
#[cfg(all(test, feature = "x"))] pub struct AllOnly;
#[cfg(any(test, feature = "x"))] pub struct AnyVisible;
'''
        tokens = standardizer.rust_tokens(source)
        excluded = standardizer.production_excluded_indexes(tokens)
        positions = {
            token.text: index
            for index, token in enumerate(tokens)
            if token.kind == "IDENT" and token.text in {"ExactOnly", "AllOnly", "AnyVisible"}
        }
        self.assertIn(positions["ExactOnly"], excluded)
        self.assertIn(positions["AllOnly"], excluded)
        self.assertNotIn(positions["AnyVisible"], excluded)

    def test_cfg_test_false_satisfiability_handles_not_cfg_attr_and_conjunctions(self) -> None:
        """Double-not, cfg_attr and multiple cfg constraints use test=false satisfiability."""

        source = '''
#[cfg(not(test))] fn NotTestVisible() {}
#[cfg(not(not(test)))] fn DoubleNotOnly() {}
#[cfg_attr(not(test), cfg(test))] fn CfgAttrOnly() {}
#[cfg_attr(test, cfg(feature = "x"))] fn CfgAttrVisible() {}
#[cfg(any(test, feature = "x"))]
#[cfg(not(feature = "x"))]
fn ConjunctionOnly() {}
'''
        tokens = standardizer.rust_tokens(source)
        excluded = standardizer.production_excluded_indexes(tokens)
        positions = {
            token.text: index for index, token in enumerate(tokens)
            if token.kind == "IDENT" and token.text.endswith(("Visible", "Only"))
        }
        self.assertNotIn(positions["NotTestVisible"], excluded)
        self.assertIn(positions["DoubleNotOnly"], excluded)
        self.assertIn(positions["CfgAttrOnly"], excluded)
        self.assertNotIn(positions["CfgAttrVisible"], excluded)
        self.assertIn(positions["ConjunctionOnly"], excluded)

    def test_nested_cfg_attr_is_recursively_converted_to_effective_constraint(self) -> None:
        """Nested cfg_attr implications exclude test-only nodes without consuming a live sibling."""

        source = '''
#[cfg_attr(all(), cfg_attr(not(test), cfg(test)))] fn NestedOnly() {}
fn LiveSibling() {}
'''
        tokens = standardizer.rust_tokens(source)
        excluded = standardizer.production_excluded_indexes(tokens)
        positions = {
            token.text: index for index, token in enumerate(tokens)
            if token.kind == "IDENT" and token.text in {"NestedOnly", "LiveSibling"}
        }
        self.assertIn(positions["NestedOnly"], excluded)
        self.assertNotIn(positions["LiveSibling"], excluded)

    def test_cfg_node_boundaries_distinguish_comparisons_from_generic_arguments(self) -> None:
        """Const, macro and statement comparisons end at their own node boundaries."""

        source = '''
#[cfg(test)] const CHECK: bool = 1 < 2;
fn live_after_const() {}
fn run() {
    #[cfg(test)] check!(1 < 2);
    live_after_macro();
    #[cfg(test)] left < right;
    live_after_statement();
}
'''
        tokens = standardizer.rust_tokens(source)
        excluded = standardizer.production_excluded_indexes(tokens)
        positions = {
            token.text: index for index, token in enumerate(tokens)
            if token.kind == "IDENT" and token.text in {
                "CHECK", "live_after_const", "check", "live_after_macro",
                "left", "right", "live_after_statement",
            }
        }
        for name in ("CHECK", "check", "left", "right"):
            self.assertIn(positions[name], excluded)
        for name in ("live_after_const", "live_after_macro", "live_after_statement"):
            self.assertNotIn(positions[name], excluded)

    def test_cfg_attribute_binds_only_comma_terminated_field_or_variant_node(self) -> None:
        """A test-only field/variant cannot consume its live sibling or following function."""

        source = '''
#[derive(Clone)]
struct Fields {
    #[cfg(test)] hidden: u8,
    live: u8,
}
#[derive(Clone)]
enum Variants {
    #[cfg(all(test, feature = "x"))] Hidden(u8),
    Live(u8),
}
fn following() {}
'''
        tokens = standardizer.rust_tokens(source)
        excluded = standardizer.production_excluded_indexes(tokens)
        positions = {
            token.text: index for index, token in enumerate(tokens)
            if token.kind == "IDENT" and token.text in {"hidden", "live", "Hidden", "Live", "following"}
        }
        self.assertIn(positions["hidden"], excluded)
        self.assertNotIn(positions["live"], excluded)
        self.assertIn(positions["Hidden"], excluded)
        self.assertNotIn(positions["Live"], excluded)
        self.assertNotIn(positions["following"], excluded)

    def test_public_item_and_trait_method_derivation_share_cfg_model(self) -> None:
        """Test-only public types/methods are absent while cfg(any(...)) stays visible."""

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            spi = root / "crates/druid-rdbc/src/spi"
            resource = root / "crates/druid-rdbc/src/resource"
            spi.mkdir(parents=True)
            resource.mkdir(parents=True)
            source_path = spi / "fixture.rs"
            source_path.write_text(
                '''
#[cfg(test)] pub struct TestOnly;
#[cfg(all(test, feature = "x"))] pub struct AllOnly;
#[cfg(any(test, feature = "x"))] pub struct AnyVisible;
pub trait VisibleTrait {
    fn live(&self);
    #[cfg(test)] fn hidden_exact(&self);
    #[cfg(all(test, feature = "x"))] fn hidden_all(&self);
    #[cfg(any(test, feature = "x"))] fn visible_any(&self);
}
''',
                encoding="utf-8",
            )
            public = standardizer.scan_public_items(root)
            self.assertNotIn("druid_rdbc::spi::TestOnly", public)
            self.assertNotIn("druid_rdbc::spi::AllOnly", public)
            self.assertIn("druid_rdbc::spi::AnyVisible", public)
            trait_path = "druid_rdbc::spi::VisibleTrait"
            row = standardizer.PhysicalRow(
                trait_path,
                "MERGE_STANDARD",
                "druid_rdbc::visible::VisibleTrait",
                "PLANNED",
                "crates/druid-rdbc/src/spi/fixture.rs",
                "trait VisibleTrait",
                "P04_TASK_3_STANDARD_TRAITS",
            )
            methods = standardizer.derive_trait_methods(root, (row,))[trait_path]
            self.assertEqual({"live", "visible_any"}, methods)

    def test_wildcard_imports_fail_closed_instead_of_hiding_symbols(self) -> None:
        """A wildcard over a standardized module is ambiguous and must be rejected."""

        mapping = {
            "druid_rdbc::spi::PhysicalConnection": "druid_rdbc::connection::Connection",
        }
        with self.assertRaises(VerificationError):
            scan_source_text(
                Path("crates/druid/src/wildcard.rs"),
                "use druid_rdbc::spi::*;",
                mapping,
            )

    def test_headers_are_exact_contracts(self) -> None:
        """Manifest schemas must not drift through permissive parsing."""

        self.assertEqual(
            "old_path\tclassification\ttarget_path\tcompatibility\tsource_file\tnormalized_signature\ttarget_task",
            PHYSICAL_HEADER,
        )
        self.assertTrue(ADAPTER_HEADER.startswith("source_path\t"))
        self.assertEqual(
            "source_path\toccurrence_kind\tsymbol\told_path\ttarget_path\ttarget_task\tstate",
            CONSUMER_HEADER,
        )

    def test_tsv_renderer_rejects_delimiters_and_controls_in_source_path(self) -> None:
        """A source path cannot inject a second TSV row or column."""

        row = self.live.consumers[0]
        for invalid in (
            "crates/bad\tpath.rs",
            "crates/bad\npath.rs",
            "crates/bad\x01path.rs",
            "crates/bad\x7fpath.rs",
            "crates/bad\u0085path.rs",
        ):
            with self.assertRaises(VerificationError):
                render_consumer_rows((replace(row, source_path=invalid),))

    def test_independent_tsv_parser_rejects_unsorted_noncanonical_bytes(self) -> None:
        """The checked manifest parser enforces order independently of live byte comparison."""

        rows = self.live.consumers[:2]
        payload = render_consumer_rows(tuple(reversed(rows)))
        with self.assertRaises(VerificationError):
            standardizer.parse_consumer_payload(payload)


if __name__ == "__main__":
    unittest.main(verbosity=2)
