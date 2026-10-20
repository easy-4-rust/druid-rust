#!/usr/bin/env python3
"""Fail-closed checks for Java 8 JDBC method evidence and precise negative claims."""

from __future__ import annotations

import argparse
import csv
import hashlib
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
FIELDS = (
    "identity", "owner", "name", "parameters", "rust_type_entry", "rust_method_entry",
    "spec_scenario", "batch", "backend", "declaration", "contract", "adapter",
    "logical_proxy", "real_db", "capability", "rust_source", "rust_source_sha256",
    "positive_evidence", "positive_evidence_sha256", "unsupported_evidence",
    "unsupported_evidence_sha256", "unsupported_result", "not_run_reason",
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


def specification_exists(reference: str) -> bool:
    """Resolve an exact requirement ID inside the repository's OpenSpec fact source."""
    file_name, separator, requirement = reference.partition("#")
    relative = Path(file_name)
    if (not separator or relative.is_absolute() or ".." in relative.parts
            or len(relative.parts) != 2 or relative.name != "spec.md"
            or not re.fullmatch(r"[A-Z]+-\d+", requirement)):
        return False
    directory = (ROOT / "openspec/specs").resolve()
    path = (directory / relative).resolve()
    if directory not in path.parents or not path.is_file():
        return False
    headings = re.findall(r"^### Requirement: " + re.escape(requirement) + r"(?:[ \t].*)?$",
                          path.read_text(encoding="utf-8"), re.MULTILINE)
    return len(headings) == 1


def file_digest(reference: str) -> str:
    """Hash an evidence reference's file, ignoring any #test selector."""
    path = (ROOT / reference.partition("#")[0]).resolve()
    if ROOT.resolve() not in path.parents or not path.is_file():
        return ""
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def inventory_metadata(path: Path) -> tuple[str, str]:
    """Read the version marker and source commit from the frozen API denominator."""
    first_line = path.read_text(encoding="utf-8").splitlines()[0]
    match = re.fullmatch(r"# schema=([^\t]+)\tsource_commit=(.+)", first_line)
    if not match:
        raise EvidenceError("Java inventory has no recognized schema/source version header")
    return match.group(1), match.group(2)


def verify(ledger: Path, inventory: Path) -> dict[str, int]:
    ledger_header = ledger.read_text(encoding="utf-8").splitlines()[0]
    schema, source_commit = inventory_metadata(inventory)
    expected_header = (
        f"# schema=java8-method-evidence-v1\tinventory_sha256="
        f"{hashlib.sha256(inventory.read_bytes()).hexdigest().upper()}\tsource_schema={schema}"
        f"\tsource_commit={source_commit}"
    )
    if ledger_header != expected_header:
        raise EvidenceError("ledger inventory hash or source version is stale; regenerate/review evidence links")
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
        if row["capability"] == "UNASSESSED" and any(row[layer] != "UNASSESSED" for layer in LAYERS):
            raise EvidenceError(f"row {line}: unassessed capability cannot claim an assessed layer")
        if row["capability"] != "UNASSESSED" and not specification_exists(row["spec_scenario"]):
            raise EvidenceError(f"row {line}: missing or ambiguous OpenSpec requirement reference")
        if row["capability"] in {"SUPPORTED", "PARTIAL"}:
            if not row["rust_method_entry"] or row["rust_method_entry"] == "UNMAPPED":
                raise EvidenceError(f"row {line}: linked capability lacks Rust method mapping")
            if not evidence_exists(row["positive_evidence"]):
                raise EvidenceError(f"row {line}: linked capability lacks a positive test/evidence reference")
            if not any(row[layer] == "PASS" for layer in LAYERS):
                raise EvidenceError(f"row {line}: linked capability lacks a passed evidence layer")
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
        for reference_field, hash_field in (("positive_evidence", "positive_evidence_sha256"),
                                            ("unsupported_evidence", "unsupported_evidence_sha256")):
            reference = row[reference_field]
            recorded_hash = row[hash_field]
            if reference and (not recorded_hash or file_digest(reference) != recorded_hash.upper()):
                raise EvidenceError(f"row {line}: stale evidence source hash for {reference}")
        if row["rust_source"]:
            rust_reference = row["rust_source"]
            rust_path = (ROOT / rust_reference).resolve()
            if (ROOT.resolve() not in rust_path.parents or not rust_path.is_file()
                    or file_digest(rust_reference) != row["rust_source_sha256"].upper()):
                raise EvidenceError(f"row {line}: stale or missing Rust source hash for {rust_reference}")
            if row["rust_method_entry"] and row["rust_method_entry"] != "UNMAPPED":
                rust_text = rust_path.read_text(encoding="utf-8", errors="replace")
                rust_method = row["rust_method_entry"].rsplit("::", 1)[-1]
                if not re.search(rf"\bfn\s+{re.escape(rust_method)}\s*(?:<|\()", rust_text):
                    raise EvidenceError(f"row {line}: Rust method entry is absent from {rust_reference}")
        elif row["capability"] in {"SUPPORTED", "PARTIAL"}:
            raise EvidenceError(f"row {line}: linked capability lacks a Rust source file")
    return {"inventory": len(source_rows), "ledger": len(rows),
            "linked": sum(row["capability"] != "UNASSESSED" for row in rows),
            "unassessed": sum(row["capability"] == "UNASSESSED" for row in rows),
            "not_run": sum(row["capability"] == "NOT_RUN" for row in rows),
            "failed_layers": sum(row[layer] == "FAIL" for row in rows for layer in LAYERS),
            "incomplete": sum(row["capability"] not in {"SUPPORTED", "UNSUPPORTED"}
                              or any(row[layer] in {"UNASSESSED", "NOT_RUN", "FAIL"} for layer in LAYERS)
                              for row in rows)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ledger", type=Path, default=ROOT / "docs/verification/rdbc-first/java8-method-evidence.tsv")
    parser.add_argument("--inventory", type=Path, default=ROOT / "docs/verification/rdbc-first/java8-public-method-inventory.tsv")
    parser.add_argument("--require-complete", action="store_true",
                        help="also fail for partial, unassessed, not-run or failed coverage; default checks integrity only")
    args = parser.parse_args()
    try:
        result = verify(args.ledger, args.inventory)
    except (EvidenceError, OSError, KeyError) as error:
        print(f"EVIDENCE_GATE_FAIL: {error}", file=sys.stderr)
        return 1
    print(f"EVIDENCE_INTEGRITY_PASS inventory={result['inventory']} ledger={result['ledger']} linked={result['linked']} "
          f"unassessed={result['unassessed']} not_run={result['not_run']} failed_layers={result['failed_layers']}")
    if args.require_complete and result["incomplete"]:
        print(f"EVIDENCE_COVERAGE_INCOMPLETE identities={result['incomplete']}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
