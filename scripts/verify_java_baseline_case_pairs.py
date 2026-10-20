"""Verify exact identities and paired evidence for the frozen 59-case Java selection.

This is one P07 baseline selection, not the whole JDBC or production-readiness denominator.
The default command fails while any selected case is unpaired or mismatched.
"""
from __future__ import annotations

import argparse
from collections import Counter
import csv
import hashlib
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
DIRECTORY = Path("docs/verification/rdbc-first")
MANIFEST = DIRECTORY / "java-baseline-59-cases.tsv"
HEADER = ("java_class", "java_method", "java_source", "java_source_sha256", "status", "comparison")
IDENTITY_SHA = "d5390a92b09e956c89698631263391baa1ab78c02a90454964da1d3186d2144b"
STATUSES = {"NOT_PAIRED", "MATCH", "APPROVED_STANDARD_DIFFERENCE", "MISMATCH"}


def records(path: Path) -> tuple[tuple[str, ...], list[dict[str, str]]]:
    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        return tuple(reader.fieldnames or ()), list(reader)


def verify(root: Path = ROOT, java_root: Path | None = None) -> Counter[str]:
    header, rows = records(root / MANIFEST)
    if header != HEADER or len(rows) != 59:
        raise ValueError("frozen selection requires its exact header and 59 cases")
    if any(set(row) != set(HEADER) or any(value is None for value in row.values()) for row in rows):
        raise ValueError("malformed case record")
    identities = "".join("\t".join(row[key] for key in HEADER[:4]) + "\n" for row in rows)
    if hashlib.sha256(identities.encode()).hexdigest() != IDENTITY_SHA:
        raise ValueError("frozen Java case identities/source hashes changed")
    comparisons: dict[str, dict[str, str]] = {}
    for row in rows:
        status = row["status"]
        if status not in STATUSES:
            raise ValueError("unknown evidence status")
        if java_root is not None:
            source = java_root / row["java_source"]
            if hashlib.sha256(source.read_bytes()).hexdigest() != row["java_source_sha256"]:
                raise ValueError(f"frozen Java source drift: {source}")
            methods = re.findall(r"public\s+void\s+(test\w*)\s*\(\s*\)", source.read_text(encoding="utf-8"))
            if row["java_method"] not in methods:
                raise ValueError("selected method is absent from pinned source")
        evidence = row["comparison"]
        if status == "NOT_PAIRED":
            if evidence != "-":
                raise ValueError("unpaired case cannot claim a comparison")
            continue
        relative = Path(evidence)
        if relative.is_absolute() or ".." in relative.parts or relative.suffix != ".tsv":
            raise ValueError("comparison must be a confined TSV path")
        if evidence not in comparisons:
            fields, observed = records(root / DIRECTORY / relative)
            if "case_id" not in fields or "status" not in fields:
                raise ValueError("comparison lacks case identity/status")
            indexed = {item["case_id"]: item["status"] for item in observed}
            if len(indexed) != len(observed):
                raise ValueError("duplicate comparison case identity")
            comparisons[evidence] = indexed
        name = row["java_class"].rsplit(".", 1)[-1]
        case_id = (name + "." + row["java_method"] if name == "DruidPooledResultSetTest" or name.startswith("DruidDataSourceTest_") else row["java_method"])
        if comparisons[evidence].get(case_id) != status:
            raise ValueError(f"case status differs from actual paired output: {name}.{row['java_method']}")
    return Counter(row["status"] for row in rows)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java-root", type=Path)
    parser.add_argument("--inventory-only", action="store_true", help="check record integrity only; does not certify parity")
    args = parser.parse_args()
    counts = verify(java_root=args.java_root)
    incomplete = counts["NOT_PAIRED"] + counts["MISMATCH"]
    print(f"Frozen Java selection: 59; {dict(sorted(counts.items()))}")
    print("Selection verdict: " + ("NOT_READY" if incomplete else "PASS"))
    return 0 if args.inventory_only or not incomplete else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError) as error:
        raise SystemExit(f"Case-pair verification failed: {error}") from error
