#!/usr/bin/env python3
"""Verify that wrapper RDBC contracts use their canonical dependency boundary."""

from __future__ import annotations

import argparse
import sys
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import VerificationError as RustLexError
from verify_rdbc_public_paths import tokenize


MANIFEST_HEADER = "path\tsymbol_family\tclassification\texpected_source"
PUBLIC_PATH_MAP = Path("docs/verification/rdbc-first/rdbc-public-path-map.tsv")
WRAPPER_SOURCE_ROOT = Path("crates/druid-wrapper/src")
CLASSIFICATIONS = {"RDBC_DIRECT", "DRUID_PRODUCT"}
SOURCES = {"druid", "druid_rdbc"}


class VerificationError(RuntimeError):
    """Raised when the wrapper dependency boundary is inconsistent."""


@dataclass(frozen=True)
class BoundaryRow:
    """One normalized production-file and symbol-family inventory row."""

    path: str
    symbol_family: str
    classification: str
    expected_source: str


@dataclass(frozen=True)
class SymbolUse:
    """One classified use of a Druid or RDBC public path."""

    family: str
    classification: str
    source: str


def repository_root() -> Path:
    """Return the repository root regardless of the caller's current directory."""

    return Path(__file__).resolve().parent.parent


def fail(message: str) -> NoReturn:
    """Raise a boundary verification error with a stable diagnostic."""

    raise VerificationError(message)


def load_public_paths(root: Path) -> tuple[dict[tuple[str, ...], tuple[str, ...]], set[tuple[str, ...]]]:
    """Load the frozen old-to-canonical public paths used to recognize moved names."""

    mapping_file = root / PUBLIC_PATH_MAP
    if not mapping_file.is_file():
        fail(f"public-path map not found: {PUBLIC_PATH_MAP}")
    lines = mapping_file.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != (
        "old_path\tnew_path\tcompile_mode\told_module_file\tsource_mode"
    ):
        fail(f"invalid public-path map header: {PUBLIC_PATH_MAP}")

    old_to_new: dict[tuple[str, ...], tuple[str, ...]] = {}
    canonical_paths: set[tuple[str, ...]] = set()
    for line_number, line in enumerate(lines[1:], start=2):
        fields = line.split("\t")
        if len(fields) != 5:
            fail(f"public-path map row {line_number} must have five columns")
        old_path = tuple(fields[0].split("::"))
        new_path = tuple(fields[1].split("::"))
        if len(old_path) < 3 or old_path[0] != "druid":
            fail(f"public-path map row {line_number} has invalid old path")
        if len(new_path) < 3 or new_path[0] != "druid_rdbc":
            fail(f"public-path map row {line_number} has invalid new path")
        if old_path in old_to_new:
            fail(f"public-path map row {line_number} duplicates {fields[0]}")
        old_to_new[old_path] = new_path
        canonical_paths.add(new_path)
    if not old_to_new:
        fail("public-path map must not be empty")
    return old_to_new, canonical_paths


def parse_use_tree(tokens: list[object], index: int, prefix: tuple[str, ...] = ()) -> tuple[list[tuple[str, ...]], int]:
    """Expand one Rust use tree into named paths, retaining wildcard markers."""

    if index < len(tokens) and tokens[index].text == "::":
        index += 1
    if index >= len(tokens):
        fail("unterminated use tree")

    if tokens[index].text == "{":
        index += 1
        paths: list[tuple[str, ...]] = []
        while index < len(tokens) and tokens[index].text != "}":
            nested, index = parse_use_tree(tokens, index, prefix)
            paths.extend(nested)
            if index < len(tokens) and tokens[index].text == ",":
                index += 1
            elif index >= len(tokens) or tokens[index].text != "}":
                fail("invalid grouped use tree")
        if index >= len(tokens):
            fail("unterminated grouped use tree")
        return paths, index + 1

    if tokens[index].text == "*":
        return [prefix + ("*",)], index + 1
    if tokens[index].kind != "IDENT":
        fail("use tree requires a named path")

    segment = tokens[index].text
    path = prefix if segment == "self" and prefix else prefix + (segment,)
    index += 1
    while index < len(tokens) and tokens[index].text == "::":
        index += 1
        if index >= len(tokens):
            fail("unterminated use path")
        if tokens[index].text == "{":
            return parse_use_tree(tokens, index, path)
        if tokens[index].text == "*":
            return [path + ("*",)], index + 1
        if tokens[index].kind != "IDENT":
            fail("use path requires an identifier after ::")
        path += (tokens[index].text,)
        index += 1

    if index < len(tokens) and tokens[index].text == "as":
        if index + 1 >= len(tokens) or tokens[index + 1].kind != "IDENT":
            fail("use alias requires an identifier")
        index += 2
    return [path], index


def target_use_paths(tokens: list[object]) -> tuple[list[tuple[str, ...]], set[int]]:
    """Expand all druid/druid_rdbc use items and return their token ranges."""

    paths: list[tuple[str, ...]] = []
    covered: set[int] = set()
    index = 0
    while index < len(tokens):
        if tokens[index].kind != "IDENT" or tokens[index].text != "use":
            index += 1
            continue
        start = index
        probe = index + 1
        if probe < len(tokens) and tokens[probe].text == "::":
            probe += 1
        is_target = (
            probe < len(tokens)
            and tokens[probe].kind == "IDENT"
            and tokens[probe].text in SOURCES
        )
        if not is_target:
            while index < len(tokens) and tokens[index].text != ";":
                index += 1
            index += 1
            covered.update(range(start, index))
            continue
        expanded, index = parse_use_tree(tokens, index + 1)
        if index >= len(tokens) or tokens[index].text != ";":
            fail("target use item must end with a semicolon")
        index += 1
        covered.update(range(start, index))
        paths.extend(expanded)
    return paths, covered


def fully_qualified_paths(tokens: list[object], covered: set[int]) -> list[tuple[str, ...]]:
    """Find druid/druid_rdbc paths outside use items, including method-body paths."""

    paths: list[tuple[str, ...]] = []
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if (
            index in covered
            or token.kind != "IDENT"
            or token.text not in SOURCES
            or index + 1 >= len(tokens)
            or tokens[index + 1].text != "::"
        ):
            index += 1
            continue
        path = (token.text,)
        cursor = index + 1
        while (
            cursor + 1 < len(tokens)
            and cursor not in covered
            and tokens[cursor].text == "::"
            and tokens[cursor + 1].kind == "IDENT"
        ):
            path += (tokens[cursor + 1].text,)
            cursor += 2
        paths.append(path)
        index = cursor
    return paths


def longest_prefix(path: tuple[str, ...], candidates: set[tuple[str, ...]]) -> tuple[str, ...] | None:
    """Return the longest candidate that prefixes a possibly variant-qualified path."""

    matches = [candidate for candidate in candidates if path[: len(candidate)] == candidate]
    return max(matches, key=len) if matches else None


def rdbc_family(canonical_path: tuple[str, ...]) -> str:
    """Map one canonical RDBC path to its reviewed symbol family."""

    domain = canonical_path[1]
    if domain == "types":
        return "RDBC_TYPES"
    if domain == "spi":
        return "RDBC_SPI"
    if domain == "resource":
        return "RDBC_RESOURCE"
    if domain == "error":
        return "RDBC_ERROR"
    if domain == "compat" and canonical_path[2] == "DruidError":
        return "RDBC_COMPAT_ERROR"
    if domain == "compat":
        return "RDBC_ERROR"
    fail(f"unclassified canonical RDBC path: {'::'.join(canonical_path)}")


def product_family(path: tuple[str, ...]) -> str | None:
    """Classify the explicit Druid product contracts that must remain on druid."""

    if len(path) >= 2 and path[1] == "pool":
        return "DRUID_POOL"
    if len(path) >= 2 and path[1] == "filter":
        return "DRUID_FILTER"
    if len(path) >= 2 and path[1] == "dynamic":
        return "DRUID_LIFECYCLE"
    if len(path) >= 3 and path[1] == "core":
        item = path[2]
        if item.startswith("DruidPooled"):
            return "DRUID_LOGICAL"
        if item in {"Pool", "PoolState"}:
            return "DRUID_POOL"
        if item in {"ConnectionRecycleDisposition", "PhysicalConnectionLease"}:
            return "DRUID_LIFECYCLE"
        if item == "FilterChain":
            return "DRUID_FILTER"
        if item in {
            "Db2ExceptionSorter",
            "ExceptionSorter",
            "MsSqlValidConnectionChecker",
            "MySqlExceptionSorter",
            "MySqlValidConnectionChecker",
            "OceanBaseOracleExceptionSorter",
            "OceanBaseValidConnectionChecker",
            "OracleExceptionSorter",
            "OracleValidConnectionChecker",
            "PgExceptionSorter",
            "PgValidConnectionChecker",
            "ValidConnectionChecker",
        }:
            return "DRUID_CATALOG_CONFIG"
    if len(path) >= 3 and path[1] == "sql":
        if path[2] in {"CommonDataSource", "DataSource"}:
            return "DRUID_LIFECYCLE"
        if path[2] in {"DbType", "RdbcUrl", "RdbcUtils"}:
            return "DRUID_CATALOG_CONFIG"
    if len(path) >= 3 and path[1:3] == ("spi", "driver_extension_descriptor"):
        return "DRUID_CATALOG_CONFIG"
    return None


def classify_path(
    path: tuple[str, ...],
    old_to_new: dict[tuple[str, ...], tuple[str, ...]],
    canonical_paths: set[tuple[str, ...]],
) -> SymbolUse:
    """Classify one referenced dependency path without blessing unknown Druid names."""

    if "*" in path:
        fail(f"wildcard import is forbidden at {'::'.join(path)}")
    if not path or path[0] not in SOURCES:
        fail(f"internal verifier error for path {'::'.join(path)}")

    if path[0] == "druid_rdbc":
        canonical = longest_prefix(path, canonical_paths)
        if canonical is None:
            fail(f"unclassified druid_rdbc symbol: {'::'.join(path)}")
        return SymbolUse(rdbc_family(canonical), "RDBC_DIRECT", "druid_rdbc")

    old_path = longest_prefix(path, set(old_to_new))
    if old_path is not None:
        return SymbolUse(rdbc_family(old_to_new[old_path]), "RDBC_DIRECT", "druid")
    family = product_family(path)
    if family is None:
        fail(f"unclassified druid symbol: {'::'.join(path)}")
    return SymbolUse(family, "DRUID_PRODUCT", "druid")


def scan_live_rows(root: Path) -> list[BoundaryRow]:
    """Recursively inventory every production Rust source, including cfg-gated files."""

    old_to_new, canonical_paths = load_public_paths(root)
    source_root = root / WRAPPER_SOURCE_ROOT
    if not source_root.is_dir():
        fail(f"wrapper source root not found: {WRAPPER_SOURCE_ROOT}")

    origins: dict[tuple[str, str, str], set[str]] = {}
    for source_file in sorted(source_root.rglob("*.rs")):
        relative = source_file.relative_to(root).as_posix()
        try:
            tokens = tokenize(source_file.read_text(encoding="utf-8"))
        except RustLexError as error:
            fail(f"cannot tokenize {relative}: {error}")
        use_paths, covered = target_use_paths(tokens)
        paths = use_paths + fully_qualified_paths(tokens, covered)
        for path in paths:
            symbol_use = classify_path(path, old_to_new, canonical_paths)
            key = (relative, symbol_use.family, symbol_use.classification)
            origins.setdefault(key, set()).add(symbol_use.source)

    rows: list[BoundaryRow] = []
    for key, sources in origins.items():
        if len(sources) != 1:
            fail(
                f"mixed source origins for {key[0]} {key[1]}: "
                + ", ".join(sorted(sources))
            )
        rows.append(BoundaryRow(*key, next(iter(sources))))
    rows.sort(key=lambda row: (row.path, row.symbol_family))
    if not rows:
        fail("live wrapper import set must not be empty")
    return rows


def read_manifest(path: Path) -> list[BoundaryRow]:
    """Read a tracked boundary manifest and reject malformed or duplicate rows."""

    if not path.is_file():
        fail(f"boundary manifest not found: {path}")
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != MANIFEST_HEADER:
        fail(f"invalid boundary manifest header: {path}")
    rows: list[BoundaryRow] = []
    keys: set[tuple[str, str]] = set()
    for line_number, line in enumerate(lines[1:], start=2):
        fields = line.split("\t")
        if len(fields) != 4:
            fail(f"boundary manifest row {line_number} must have four columns: {path}")
        row = BoundaryRow(*fields)
        if not row.path.startswith(f"{WRAPPER_SOURCE_ROOT.as_posix()}/") or not row.path.endswith(".rs"):
            fail(f"boundary manifest row {line_number} has invalid path: {row.path}")
        if row.classification not in CLASSIFICATIONS:
            fail(f"boundary manifest row {line_number} has invalid classification")
        if row.expected_source not in SOURCES:
            fail(f"boundary manifest row {line_number} has invalid expected source")
        expected_classification = (
            "RDBC_DIRECT" if row.symbol_family.startswith("RDBC_") else "DRUID_PRODUCT"
        )
        if row.classification != expected_classification:
            fail(f"boundary manifest row {line_number} has inconsistent symbol family")
        key = (row.path, row.symbol_family)
        if key in keys:
            fail(f"boundary manifest row {line_number} duplicates {key}")
        keys.add(key)
        rows.append(row)
    if not rows:
        fail(f"boundary manifest must not be empty: {path}")
    return sorted(rows, key=lambda row: (row.path, row.symbol_family))


def row_keys(rows: list[BoundaryRow]) -> set[tuple[str, str]]:
    """Return the manifest identity keys independent of source origin."""

    return {(row.path, row.symbol_family) for row in rows}


def verify_manifest_keys(
    live: list[BoundaryRow], manifest: list[BoundaryRow], label: str
) -> None:
    """Require exact live/manifest key equality and diagnose each direction."""

    live_keys = row_keys(live)
    manifest_keys = row_keys(manifest)
    missing = sorted(live_keys - manifest_keys)
    stale = sorted(manifest_keys - live_keys)
    if missing:
        fail(f"{label} manifest missing live keys: {missing}")
    if stale:
        fail(f"{label} manifest has stale manifest keys: {stale}")


def verify_policy(rows: list[BoundaryRow], before: bool) -> None:
    """Require reviewed before/after source policy for every classification."""

    for row in rows:
        required = "druid"
        if not before and row.classification == "RDBC_DIRECT":
            required = "druid_rdbc"
        if row.expected_source != required:
            phase = "before" if before else "after"
            fail(
                f"{phase} policy mismatch for {(row.path, row.symbol_family)}: "
                f"expected {required}, found {row.expected_source}"
            )


def verify_current_sources(live: list[BoundaryRow], manifest: list[BoundaryRow]) -> None:
    """Require each live family to use the reviewed after-cutover source."""

    expected = {(row.path, row.symbol_family): row.expected_source for row in manifest}
    for row in live:
        key = (row.path, row.symbol_family)
        if expected.get(key) != row.expected_source:
            fail(
                f"source mismatch for {key}: expected {expected.get(key)}, "
                f"found {row.expected_source}"
            )


def render_rows(rows: list[BoundaryRow]) -> str:
    """Render rows in deterministic tracked TSV form."""

    body = "\n".join(
        "\t".join((row.path, row.symbol_family, row.classification, row.expected_source))
        for row in rows
    )
    return f"{MANIFEST_HEADER}\n{body}\n"


class WrapperBoundaryVerifierTest(unittest.TestCase):
    """Exercise the live inventory rather than matching verifier source text."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        (self.root / "crates/druid-wrapper/src").mkdir(parents=True)
        mapping = self.root / "docs/verification/rdbc-first/rdbc-public-path-map.tsv"
        mapping.parent.mkdir(parents=True)
        mapping.write_text(
            "old_path\tnew_path\tcompile_mode\told_module_file\tsource_mode\n"
            "druid::core::PhysicalConnection\tdruid_rdbc::spi::PhysicalConnection\t"
            "DYN_TRAIT\tcrates/druid/src/core/mod.rs\tFACADE_MEMBERSHIP\n"
            "druid::core::Value\tdruid_rdbc::types::Value\tOWNED\t"
            "crates/druid/src/core/mod.rs\tFACADE_MEMBERSHIP\n"
            "druid::core::DruidError\tdruid_rdbc::compat::DruidError\tOWNED\t"
            "crates/druid/src/core/mod.rs\tFACADE_MEMBERSHIP\n",
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_source(self, relative: str, source: str) -> None:
        target = self.root / "crates/druid-wrapper/src" / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(source, encoding="utf-8")

    def test_missing_manifest_row_is_rejected(self) -> None:
        self.write_source("adapter.rs", "use druid::core::{PhysicalConnection, Value};\n")
        rows = scan_live_rows(self.root)
        manifest: list[BoundaryRow] = []
        with self.assertRaisesRegex(VerificationError, "missing live keys"):
            verify_manifest_keys(rows, manifest, "before")

    def test_stale_manifest_row_is_rejected(self) -> None:
        self.write_source("adapter.rs", "use druid::core::PhysicalConnection;\n")
        rows = scan_live_rows(self.root)
        stale = rows + [BoundaryRow("crates/druid-wrapper/src/stale.rs", "RDBC_TYPES", "RDBC_DIRECT", "druid")]
        with self.assertRaisesRegex(VerificationError, "stale manifest keys"):
            verify_manifest_keys(rows, stale, "before")

    def test_extra_live_row_is_rejected(self) -> None:
        self.write_source("adapter.rs", "use druid::core::{PhysicalConnection, Value};\n")
        rows = scan_live_rows(self.root)
        with self.assertRaisesRegex(VerificationError, "missing live keys"):
            verify_manifest_keys(rows, rows[:1], "after")

    def test_wildcard_import_is_rejected(self) -> None:
        self.write_source("adapter.rs", "use druid::core::*;\n")
        with self.assertRaisesRegex(VerificationError, "wildcard"):
            scan_live_rows(self.root)

    def test_unclassified_druid_symbol_is_rejected(self) -> None:
        self.write_source("adapter.rs", "use druid::core::MysteryContract;\n")
        with self.assertRaisesRegex(VerificationError, "unclassified"):
            scan_live_rows(self.root)

    def test_renamed_nested_multiline_use_keeps_canonical_family(self) -> None:
        self.write_source(
            "adapter.rs",
            "use druid::{\n    core::{PhysicalConnection as RawConnection, Value},\n"
            "    pool::DruidDataSource as ProductDataSource,\n};\n",
        )
        rows = scan_live_rows(self.root)
        self.assertEqual(
            [(row.symbol_family, row.classification, row.expected_source) for row in rows],
            [
                ("DRUID_POOL", "DRUID_PRODUCT", "druid"),
                ("RDBC_SPI", "RDBC_DIRECT", "druid"),
                ("RDBC_TYPES", "RDBC_DIRECT", "druid"),
            ],
        )

    def test_fully_qualified_old_path_is_inventory_and_rejected_after(self) -> None:
        self.write_source(
            "adapter.rs",
            "fn check(_: &dyn druid::core::PhysicalConnection) {}\n",
        )
        rows = scan_live_rows(self.root)
        after = [
            BoundaryRow(row.path, row.symbol_family, row.classification, "druid_rdbc")
            for row in rows
        ]
        with self.assertRaisesRegex(VerificationError, "source mismatch"):
            verify_current_sources(rows, after)

    def test_cfg_feature_file_is_scanned_even_when_feature_is_disabled(self) -> None:
        self.write_source(
            "feature/adapter.rs",
            "#![cfg(feature = \"optional-driver\")]\nuse druid::core::DruidError;\n",
        )
        rows = scan_live_rows(self.root)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0].symbol_family, "RDBC_COMPAT_ERROR")

    def test_product_rdbc_url_is_not_confused_with_callable_url_value(self) -> None:
        self.write_source("catalog.rs", "use druid::sql::RdbcUrl;\n")
        rows = scan_live_rows(self.root)
        self.assertEqual(
            rows,
            [
                BoundaryRow(
                    "crates/druid-wrapper/src/catalog.rs",
                    "DRUID_CATALOG_CONFIG",
                    "DRUID_PRODUCT",
                    "druid",
                )
            ],
        )

    def test_local_druid_names_do_not_masquerade_as_dependency_paths(self) -> None:
        self.write_source(
            "adapter.rs",
            "use crate::druid::LocalHelper;\n"
            "use druid::core::PhysicalConnection;\n"
            "fn local_name() { let druid = LocalHelper; let _ = druid; }\n",
        )
        try:
            rows = scan_live_rows(self.root)
        except VerificationError as error:
            self.fail(f"local names must be ignored: {error}")
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0].symbol_family, "RDBC_SPI")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--emit-live", action="store_true")
    parser.add_argument("--before", type=Path)
    parser.add_argument("--after", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(WrapperBoundaryVerifierTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    root = repository_root()
    live = scan_live_rows(root)
    if args.emit_live:
        if args.before is not None or args.after is not None:
            fail("--emit-live cannot be combined with --before/--after")
        sys.stdout.write(render_rows(live))
        return 0
    if args.before is None or args.after is None:
        fail("verification requires both --before and --after")
    before = read_manifest(root / args.before)
    after = read_manifest(root / args.after)
    verify_manifest_keys(live, before, "before")
    verify_manifest_keys(live, after, "after")
    if row_keys(before) != row_keys(after):
        fail("before/after manifest keys differ")
    verify_policy(before, before=True)
    verify_policy(after, before=False)
    verify_current_sources(live, after)
    file_count = len({row.path for row in live})
    family_count = len({row.symbol_family for row in live})
    print(
        f"PASS: wrapper RDBC import boundary ({len(live)} rows, "
        f"{file_count} files, {family_count} families)"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (VerificationError, RustLexError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        raise SystemExit(1) from None
