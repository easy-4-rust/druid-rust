#!/usr/bin/env python3
"""Fail-closed self-tests for the live RDBC standardization denominator."""

from __future__ import annotations

import sys
import tempfile
import unittest
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
        ]
        self.assertEqual(35, len(receiver_calls))
        names = [row.symbol.split("@", 1)[0] for row in receiver_calls]
        self.assertEqual(1, names.count("commit"))
        self.assertEqual(2, names.count("rollback"))

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
