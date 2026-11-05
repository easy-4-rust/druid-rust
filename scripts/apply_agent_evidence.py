#!/usr/bin/env python3
"""Apply agent-audited evidence candidates to the JDBC evidence ledger.

Dry-run by default. Use --apply to write the ledger.
Modes:
  method <tsv>  columns: identity, rust_type_entry, rust_method_entry,
                contract_evidence, logical_proxy_evidence, adapter_evidence, notes
  layer <tsv>   columns: identity, layer, evidence, note
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "docs/verification/rdbc-first/java8-method-evidence.tsv"
SPEC = "rdbc-jdbc-method-conformance/spec.md#MC-08"


def digest(relative: str) -> str:
    return hashlib.sha256((ROOT / relative).read_bytes()).hexdigest().upper()


def evidence_ok(reference: str) -> tuple[bool, str]:
    if not reference or reference in {"NONE", "-", "n/a"}:
        return False, "empty"
    file_name, separator, symbol = reference.partition("#")
    path = ROOT / file_name
    if not path.is_file():
        return False, f"missing file {file_name}"
    if not separator:
        return False, "missing #symbol"
    text = path.read_text(encoding="utf-8", errors="replace")
    if not re.search(rf"\b(?:async\s+)?fn\s+{re.escape(symbol)}\s*\(", text):
        return False, f"missing fn {symbol}"
    return True, ""


def find_rust_source(method: str) -> str:
    for base in ("crates/druid-rdbc/src", "crates/druid/src"):
        candidates = []
        for path in (ROOT / base).rglob("*.rs"):
            text = path.read_text(encoding="utf-8", errors="replace")
            if re.search(rf"\bfn\s+{re.escape(method)}\s*(?:<|\()", text):
                candidates.append(path.relative_to(ROOT).as_posix())
        if candidates:
            return sorted(candidates, key=lambda item: (len(item.split("/")), item))[0]
    return ""


def load_rows() -> tuple[str, list[dict[str, str]], list[str]]:
    raw = LEDGER.read_text(encoding="utf-8").splitlines()
    rows = list(csv.DictReader(raw[1:], delimiter="\t"))
    return raw[0], rows, list(rows[0].keys())


def write_rows(header: str, rows: list[dict[str, str]], fields: list[str]) -> None:
    output = io.StringIO()
    output.write(header + "\n")
    writer = csv.DictWriter(output, fieldnames=fields, delimiter="\t", lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    LEDGER.write_text(output.getvalue(), encoding="utf-8", newline="\n")


def select_evidence(value: str, identity: str, column: str, errors: list[str]) -> str:
    """Pick the first valid reference from a semicolon-separated evidence cell."""
    tokens = [token.strip() for token in value.split(";") if token.strip()]
    selected = ""
    for token in tokens:
        if token in {"NONE", "-", "n/a"}:
            continue
        ok, reason = evidence_ok(token)
        if not ok:
            errors.append(f"{identity}: {column}: {token}: {reason}")
            continue
        if not selected:
            selected = token
    return selected


def set_evidence(row: dict[str, str], evidence: str, layer: str) -> None:
    row[layer] = "PASS"
    if not row["positive_evidence"]:
        row["positive_evidence"] = evidence
        row["positive_evidence_sha256"] = digest(evidence.partition("#")[0])


def apply_method(report: Path, apply: bool) -> int:
    header, rows, fields = load_rows()
    by_id = {row["identity"]: row for row in rows}
    updated = 0
    skipped = 0
    errors: list[str] = []
    with report.open(encoding="utf-8", newline="") as source:
        for candidate in csv.DictReader(source, delimiter="\t"):
            identity = (candidate.get("identity") or "").strip()
            if not identity:
                continue
            row = by_id.get(identity)
            if row is None:
                errors.append(f"unknown identity: {identity}")
                continue
            if row["capability"] != "UNASSESSED":
                skipped += 1
                continue
            contract = select_evidence(
                candidate.get("contract_evidence") or "", identity, "contract_evidence", errors
            )
            logical = select_evidence(
                candidate.get("logical_proxy_evidence") or "", identity,
                "logical_proxy_evidence", errors,
            )
            adapter = select_evidence(
                candidate.get("adapter_evidence") or "", identity, "adapter_evidence", errors
            )
            if not any((contract, logical, adapter)):
                continue
            rust_method = (candidate.get("rust_method_entry") or "").strip()
            method_name = rust_method.rsplit("::", 1)[-1] if rust_method else ""
            declared_source = (candidate.get("rust_source") or "").strip()
            if declared_source:
                declared_path = ROOT / declared_source
                if method_name.startswith("const:"):
                    name = method_name.partition(":")[2]
                    text = declared_path.read_text(encoding="utf-8", errors="replace") if declared_path.is_file() else ""
                    declared = re.search(rf"\bconst\s+{re.escape(name)}\b", text) or re.search(
                        rf"\b{re.escape(name)}\s*,", text) or re.search(
                        rf"\b{re.escape(name)}\s*\}}", text)
                else:
                    text = declared_path.read_text(encoding="utf-8", errors="replace") if declared_path.is_file() else ""
                    declared = re.search(rf"\bfn\s+{re.escape(method_name)}\s*(?:<|\()", text)
                if declared:
                    source = declared_source
                else:
                    errors.append(
                        f"{identity}: declared rust_source lacks {method_name}: {declared_source}"
                    )
                    continue
            else:
                source = find_rust_source(method_name) if method_name else ""
            if not source:
                errors.append(f"{identity}: rust method {method_name} not found in druid-rdbc")
                continue
            row["rust_type_entry"] = (candidate.get("rust_type_entry") or "").strip()
            row["rust_method_entry"] = rust_method
            row["spec_scenario"] = SPEC
            row["batch"] = "jdbc-agent-wave-20261102"
            row["declaration"] = "PASS"
            row["contract"] = "PASS" if contract not in ("", "NONE", "-") else "UNASSESSED"
            real_db = select_evidence(
                candidate.get("real_db_evidence") or "", identity, "real_db_evidence", errors
            )
            mode = (candidate.get("applicability") or "").strip()
            constant = mode == "constant"
            unsupported = mode == "unsupported"
            if unsupported:
                row["adapter"] = "UNASSESSED"
                row["logical_proxy"] = "UNASSESSED"
                row["real_db"] = "UNASSESSED"
                row["capability"] = "UNSUPPORTED"
                row["positive_evidence"] = ""
                row["positive_evidence_sha256"] = ""
                row["unsupported_evidence"] = contract
                row["unsupported_evidence_sha256"] = digest(contract.partition("#")[0])
                row["unsupported_result"] = (candidate.get("unsupported_result") or "").strip()
                row["not_run_reason"] = ""
            elif constant:
                row["adapter"] = "NOT_APPLICABLE"
                row["logical_proxy"] = "NOT_APPLICABLE"
                row["real_db"] = "NOT_APPLICABLE"
                row["capability"] = "SUPPORTED"
            else:
                row["adapter"] = "PASS" if adapter not in ("", "NONE", "-") else "UNASSESSED"
                row["logical_proxy"] = "PASS" if logical not in ("", "NONE", "-") else "UNASSESSED"
                row["real_db"] = "PASS" if real_db else "UNASSESSED"
                row["capability"] = "PARTIAL"
            row["rust_source"] = source
            row["rust_source_sha256"] = digest(source)
            note = (candidate.get("notes") or "").strip()
            if real_db:
                note = f"{note} real_db covered by {real_db}.".strip()
            row["backend"] = (
                "Rust canonical interface; existing parity tests; real H2 XA via JDBC Agent"
                if real_db else "Rust canonical interface; existing parity tests"
            )
            missing = [layer for layer in ("contract", "adapter", "logical_proxy", "real_db") if row[layer] != "PASS"]
            row["not_run_reason"] = (
                f"{', '.join(missing)} remain unassessed; {note}" if note else
                f"{', '.join(missing)} remain unassessed."
            )
            if contract not in ("", "NONE", "-"):
                set_evidence(row, contract, "contract")
            if adapter not in ("", "NONE", "-"):
                set_evidence(row, adapter, "adapter")
            if logical not in ("", "NONE", "-"):
                set_evidence(row, logical, "logical_proxy")
            updated += 1
    print(f"method candidates updated={updated} skipped_already_linked={skipped} errors={len(errors)}")
    for error in errors[:40]:
        print("  ERROR", error)
    if apply and updated:
        write_rows(header, rows, fields)
        print("ledger written")
    return 0 if not errors else 1


def apply_layer(report: Path, apply: bool) -> int:
    header, rows, fields = load_rows()
    by_id = {row["identity"]: row for row in rows}
    updated = 0
    skipped = 0
    errors: list[str] = []
    with report.open(encoding="utf-8", newline="") as source:
        for candidate in csv.DictReader(source, delimiter="\t"):
            identity = (candidate.get("identity") or "").strip()
            layer = (candidate.get("layer") or "").strip()
            evidence = (candidate.get("evidence") or "").strip()
            if not identity or layer not in {"adapter", "logical_proxy"}:
                continue
            row = by_id.get(identity)
            if row is None:
                errors.append(f"unknown identity: {identity}")
                continue
            if row["capability"] == "UNASSESSED":
                skipped += 1
                continue
            selected = select_evidence(evidence, identity, layer, errors)
            if not selected:
                continue
            evidence = selected
            row[layer] = "PASS"
            row["capability"] = "PARTIAL"
            if not row["positive_evidence"]:
                row["positive_evidence"] = evidence
                row["positive_evidence_sha256"] = digest(evidence.partition("#")[0])
            note = (candidate.get("note") or "").strip()
            row["not_run_reason"] = (
                f"{note}; remaining layers without PASS stay unassessed." if note else
                "Remaining layers without PASS stay unassessed."
            )
            updated += 1
    print(f"layer candidates updated={updated} skipped_unassessed={skipped} errors={len(errors)}")
    for error in errors[:40]:
        print("  ERROR", error)
    if apply and updated:
        write_rows(header, rows, fields)
        print("ledger written")
    return 0 if not errors else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["method", "layer"])
    parser.add_argument("report", type=Path)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    if args.mode == "method":
        return apply_method(args.report, args.apply)
    return apply_layer(args.report, args.apply)


if __name__ == "__main__":
    raise SystemExit(main())
