#!/usr/bin/env python3
"""Verify the sole 79-path Task 6 migration authority and its derived artifacts."""

from __future__ import annotations

import argparse
import csv
import hashlib
import re
from collections import Counter
from pathlib import Path
from typing import NoReturn

from render_task6_migration_document import (
    AUTHORITY,
    DOCUMENT,
    FINAL_INVARIANTS_HEADING,
    HEADER,
    NEXT_HEADING,
    TABLE_HEADER,
    TEST_LEDGER,
    TEST_TABLE_HEADER,
    load_rows,
    render_deleted_test_table,
    render_table,
)


PHYSICAL = Path("docs/verification/rdbc-first/rdbc-physical-standardization.tsv")
METHOD_LEDGER = Path("docs/verification/rdbc-first/task6-method-migration.tsv")
TEST_LEDGER = Path("docs/verification/rdbc-first/task6-deleted-test-migration.tsv")
API_SNAPSHOT = Path("api/druid-rdbc.single-standard.after.txt")
ALLOWED_DISPOSITIONS = {
    "MERGED_STANDARD",
    "INTERNALIZED",
    "CONCRETE_MOVED",
    "DELETED",
}
ALLOWED_STATES = {"TASK6_CLOSED", "TASK7_PENDING"}
CLASSIFICATION_DISPOSITION = {
    "MERGE_STANDARD": "MERGED_STANDARD",
    "RDBC_EXTENSION": "MERGED_STANDARD",
    "DRUID_INTERNAL": "INTERNALIZED",
    "CONCRETE_IMPL": "CONCRETE_MOVED",
}
STANDARDIZATION_ONLY_TASK7 = {
    "druid_rdbc::resource::Array",
    "druid_rdbc::resource::Blob",
    "druid_rdbc::resource::Clob",
    "druid_rdbc::resource::NClob",
    "druid_rdbc::resource::Ref",
    "druid_rdbc::resource::SqlXml",
}
METHOD_COUNTS = {
    "PhysicalConnection": 52,
    "PhysicalStatement": 30,
    "PhysicalPreparedStatement": 32,
    "PhysicalCallableStatement": 32,
    "PhysicalResultSet": 120,
    "PhysicalDatabaseMetaData": 173,
    "PhysicalResultSetMetaData": 21,
    "ConnectionExt": 19,
}
TEST_HANDOFF = "task6-deleted-test-migration.tsv#exact-denominator=1283"


class VerificationError(RuntimeError):
    """Raised when an authority mapping or handoff can be forged or is stale."""


def fail(message: str) -> NoReturn:
    """Stop on the first deterministic authority violation."""

    raise VerificationError(message)


def read_tsv(path: Path) -> tuple[dict[str, str], ...]:
    """Read a canonical LF TSV with a complete header."""

    payload = path.read_bytes()
    if not payload.endswith(b"\n") or payload.endswith(b"\n\n") or b"\r" in payload:
        fail(f"TSV must have exactly one LF final newline: {path}")
    rows = tuple(csv.DictReader(payload.decode("utf-8").splitlines(), delimiter="\t"))
    if not rows or None in rows[0] or any(None in row.values() for row in rows):
        fail(f"TSV schema is malformed: {path}")
    return rows


def verify_authority_schema(rows: tuple[dict[str, str], ...]) -> None:
    """Require exact columns, row states, tasks, and non-forgeable handoff selectors."""

    if len(rows) != 79 or tuple(rows[0]) != HEADER:
        fail("migration authority schema/79-row denominator is not exact")
    counts = Counter(row["old_path"] for row in rows)
    if any(count != 1 for count in counts.values()):
        fail("migration authority old paths are not unique")
    for row in rows:
        old_path = row["old_path"]
        if row["disposition"] not in ALLOWED_DISPOSITIONS:
            fail(f"migration disposition is invalid: {old_path}")
        if row["state"] not in ALLOWED_STATES:
            fail(f"migration state is invalid: {old_path}")
        if not row["target_task"].startswith("P04_TASK_"):
            fail(f"migration target task is invalid: {old_path}")
        if row["state"] == "TASK7_PENDING" and row["target_task"] != (
            "P04_TASK_7_SEMANTIC_HARDENING"
        ):
            fail(f"Task 7 pending row has another owner: {old_path}")
        if row["disposition"] == "DELETED":
            if row["target_path"] != "-":
                fail(f"deleted row must use '-' target: {old_path}")
        elif row["target_path"] == "-":
            fail(f"non-deleted row has no target: {old_path}")
        if not row["source_file"] or not row["normalized_signature"]:
            fail(f"BASE source identity is missing: {old_path}")
        if row["test_handoff"] != TEST_HANDOFF:
            fail(f"exact test handoff is missing: {old_path}")
        leaf = old_path.rsplit("::", 1)[-1]
        if leaf == "PhysicalConnection":
            expected = (
                "task6-method-migration.tsv#object=PhysicalConnection;count=52;"
                "object=ConnectionExt;count=19"
            )
            if row["method_handoff"] != expected:
                fail(f"method handoff selector is not exact: {old_path}")
        elif leaf in METHOD_COUNTS:
            expected = (
                f"task6-method-migration.tsv#object={leaf};"
                f"count={METHOD_COUNTS[leaf]}"
            )
            if row["method_handoff"] != expected:
                fail(f"method handoff selector is not exact: {old_path}")
        elif row["method_handoff"] != "NOT_APPLICABLE":
            fail(f"non-method identity has an unexpected handoff: {old_path}")
        if not row["evidence"]:
            fail(f"migration evidence is missing: {old_path}")


def verify_standardization(
    authority_rows: tuple[dict[str, str], ...],
    physical_rows: tuple[dict[str, str], ...],
) -> None:
    """Compare overlap field-exactly and keep the six Task 7 aliases pending."""

    authority = {row["old_path"]: row for row in authority_rows}
    physical = {row["old_path"]: row for row in physical_rows}
    if len(authority) != len(authority_rows) or len(physical) != len(physical_rows):
        fail("migration or physical standardization manifest has duplicate paths")
    overlap = set(authority).intersection(physical)
    if len(overlap) != 67:
        fail(f"authority/standardization overlap must be 67, found {len(overlap)}")
    extra = set(physical).difference(authority)
    if extra != STANDARDIZATION_ONLY_TASK7:
        fail(f"unexpected standardization-only paths: {sorted(extra)}")
    for old_path in sorted(overlap):
        source = physical[old_path]
        target = authority[old_path]
        disposition = CLASSIFICATION_DISPOSITION.get(source["classification"])
        fields = (
            disposition,
            source["target_path"],
            source["target_task"],
            source["source_file"],
            source["normalized_signature"],
        )
        expected = (
            target["disposition"],
            target["target_path"],
            target["target_task"],
            target["source_file"],
            target["normalized_signature"],
        )
        if fields != expected:
            fail(f"standardization mapping disagrees with authority: {old_path}")
        if target["state"] == "TASK7_PENDING":
            if source["compatibility"] != "TASK7_PENDING":
                fail(f"Task 7 row is falsely closed: {old_path}")
        elif source["compatibility"] not in {
            "TASK6_CLOSED",
            "MOVE_TO_DRUID_P04_TASK5",
        }:
            fail(f"closed authority row has an invalid historical state: {old_path}")
    for old_path in sorted(extra):
        row = physical[old_path]
        if row["compatibility"] != "TASK7_PENDING" or row["target_task"] != (
            "P04_TASK_7_SEMANTIC_HARDENING"
        ):
            fail(f"standardization-only Task 7 alias is falsely closed: {old_path}")


def module_declares_exact_target(root: Path, target: str) -> bool:
    """Resolve the exact private module path; never accept a same-named leaf elsewhere."""

    crate, separator, remainder = target.partition("::")
    crate_directory = {
        "druid": "druid",
        "druid_wrapper": "druid-wrapper",
    }.get(crate)
    if not separator or crate_directory is None:
        return False
    parts = remainder.split("::")
    if len(parts) < 2:
        return False
    candidate_parts = parts[:-1]
    leaf = parts[-1]
    source_root = root / "crates" / crate_directory / "src"
    module_parts: list[str] | None = None
    source: Path | None = None
    for length in range(len(candidate_parts), 0, -1):
        prefix = candidate_parts[:length]
        module_file = source_root.joinpath(*prefix).with_suffix(".rs")
        module_directory_file = source_root.joinpath(*prefix, "mod.rs")
        candidate = module_file if module_file.is_file() else module_directory_file
        if candidate.is_file():
            module_parts = prefix
            source = candidate
            break
    if module_parts is None or source is None:
        return False
    declaration = re.compile(
        rf"\b(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe)\s+)*(?:struct|enum|trait|type|fn)\s+{re.escape(leaf)}\b"
    )
    if not declaration.search(source.read_text(encoding="utf-8")):
        return False
    owners = candidate_parts[len(module_parts) :]
    source_text = source.read_text(encoding="utf-8")
    if owners and not all(
        re.search(rf"\bimpl(?:<[^>]*>)?\s+{re.escape(owner)}\b", source_text)
        for owner in owners
    ):
        return False
    parent = source_root / "lib.rs"
    for index, module in enumerate(module_parts):
        if not parent.is_file():
            return False
        declaration_pattern = re.compile(
            rf"\b(?:pub(?:\([^)]*\))?\s+)?mod\s+{re.escape(module)}\s*;"
        )
        if not declaration_pattern.search(parent.read_text(encoding="utf-8")):
            return False
        prefix = module_parts[: index + 1]
        flat = source_root.joinpath(*prefix).with_suffix(".rs")
        nested = source_root.joinpath(*prefix, "mod.rs")
        parent = flat if flat.is_file() else nested
    return True


def public_api_items(text: str) -> set[str]:
    """Extract exact druid-rdbc paths from cargo-public-api stable text."""

    return set(re.findall(r"\bdruid_rdbc(?:::[A-Za-z_][A-Za-z0-9_]*)+", text))


def verify_closed_targets(
    root: Path,
    rows: tuple[dict[str, str], ...],
    api_items: set[str],
) -> None:
    """Require a real exact target for every closed non-deleted migration."""

    for row in rows:
        if row["state"] == "TASK7_PENDING" or row["disposition"] == "DELETED":
            continue
        target = row["target_path"]
        if target.startswith("druid_rdbc::"):
            exists = target in api_items
        else:
            exists = module_declares_exact_target(root, target)
        if not exists:
            fail(f"closed migration target does not exist at exact path: {target}")


def verify_handoff_ledgers(root: Path, rows: tuple[dict[str, str], ...]) -> None:
    """Bind authority selectors to the exact 479-method and 1,283-test ledgers."""

    method_rows = read_tsv(root / METHOD_LEDGER)
    if len(method_rows) != sum(METHOD_COUNTS.values()):
        fail(f"method ledger must contain 479 rows, found {len(method_rows)}")
    method_counts = Counter(row["source_object"] for row in method_rows)
    if method_counts != Counter(METHOD_COUNTS):
        fail(f"method ledger object denominator drift: {method_counts}")
    test_rows = read_tsv(root / TEST_LEDGER)
    if len(test_rows) != 1283:
        fail(f"test ledger must contain 1283 rows, found {len(test_rows)}")
    identities = [row["base_identity"] for row in test_rows]
    if len(set(identities)) != len(identities):
        fail("test ledger base identities are not unique")
    if any(row["disposition"] in {"MISSING", "UNREVIEWED"} for row in test_rows):
        fail("test ledger contains an unresolved disposition")
    verify_authority_schema(rows)


def verify_derived_document(root: Path, rows: tuple[dict[str, str], ...]) -> None:
    """Require the Markdown table to be byte-derived from the TSV authority."""

    document = (root / DOCUMENT).read_text(encoding="utf-8")
    start = document.find(TABLE_HEADER)
    end = document.find(NEXT_HEADING)
    if start < 0 or end < 0 or end <= start:
        fail("derived migration document table boundaries are missing")
    observed = document[start:end].rstrip("\n") + "\n"
    expected = render_table(rows)
    if observed != expected:
        fail("derived migration document disagrees with authority")
    test_start = document.find(TEST_TABLE_HEADER)
    test_end = document.find(FINAL_INVARIANTS_HEADING)
    if test_start < 0 or test_end < 0 or test_end <= test_start:
        fail("derived deleted-test table boundaries are missing")
    observed_tests = document[test_start:test_end].rstrip("\n") + "\n"
    expected_tests = render_deleted_test_table(root / TEST_LEDGER)
    if observed_tests != expected_tests:
        fail("derived deleted-test file index disagrees with exact function ledger")


def verify_repository(root: Path) -> tuple[int, int, int, str]:
    """Run every authority, exact-target, handoff, and derived-artifact check."""

    rows = load_rows(root / AUTHORITY)
    verify_authority_schema(rows)
    physical_rows = read_tsv(root / PHYSICAL)
    verify_standardization(rows, physical_rows)
    api = (root / API_SNAPSHOT).read_text(encoding="utf-8")
    verify_closed_targets(root, rows, public_api_items(api))
    verify_handoff_ledgers(root, rows)
    verify_derived_document(root, rows)
    authority_hash = hashlib.sha256((root / AUTHORITY).read_bytes()).hexdigest()
    pending = sum(row["state"] == "TASK7_PENDING" for row in rows)
    return len(rows), pending, len(physical_rows), authority_hash


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    arguments = parser.parse_args()
    try:
        count, pending, physical, digest = verify_repository(arguments.root.resolve())
    except (OSError, UnicodeError, VerificationError, RuntimeError) as error:
        print(f"TASK6_MIGRATION_AUTHORITY_FAIL={error}")
        return 1
    print(f"TASK6_MIGRATION_AUTHORITY_ROWS={count}")
    print(f"TASK7_MIGRATION_ROWS_PENDING={pending}")
    print(f"PHYSICAL_STANDARDIZATION_ROWS={physical}")
    print(f"TASK6_MIGRATION_AUTHORITY_SHA256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
