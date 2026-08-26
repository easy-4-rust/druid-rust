#!/usr/bin/env python3
"""Render/check the exact 105-row Task 4 BASE-to-Task 6 compiler identity map."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import subprocess
from pathlib import Path
from typing import NoReturn


TASK6_BASE = "5afe8609b177544ff7d18461aa84f69efe8ee90e"
MANIFEST = Path("docs/verification/rdbc-first/rdbc-adapter-coverage-regions.json")
OUTPUT = Path("docs/verification/rdbc-first/task6-compiler-identity-migration.tsv")
HEADER = (
    "family",
    "requirement_id",
    "requirement_kind",
    "base_test_target",
    "base_test_id",
    "base_profile",
    "base_trace_version",
    "base_coverage_version",
    "base_source_path",
    "base_source_sha256",
    "base_source_function",
    "base_compiler_function_name",
    "base_anchor_id",
    "base_minimum_count",
    "task6_test_target",
    "task6_test_id",
    "task6_profile",
    "task6_trace_version",
    "task6_coverage_version",
    "task6_source_path",
    "task6_source_sha256",
    "task6_source_function",
    "task6_compiler_function_name",
    "task6_anchor_id",
    "task6_minimum_count",
    "change",
)


class VerificationError(RuntimeError):
    """Raised when either compiler manifest or the checked migration map drifts."""


def fail(message: str) -> NoReturn:
    """Stop with a deterministic renderer failure."""

    raise VerificationError(message)


def load_manifest(payload: bytes, label: str) -> dict[str, object]:
    """Load the canonical Task 4 compiler manifest schema."""

    if not payload.endswith(b"\n") or payload.endswith(b"\n\n") or b"\r" in payload:
        fail(f"{label} manifest must have exactly one LF final newline")
    try:
        document = json.loads(payload)
    except json.JSONDecodeError as error:
        fail(f"{label} manifest is malformed: {error}")
    if not isinstance(document, dict) or set(document) != {
        "schema_version",
        "families",
    }:
        fail(f"{label} manifest root schema is not exact")
    if document["schema_version"] != "task4-compiler-coverage-v2":
        fail(f"{label} compiler coverage protocol is not v2")
    canonical = (
        json.dumps(document, ensure_ascii=False, indent=2).encode("utf-8") + b"\n"
    )
    if payload != canonical:
        fail(f"{label} manifest JSON is not canonical")
    return document


def flatten(document: dict[str, object], label: str) -> tuple[dict[str, str], ...]:
    """Flatten ordered family/requirement records without discarding binding fields."""

    families = document.get("families")
    if not isinstance(families, list) or len(families) != 7:
        fail(f"{label} manifest must contain seven families")
    result: list[dict[str, str]] = []
    seen: set[tuple[str, str]] = set()
    for family in families:
        if not isinstance(family, dict):
            fail(f"{label} family record is malformed")
        family_name = family.get("family")
        requirements = family.get("requirements")
        if not isinstance(family_name, str) or not isinstance(requirements, list):
            fail(f"{label} family binding is malformed")
        if len(requirements) != 15:
            fail(f"{label} family {family_name} must contain fifteen requirements")
        for requirement in requirements:
            if not isinstance(requirement, dict):
                fail(f"{label} requirement is malformed")
            key = (family_name, str(requirement.get("requirement_id", "")))
            if not key[1] or key in seen:
                fail(f"{label} requirement identity is duplicate or empty: {key}")
            seen.add(key)
            row = {
                "family": family_name,
                "requirement_id": key[1],
                "requirement_kind": str(requirement.get("requirement_kind", "")),
                "test_target": str(family.get("test_target", "")),
                "test_id": str(family.get("test_id", "")),
                "profile": str(family.get("profile", "")),
                "trace_version": str(family.get("trace_version", "")),
                "coverage_version": str(family.get("coverage_version", "")),
                "source_path": str(requirement.get("source_path", "")),
                "source_sha256": str(requirement.get("source_sha256", "")),
                "source_function": str(requirement.get("source_function", "")),
                "compiler_function_name": str(
                    requirement.get("compiler_function_name", "")
                ),
                "anchor_id": str(requirement.get("anchor_id", "")),
                "minimum_count": str(requirement.get("minimum_count", "")),
            }
            if any(not value for value in row.values()):
                fail(f"{label} requirement binding is incomplete: {key}")
            result.append(row)
    if len(result) != 105:
        fail(f"{label} flattened denominator must be 105, found {len(result)}")
    return tuple(result)


def migration_rows(
    base: tuple[dict[str, str], ...], current: tuple[dict[str, str], ...]
) -> tuple[dict[str, str], ...]:
    """Join the two ordered manifests by exact family/requirement identity."""

    base_keys = [(row["family"], row["requirement_id"]) for row in base]
    current_keys = [(row["family"], row["requirement_id"]) for row in current]
    if base_keys != current_keys:
        fail("BASE and Task 6 family/requirement order or denominator differs")
    rows: list[dict[str, str]] = []
    binding_fields = (
        "test_target",
        "test_id",
        "profile",
        "trace_version",
        "coverage_version",
        "source_path",
        "source_sha256",
        "source_function",
        "compiler_function_name",
        "anchor_id",
        "minimum_count",
    )
    for old, new in zip(base, current, strict=True):
        row = {
            "family": old["family"],
            "requirement_id": old["requirement_id"],
            "requirement_kind": old["requirement_kind"],
        }
        if old["requirement_kind"] != new["requirement_kind"]:
            fail(f"requirement kind drift: {old['family']}/{old['requirement_id']}")
        for field in binding_fields:
            row[f"base_{field}"] = old[field]
            row[f"task6_{field}"] = new[field]
        row["change"] = (
            "UNCHANGED"
            if all(old[field] == new[field] for field in binding_fields)
            else "MIGRATED"
        )
        rows.append({column: row[column] for column in HEADER})
    return tuple(rows)


def render_rows(rows: tuple[dict[str, str], ...]) -> bytes:
    """Serialize the exact map as canonical LF TSV."""

    output = io.StringIO(newline="")
    writer = csv.DictWriter(
        output, fieldnames=HEADER, delimiter="\t", lineterminator="\n"
    )
    writer.writeheader()
    for row in rows:
        if tuple(row) != HEADER:
            fail("compiler identity migration row schema is not exact")
        if any(
            "\t" in value or "\n" in value or "\r" in value for value in row.values()
        ):
            fail("compiler identity migration value contains a TSV delimiter/control")
        writer.writerow(row)
    return output.getvalue().encode("utf-8")


def git_base_manifest(root: Path) -> bytes:
    """Read the immutable BASE compiler manifest without switching the checkout."""

    result = subprocess.run(
        ["git", "show", f"{TASK6_BASE}:{MANIFEST.as_posix()}"],
        cwd=root,
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        fail(f"cannot read BASE compiler manifest: {result.stderr.decode().strip()}")
    return result.stdout


def expected_map(root: Path) -> bytes:
    """Render the reproducible BASE-to-current map."""

    base = flatten(load_manifest(git_base_manifest(root), "BASE"), "BASE")
    current_payload = (root / MANIFEST).read_bytes()
    current = flatten(load_manifest(current_payload, "Task 6"), "Task 6")
    return render_rows(migration_rows(base, current))


def main() -> int:
    """Write the map on request; otherwise verify bytes and print its digest."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    parser.add_argument("--write", action="store_true")
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    try:
        expected = expected_map(root)
        output = root / OUTPUT
        if arguments.write:
            output.write_bytes(expected)
        elif not output.is_file() or output.read_bytes() != expected:
            fail("checked compiler identity migration map is missing or stale")
    except (OSError, UnicodeError, VerificationError) as error:
        print(f"TASK6_COMPILER_IDENTITY_MAP_FAIL={error}")
        return 1
    digest = hashlib.sha256(expected).hexdigest()
    print("TASK6_COMPILER_IDENTITY_MAP_ROWS=105")
    print("TASK6_COMPILER_IDENTITY_MAP_PROTOCOL=task4-compiler-coverage-v2")
    print(f"TASK6_COMPILER_IDENTITY_MAP_SHA256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
