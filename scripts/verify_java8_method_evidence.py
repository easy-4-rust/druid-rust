#!/usr/bin/env python3
"""Fail-closed checks for Java 8 JDBC method evidence and precise negative claims."""

from __future__ import annotations

import argparse
import csv
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
FIELDS = (
    "identity", "owner", "name", "parameters", "rust_type_entry", "rust_method_entry",
    "spec_scenario", "batch", "backend", "declaration", "contract", "adapter",
    "logical_proxy", "real_db", "capability", "positive_evidence",
    "unsupported_evidence", "unsupported_result", "not_run_reason",
)
LAYERS = ("declaration", "contract", "adapter", "logical_proxy", "real_db")
VALID_STATES = {"UNASSESSED", "PASS", "FAIL", "NOT_RUN", "NOT_APPLICABLE"}


class EvidenceError(ValueError):
    """Invalid or unsupported evidence claim."""


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader((line for line in source if not line.startswith("#")), delimiter="\t"))


def evidence_exists(reference: str) -> bool:
    if not reference:
        return False
    file_name, separator, symbol = reference.partition("#")
    path = (ROOT / file_name).resolve()
    if ROOT.resolve() not in path.parents or not path.is_file():
        return False
    if separator:
        text = path.read_text(encoding="utf-8", errors="replace")
        return bool(re.search(rf"\b(?:async\s+)?fn\s+{re.escape(symbol)}\s*\(", text))
    return True


def verify(ledger: Path, inventory: Path) -> dict[str, int]:
    source_rows = read_rows(inventory)
    rows = read_rows(ledger)
    source_ids = {row["identity"] for row in source_rows}
    ledger_ids = [row.get("identity", "") for row in rows]
    if len(source_ids) != len(source_rows):
        raise EvidenceError("Java inventory has duplicate identities")
    if len(ledger_ids) != len(set(ledger_ids)):
        raise EvidenceError("evidence ledger has duplicate Java identities")
    if source_ids != set(ledger_ids):
        raise EvidenceError(f"ledger denominator mismatch: missing={len(source_ids-set(ledger_ids))}, extra={len(set(ledger_ids)-source_ids)}")
    for line, row in enumerate(rows, start=2):
        missing = [field for field in FIELDS if field not in row]
        if missing:
            raise EvidenceError(f"row {line}: missing columns {missing}")
        if any(row[field] not in VALID_STATES for field in LAYERS):
            raise EvidenceError(f"row {line}: invalid evidence layer state")
        if row["capability"] not in {"UNASSESSED", "SUPPORTED", "PARTIAL", "UNSUPPORTED", "NOT_RUN"}:
            raise EvidenceError(f"row {line}: invalid capability state")
        if row["capability"] == "SUPPORTED":
            if not row["rust_method_entry"] or row["rust_method_entry"] == "UNMAPPED":
                raise EvidenceError(f"row {line}: supported capability lacks Rust method mapping")
            if not evidence_exists(row["positive_evidence"]):
                raise EvidenceError(f"row {line}: supported capability lacks a positive test/evidence reference")
            if not any(row[layer] == "PASS" for layer in LAYERS):
                raise EvidenceError(f"row {line}: supported capability lacks a passed evidence layer")
        if row["capability"] == "UNSUPPORTED":
            result = row["unsupported_result"]
            if not evidence_exists(row["unsupported_evidence"]):
                raise EvidenceError(f"row {line}: unsupported capability lacks precise negative evidence")
            if not result or result in {"UNSUPPORTED", "ERROR", "0A000"}:
                raise EvidenceError(f"row {line}: unsupported claim requires an exact observable error (SQLState alone is insufficient)")
        if row["capability"] == "NOT_RUN" and not row["not_run_reason"]:
            raise EvidenceError(f"row {line}: NOT_RUN requires a reason")
        for reference in (row["positive_evidence"], row["unsupported_evidence"]):
            if reference and not evidence_exists(reference):
                raise EvidenceError(f"row {line}: stale or missing evidence reference: {reference}")
    return {"inventory": len(source_rows), "ledger": len(rows),
            "linked": sum(row["capability"] != "UNASSESSED" for row in rows)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ledger", type=Path, default=ROOT / "docs/verification/rdbc-first/java8-method-evidence.tsv")
    parser.add_argument("--inventory", type=Path, default=ROOT / "docs/verification/rdbc-first/java8-public-method-inventory.tsv")
    args = parser.parse_args()
    try:
        result = verify(args.ledger, args.inventory)
    except (EvidenceError, OSError, KeyError) as error:
        print(f"EVIDENCE_GATE_FAIL: {error}", file=sys.stderr)
        return 1
    print(f"EVIDENCE_GATE_PASS inventory={result['inventory']} ledger={result['ledger']} linked={result['linked']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
