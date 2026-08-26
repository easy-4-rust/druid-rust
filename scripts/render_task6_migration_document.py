#!/usr/bin/env python3
"""Render/check the human Task 6 migration table from its authoritative TSV."""

from __future__ import annotations

import argparse
import csv
from collections import defaultdict
from pathlib import Path


AUTHORITY = Path("docs/verification/rdbc-first/task6-migration-authority.tsv")
DOCUMENT = Path("docs/rdbc/physical-to-standard-migration.md")
HEADER = (
    "old_path",
    "disposition",
    "target_path",
    "target_task",
    "state",
    "source_file",
    "normalized_signature",
    "method_handoff",
    "test_handoff",
    "evidence",
)
TABLE_HEADER = (
    "| old_path | disposition | target_path | target_task | state | evidence |"
)
NEXT_HEADING = "## Deleted integration-test dispositions"
TEST_LEDGER = Path("docs/verification/rdbc-first/task6-deleted-test-migration.tsv")
TEST_TABLE_HEADER = "| deleted_test_path | base_test_functions | disposition | exact_handoff | behavioral_themes |"
FINAL_INVARIANTS_HEADING = "## Final cutover invariants"
ZERO_TEST_IDENTITY = "crates/druid-wrapper/tests/common/mod.rs"
ZERO_TEST_BEHAVIOR = "crates/druid-wrapper/tests/common/physical_connection_contract.rs"
ZERO_TEST_BEHAVIOR_HANDOFFS = (
    "crates/druid-wrapper/tests/sqlx_bb8_pool_test.rs#bb8_pool_executes_and_recycles_one_canonical_lease",
    "crates/druid-wrapper/tests/sqlx_bb8_pool_test.rs#bb8_transactional_close_resets_and_dirty_drop_discards",
    "crates/druid-wrapper/tests/sqlx_bb8_pool_test.rs#bb8_fatal_disposition_poison_rejects_transaction_dirty_connection",
    "crates/druid-wrapper/tests/sqlx_bb8_pool_test.rs#bb8_recycle_error_poison_rejects_transaction_dirty_connection",
    "crates/druid-wrapper/tests/sqlx_deadpool_pool_test.rs#deadpool_executes_recycles_and_resets_one_canonical_lease",
    "crates/druid-wrapper/tests/sqlx_deadpool_pool_test.rs#deadpool_dirty_transaction_drop_is_discarded",
    "crates/druid-wrapper/tests/sqlx_deadpool_pool_test.rs#deadpool_fatal_disposition_poison_rejects_transaction_dirty_connection",
    "crates/druid-wrapper/tests/sqlx_deadpool_pool_test.rs#deadpool_recycle_error_poison_rejects_transaction_dirty_connection",
)


class RenderError(RuntimeError):
    """Raised when the authority or derived document is malformed."""


def load_rows(path: Path) -> tuple[dict[str, str], ...]:
    """Load the exact canonical authority schema and unique 79-row denominator."""

    payload = path.read_bytes()
    if not payload.endswith(b"\n") or payload.endswith(b"\n\n") or b"\r" in payload:
        raise RenderError("migration authority must have exactly one LF final newline")
    rows = tuple(csv.DictReader(payload.decode("utf-8").splitlines(), delimiter="\t"))
    if not rows or tuple(rows[0]) != HEADER:
        raise RenderError("migration authority header is not exact")
    if len(rows) != 79:
        raise RenderError(
            f"migration authority must contain 79 rows, found {len(rows)}"
        )
    paths = [row["old_path"] for row in rows]
    if len(set(paths)) != len(paths):
        raise RenderError("migration authority contains duplicate old paths")
    return rows


def render_table(rows: tuple[dict[str, str], ...]) -> str:
    """Render deterministic Markdown without inventing a second mapping authority."""

    lines = [
        TABLE_HEADER,
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for row in rows:
        cells = (
            row["old_path"],
            row["disposition"],
            row["target_path"],
            row["target_task"],
            row["state"],
            row["evidence"],
        )
        if any("|" in cell or "\n" in cell or "\r" in cell for cell in cells):
            raise RenderError(
                f"migration row cannot be rendered safely: {row['old_path']}"
            )
        lines.append("| " + " | ".join(f"`{cell}`" for cell in cells) + " |")
    return "\n".join(lines) + "\n"


def replace_table(document: str, rendered: str) -> str:
    """Replace only the generated old-path table, retaining the behavioral sections."""

    start = document.find(TABLE_HEADER)
    if start < 0:
        legacy = "| old_path | disposition | target_path | evidence |"
        start = document.find(legacy)
    end = document.find(NEXT_HEADING)
    if start < 0 or end < 0 or end <= start:
        raise RenderError("migration document table boundaries are missing")
    prefix = document[:start]
    suffix = document[end:]
    return prefix + rendered + "\n" + suffix


def render_deleted_test_table(path: Path) -> str:
    """Render the exact 67-file summary without replacing the 1,283-function authority."""

    rows = tuple(
        csv.DictReader(path.read_text(encoding="utf-8").splitlines(), delimiter="\t")
    )
    if len(rows) != 1283:
        raise RenderError(
            f"deleted-test function ledger must contain 1283 rows, found {len(rows)}"
        )
    by_path: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in rows:
        by_path[row["base_source_path"]].append(row)
    if len(by_path) != 65:
        raise RenderError(
            f"deleted-test function ledger must cover 65 files, found {len(by_path)}"
        )
    output = [
        TEST_TABLE_HEADER,
        "| --- | ---: | --- | --- | --- |",
    ]
    summaries: list[tuple[str, int, str, str, str]] = []
    for source_path, source_rows in by_path.items():
        themes = sorted(
            {
                theme
                for row in source_rows
                for theme in row["behavioral_theme"].split("+")
                if theme
            }
        )
        dispositions = sorted({row["disposition"] for row in source_rows})
        if any(
            disposition != "MIGRATED_EXACT"
            and not disposition.startswith("MERGED_PARAMETERIZED(")
            for disposition in dispositions
        ):
            raise RenderError(
                f"deleted-test file has unresolved dispositions: {source_path}: {dispositions}"
            )
        has_exact = "MIGRATED_EXACT" in dispositions
        has_parameterized = any(
            disposition.startswith("MERGED_PARAMETERIZED(")
            for disposition in dispositions
        )
        summary_disposition = {
            (True, False): "MIGRATED_EXACT",
            (False, True): "MERGED_PARAMETERIZED",
            (True, True): "MIGRATED_MIXED",
        }[(has_exact, has_parameterized)]
        summaries.append(
            (
                source_path,
                len(source_rows),
                summary_disposition,
                f"task6-deleted-test-migration.tsv#base_source_path={source_path};count={len(source_rows)}",
                "+".join(themes),
            )
        )
    summaries.extend(
        (
            (ZERO_TEST_IDENTITY, 0, "PURE_REMOVED_API_IDENTITY", "-", "module_shell"),
            (
                ZERO_TEST_BEHAVIOR,
                0,
                "MIGRATED_EXACT",
                ";".join(ZERO_TEST_BEHAVIOR_HANDOFFS),
                "external_pool+transaction_recycle+fatal+recycle_error",
            ),
        )
    )
    for source_path, count, disposition, handoff, themes in sorted(summaries):
        cells = (source_path, str(count), disposition, handoff, themes)
        if any("|" in cell or "\n" in cell or "\r" in cell for cell in cells):
            raise RenderError(f"deleted-test summary cannot be rendered: {source_path}")
        output.append("| " + " | ".join(f"`{cell}`" for cell in cells) + " |")
    if len(summaries) != 67:
        raise RenderError(
            f"deleted-test file denominator must be 67, found {len(summaries)}"
        )
    return "\n".join(output) + "\n"


def replace_deleted_test_table(document: str, rendered: str) -> str:
    """Replace the stale file-level table while retaining later report sections."""

    start = document.find(TEST_TABLE_HEADER)
    if start < 0:
        start = document.find(
            "| deleted_test_path | disposition | replacement_tests | behavioral_assertions |"
        )
    end = document.find(FINAL_INVARIANTS_HEADING)
    if start < 0 or end < 0 or end <= start:
        raise RenderError("deleted-test migration table boundaries are missing")
    return document[:start] + rendered + "\n" + document[end:]


def main() -> int:
    """Write the derived table on request, otherwise fail closed on drift."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    parser.add_argument("--write", action="store_true")
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    rows = load_rows(root / AUTHORITY)
    rendered = render_table(rows)
    path = root / DOCUMENT
    current = path.read_text(encoding="utf-8")
    expected = replace_table(current, rendered)
    expected = replace_deleted_test_table(
        expected, render_deleted_test_table(root / TEST_LEDGER)
    )
    if arguments.write:
        path.write_text(expected, encoding="utf-8", newline="\n")
        print(f"TASK6_MIGRATION_DOCUMENT_ROWS={len(rows)}")
        return 0
    if current != expected:
        print("TASK6_MIGRATION_DOCUMENT_FAIL=derived Markdown table is stale")
        return 1
    print(f"TASK6_MIGRATION_DOCUMENT_ROWS={len(rows)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
