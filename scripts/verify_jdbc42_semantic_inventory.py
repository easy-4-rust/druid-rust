#!/usr/bin/env python3
"""Verify the source-bound JDBC 4.2 semantic inventory for P04 Task 2."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

from generate_jdbc42_semantic_inventory import (
    ARCHIVE_SHA256,
    ARCHIVE_SIZE,
    ARCHIVE_URL,
    ORACLE_COMMIT,
    ORACLE_TAG,
    SOURCE_MANIFEST_SHA256,
)


SEMANTIC_HEADER = (
    "item_key\tjava_fqcn\tjava_member\trust_shape\tsemantic_disposition\t"
    "canonical_rust_entry\ttarget_state\tnormalized_signature\tcall_kind\t"
    "index_null_adapter\terror_adapter\tlifecycle_owner\tadapter_implementation\t"
    "adapter_state\tlogical_implementation\tlogical_state\tevidence_level\t"
    "capability_state\tnotes"
)
DEFAULT_SEMANTIC = Path("docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv")
DEFAULT_ORACLE = Path("docs/verification/rdbc-first/jdbc42-oracle.json")
RUST_SHAPES = {"TRAIT", "DTO", "ENUM_KIND", "MODULE", "CALLBACK", "EXCLUDED_RUNTIME"}
SEMANTIC_DISPOSITIONS = {
    "MATCH", "SQL_FEATURE_UNSUPPORTED_0A000", "JAVA_DEFAULT_UNSUPPORTED_TYPED",
    "JAVA_RUNTIME_ONLY",
}
ADAPTER_STATES = {"PLANNED", "NOT_APPLICABLE"}
LOGICAL_STATES = {"PLANNED", "NOT_APPLICABLE"}
CAPABILITY_STATES = {"PLANNED", "NOT_APPLICABLE"}


class VerificationError(RuntimeError):
    """Raised when the semantic or source oracle manifest is inconsistent."""


@dataclass(frozen=True)
class InventorySummary:
    """Counts proven by one successful verification run."""

    java_sql: int
    javax_sql: int
    unknown: int
    semantic_sha256: str


def fail(message: str) -> NoReturn:
    """Stop verification with a stable diagnostic."""

    raise VerificationError(message)


def resolve_path(root: Path, path: Path) -> Path:
    """Resolve one CLI path relative to the repository root."""

    return path if path.is_absolute() else root / path


def require_sha256(value: object, label: str) -> str:
    """Validate a lowercase hexadecimal SHA-256 string."""

    if not isinstance(value, str) or len(value) != 64 or any(ch not in "0123456789abcdef" for ch in value):
        fail(f"{label} is not a lowercase SHA-256")
    return value


def load_oracle(
    path: Path,
    expected_source_manifest_sha256: str,
) -> tuple[dict[str, dict[str, object]], set[str]]:
    """Load and validate the fixed OpenJDK oracle plus its source-derived object rows."""

    if not path.is_file():
        fail(f"JDBC oracle not found: {path}")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"cannot read JDBC oracle: {error}")
    expected_scalars = {
        "schema_version": 1,
        "oracle_tag": ORACLE_TAG,
        "oracle_commit": ORACLE_COMMIT,
        "archive_url": ARCHIVE_URL,
        "archive_sha256": ARCHIVE_SHA256,
        "archive_size": ARCHIVE_SIZE,
    }
    for key, expected in expected_scalars.items():
        if payload.get(key) != expected:
            fail(f"JDBC oracle drift for {key}: {payload.get(key)!r}")
    semantic_sha = require_sha256(payload.get("semantic_sha256"), "semantic_sha256")
    recorded_source_manifest = require_sha256(
        payload.get("source_manifest_sha256"),
        "source_manifest_sha256",
    )
    if recorded_source_manifest != expected_source_manifest_sha256:
        fail("JDBC oracle source manifest digest drift")
    packages = payload.get("packages")
    if not isinstance(packages, dict) or set(packages) != {"java.sql", "javax.sql"}:
        fail("JDBC oracle packages must be exactly java.sql and javax.sql")
    expected_counts = {"java.sql": 52, "javax.sql": 17}
    records: dict[str, dict[str, object]] = {}
    for package, expected_count in expected_counts.items():
        package_record = packages.get(package)
        if not isinstance(package_record, dict):
            fail(f"JDBC oracle package is not an object: {package}")
        objects = package_record.get("objects")
        if package_record.get("object_count") != expected_count or not isinstance(objects, list) or len(objects) != expected_count:
            fail(f"JDBC oracle {package} count drift")
        for index, record in enumerate(objects):
            if not isinstance(record, dict):
                fail(f"JDBC oracle {package} object {index} is invalid")
            required = {
                "fqcn", "object_name", "declaration_kind", "normalized_signature",
                "source_path", "source_sha256",
            }
            if set(record) != required:
                fail(f"JDBC oracle {package} object {index} has schema drift")
            fqcn = record["fqcn"]
            name = record["object_name"]
            if not isinstance(fqcn, str) or not fqcn.startswith(package + "."):
                fail(f"JDBC oracle object has wrong package: {fqcn!r}")
            if not isinstance(name, str) or fqcn != f"{package}.{name}":
                fail(f"JDBC oracle object name drift: {fqcn!r}")
            if fqcn in records:
                fail(f"JDBC oracle duplicates {fqcn}")
            if record["declaration_kind"] not in {"class", "interface", "enum"}:
                fail(f"JDBC oracle has unknown declaration kind: {fqcn}")
            signature = record["normalized_signature"]
            if not isinstance(signature, str) or not signature or name not in signature:
                fail(f"JDBC oracle has invalid normalized signature: {fqcn}")
            source_path = record["source_path"]
            expected_suffix = f"jdk/src/share/classes/{package.replace('.', '/')}/{name}.java"
            if not isinstance(source_path, str) or source_path != expected_suffix:
                fail(f"JDBC oracle source path drift: {fqcn}")
            require_sha256(record["source_sha256"], f"source_sha256 for {fqcn}")
            records[fqcn] = record
    canonical_objects = [
        record
        for package in ("java.sql", "javax.sql")
        for record in packages[package]["objects"]
    ]
    computed_source_manifest = hashlib.sha256(
        (json.dumps(canonical_objects, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    ).hexdigest()
    if computed_source_manifest != recorded_source_manifest:
        fail("JDBC oracle source records do not match source_manifest_sha256")
    return records, {semantic_sha}


def verify_inventory(
    root: Path,
    semantic_path: Path,
    oracle_path: Path,
    *,
    expected_source_manifest_sha256: str = SOURCE_MANIFEST_SHA256,
) -> InventorySummary:
    """Verify exact source/semantic coverage and Task 2 planned-state honesty."""

    semantic = resolve_path(root, semantic_path)
    oracle = resolve_path(root, oracle_path)
    records, expected_semantic_hashes = load_oracle(
        oracle,
        expected_source_manifest_sha256,
    )
    if not semantic.is_file():
        fail(f"JDBC semantic inventory not found: {semantic}")
    payload = semantic.read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    if digest not in expected_semantic_hashes:
        fail("semantic inventory bytes do not match the source oracle digest")
    try:
        lines = payload.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        fail(f"semantic inventory is not UTF-8: {error}")
    if not lines or lines[0] != SEMANTIC_HEADER:
        fail("semantic inventory header drift")
    seen_keys: set[str] = set()
    seen_fqcns: set[str] = set()
    counts = {"java.sql": 0, "javax.sql": 0}
    for line_number, line in enumerate(lines[1:], start=2):
        fields = line.split("\t")
        if len(fields) != 19:
            fail(f"semantic row {line_number} must have 19 fields")
        (
            item_key, fqcn, java_member, rust_shape, disposition, canonical,
            target_state, signature, call_kind, index_null_adapter, error_adapter,
            owner, adapter_implementation, adapter_state, logical_implementation,
            logical_state, evidence_level, capability_state, notes,
        ) = fields
        if any(not field for field in fields):
            fail(f"semantic row {line_number} contains an empty field")
        if item_key in seen_keys:
            fail(f"semantic row {line_number} duplicates item key {item_key}")
        if fqcn in seen_fqcns:
            fail(f"semantic row {line_number} duplicates Java object {fqcn}")
        seen_keys.add(item_key)
        seen_fqcns.add(fqcn)
        record = records.get(fqcn)
        if record is None:
            fail(f"semantic row {line_number} is not present in the source oracle: {fqcn}")
        if item_key != f"java:{fqcn}#TYPE":
            fail(f"semantic row {line_number} has an invalid item key")
        if java_member != record["object_name"] or signature != record["normalized_signature"]:
            fail(f"semantic row {line_number} drifts from source declaration {fqcn}")
        package = "javax.sql" if fqcn.startswith("javax.sql.") else "java.sql"
        counts[package] += 1
        if rust_shape not in RUST_SHAPES:
            fail(f"semantic row {line_number} has unknown rust_shape {rust_shape}")
        if disposition not in SEMANTIC_DISPOSITIONS:
            fail(f"semantic row {line_number} has unknown disposition {disposition}")
        if target_state != "PLANNED_TARGET":
            fail(f"semantic row {line_number} must remain PLANNED_TARGET in Task 2")
        if canonical == "-":
            fail(f"semantic row {line_number} has no canonical semantic target")
        if rust_shape == "EXCLUDED_RUNTIME":
            if disposition != "JAVA_RUNTIME_ONLY" or not canonical.startswith("host::"):
                fail(f"semantic row {line_number} has an invalid runtime exclusion")
        elif not canonical.startswith("druid_rdbc::"):
            fail(f"semantic row {line_number} has an invalid canonical Rust target")
        if adapter_state not in ADAPTER_STATES or logical_state not in LOGICAL_STATES:
            fail(f"semantic row {line_number} has unknown implementation state")
        if adapter_implementation != "-" or logical_implementation != "-":
            fail(f"semantic row {line_number} claims a fake Task 2 implementation path")
        if evidence_level != "V0_STATIC":
            fail(f"semantic row {line_number} overstates evidence {evidence_level}")
        if capability_state not in CAPABILITY_STATES:
            fail(f"semantic row {line_number} has unknown capability state")
        if not call_kind.endswith("_DECLARATION"):
            fail(f"semantic row {line_number} has invalid call_kind")
        if index_null_adapter == "UNKNOWN" or error_adapter == "UNKNOWN" or owner == "UNKNOWN" or notes == "UNKNOWN":
            fail(f"semantic row {line_number} contains UNKNOWN")
    if seen_fqcns != set(records):
        missing = sorted(set(records) - seen_fqcns)
        extra = sorted(seen_fqcns - set(records))
        fail(f"semantic source coverage mismatch: missing={missing} extra={extra}")
    if (counts["java.sql"], counts["javax.sql"]) != (52, 17):
        fail(
            "semantic package count drift: "
            f"JAVA_SQL={counts['java.sql']} JAVAX_SQL={counts['javax.sql']}"
        )
    return InventorySummary(counts["java.sql"], counts["javax.sql"], 0, digest)


def repository_root() -> Path:
    """Return the repository root independently of the command's CWD."""

    return Path(__file__).resolve().parent.parent


def parse_arguments() -> argparse.Namespace:
    """Parse optional manifest paths for self-tests and the default live gate."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semantic", type=Path, default=DEFAULT_SEMANTIC)
    parser.add_argument("--oracle", type=Path, default=DEFAULT_ORACLE)
    return parser.parse_args()


def main() -> int:
    """Run the live semantic gate."""

    arguments = parse_arguments()
    try:
        summary = verify_inventory(repository_root(), arguments.semantic, arguments.oracle)
    except VerificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"JAVA_SQL={summary.java_sql}")
    print(f"JAVAX_SQL={summary.javax_sql}")
    print(f"UNKNOWN={summary.unknown}")
    print(f"SEMANTIC_SHA256={summary.semantic_sha256}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
