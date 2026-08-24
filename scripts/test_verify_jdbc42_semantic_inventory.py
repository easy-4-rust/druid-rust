#!/usr/bin/env python3
"""Fail-closed self-tests for the JDBC 4.2 semantic inventory verifier."""

from __future__ import annotations

import hashlib
import json
import sys
import tarfile
import tempfile
import unittest
from io import BytesIO
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

from generate_jdbc42_semantic_inventory import discover_java_records  # noqa: E402
from verify_jdbc42_semantic_inventory import (  # noqa: E402
    ARCHIVE_SHA256,
    ARCHIVE_SIZE,
    ARCHIVE_URL,
    ORACLE_COMMIT,
    ORACLE_TAG,
    SEMANTIC_HEADER,
    VerificationError,
    verify_inventory,
)


def semantic_row(fqcn: str) -> list[str]:
    """Build one independently checked planned semantic row."""

    name = fqcn.rsplit(".", 1)[1]
    return [
        f"java:{fqcn}#TYPE",
        fqcn,
        name,
        "TRAIT",
        "MATCH",
        f"druid_rdbc::planned::{name}",
        "PLANNED_TARGET",
        f"public interface {name}",
        "TYPE_DECLARATION",
        "STANDARD_1_BASED_AND_SQL_NULL",
        "RDBC_ERROR",
        "STANDARD_OWNER",
        "-",
        "PLANNED",
        "-",
        "NOT_APPLICABLE",
        "V0_STATIC",
        "PLANNED",
        "synthetic verifier fixture",
    ]


def write_fixture(root: Path) -> tuple[Path, Path]:
    """Write a complete 52 + 17 synthetic oracle and matching semantic TSV."""

    fqcns = [f"java.sql.Type{index:02d}" for index in range(52)]
    fqcns.extend(f"javax.sql.Type{index:02d}" for index in range(17))
    rows = [semantic_row(fqcn) for fqcn in fqcns]
    semantic = root / "semantic.tsv"
    semantic.write_text(
        SEMANTIC_HEADER + "\n" + "\n".join("\t".join(row) for row in rows) + "\n",
        encoding="utf-8",
    )
    objects = []
    for fqcn in fqcns:
        name = fqcn.rsplit(".", 1)[1]
        package_path = fqcn.rsplit(".", 1)[0].replace(".", "/")
        objects.append(
            {
                "fqcn": fqcn,
                "object_name": name,
                "declaration_kind": "interface",
                "normalized_signature": f"public interface {name}",
                "source_path": f"jdk/src/share/classes/{package_path}/{name}.java",
                "source_sha256": hashlib.sha256(fqcn.encode("utf-8")).hexdigest(),
            }
        )
    source_manifest_sha256 = hashlib.sha256(
        (json.dumps(objects, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    ).hexdigest()
    oracle = root / "oracle.json"
    oracle.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "oracle_tag": ORACLE_TAG,
                "oracle_commit": ORACLE_COMMIT,
                "archive_url": ARCHIVE_URL,
                "archive_sha256": ARCHIVE_SHA256,
                "archive_size": ARCHIVE_SIZE,
                "source_manifest_sha256": source_manifest_sha256,
                "packages": {
                    "java.sql": {
                        "object_count": 52,
                        "objects": [item for item in objects if item["fqcn"].startswith("java.sql.")],
                    },
                    "javax.sql": {
                        "object_count": 17,
                        "objects": [item for item in objects if item["fqcn"].startswith("javax.sql.")],
                    },
                },
                "semantic_sha256": hashlib.sha256(semantic.read_bytes()).hexdigest(),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return semantic, oracle


class Jdbc42SemanticInventoryVerifierTest(unittest.TestCase):
    """Pressure-test the verifier with controlled malformed manifests."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.semantic, self.oracle = write_fixture(self.root)
        self.source_manifest_sha256 = json.loads(
            self.oracle.read_text(encoding="utf-8")
        )["source_manifest_sha256"]

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def rows(self) -> list[list[str]]:
        """Return mutable semantic rows without the header."""

        return [line.split("\t") for line in self.semantic.read_text().splitlines()[1:]]

    def write_rows(self, rows: list[list[str]]) -> None:
        """Replace the semantic fixture and keep its recorded digest synchronized."""

        self.semantic.write_text(
            SEMANTIC_HEADER + "\n" + "\n".join("\t".join(row) for row in rows) + "\n",
            encoding="utf-8",
        )
        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        payload["semantic_sha256"] = hashlib.sha256(self.semantic.read_bytes()).hexdigest()
        self.oracle.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    def assert_rejected(self) -> None:
        """Assert the current fixture fails closed."""

        with self.assertRaises(VerificationError):
            verify_inventory(
                self.root,
                self.semantic,
                self.oracle,
                expected_source_manifest_sha256=self.source_manifest_sha256,
            )

    def test_accepts_complete_source_bound_planned_inventory(self) -> None:
        """A complete source-bound 52 + 17 planned inventory is the positive control."""

        summary = verify_inventory(
            self.root,
            self.semantic,
            self.oracle,
            expected_source_manifest_sha256=self.source_manifest_sha256,
        )
        self.assertEqual((52, 17, 0), (summary.java_sql, summary.javax_sql, summary.unknown))

    def test_rejects_missing_and_duplicate_rows(self) -> None:
        """Removing or duplicating a source object must break the frozen denominator."""

        rows = self.rows()
        self.write_rows(rows[:-1] + [rows[0]])
        self.assert_rejected()

    def test_rejects_wrong_52_17_package_counts(self) -> None:
        """Moving one object between packages must not preserve a false total of 69."""

        rows = self.rows()
        rows[0][1] = "javax.sql.TypeMoved"
        rows[0][0] = "java:javax.sql.TypeMoved#TYPE"
        rows[0][2] = "TypeMoved"
        self.write_rows(rows)
        self.assert_rejected()

    def test_rejects_unknown_shape_state_and_empty_canonical_entry(self) -> None:
        """Unknown policy values and an empty planned target are never completion-like."""

        rows = self.rows()
        rows[0][3] = "UNKNOWN"
        rows[0][5] = "-"
        rows[0][6] = "LIVE"
        self.write_rows(rows)
        self.assert_rejected()

    def test_rejects_fake_implementation_paths(self) -> None:
        """Task 2 cannot claim a live adapter or logical implementation that is absent."""

        rows = self.rows()
        rows[0][12] = "crates/fake/src/missing.rs::FakeAdapter"
        rows[0][13] = "LIVE"
        self.write_rows(rows)
        self.assert_rejected()

    def test_rejects_oracle_hash_drift(self) -> None:
        """Changing the pinned archive digest must invalidate the oracle."""

        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        payload["archive_sha256"] = "0" * 64
        self.oracle.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        self.assert_rejected()

    def test_rejects_oracle_source_manifest_drift(self) -> None:
        """Changing one recorded OpenJDK source hash must invalidate the oracle."""

        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        payload["source_manifest_sha256"] = "0" * 64
        self.oracle.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        self.assert_rejected()

    def test_rejects_extra_and_duplicate_json_keys(self) -> None:
        """Every JSON object layer has an exact schema and duplicate keys are invalid."""

        original = self.oracle.read_text(encoding="utf-8")
        payload = json.loads(original)
        payload["extra"] = "not part of the oracle schema"
        self.oracle.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        self.assert_rejected()

        self.oracle.write_text(
            original.replace("{\n", "{\n  \"archive_size\": 94004963,\n", 1),
            encoding="utf-8",
        )
        self.assert_rejected()

    def test_rejects_noncanonical_json_serialization(self) -> None:
        """Semantically equal JSON with alternate whitespace is not canonical oracle evidence."""

        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        self.oracle.write_text(json.dumps(payload, sort_keys=True) + "\n", encoding="utf-8")
        self.assert_rejected()

    def test_rejects_noncanonical_or_control_bearing_semantic_tsv(self) -> None:
        """CRLF and embedded control data cannot become canonical semantic rows."""

        canonical = self.semantic.read_bytes()
        self.semantic.write_bytes(canonical.replace(b"\n", b"\r\n"))
        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        payload["semantic_sha256"] = hashlib.sha256(self.semantic.read_bytes()).hexdigest()
        self.oracle.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        self.assert_rejected()

        self.semantic.write_bytes(canonical.replace(b"synthetic verifier fixture", b"synthetic\x01fixture", 1))
        payload["semantic_sha256"] = hashlib.sha256(self.semantic.read_bytes()).hexdigest()
        self.oracle.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        self.assert_rejected()

        self.semantic.write_bytes(canonical.replace(b"synthetic verifier fixture", b"synthetic\x7ffixture", 1))
        payload["semantic_sha256"] = hashlib.sha256(self.semantic.read_bytes()).hexdigest()
        self.oracle.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        self.assert_rejected()

        self.semantic.write_bytes(
            canonical.replace(
                b"synthetic verifier fixture",
                "synthetic\u0085fixture".encode("utf-8"),
                1,
            )
        )
        payload["semantic_sha256"] = hashlib.sha256(self.semantic.read_bytes()).hexdigest()
        self.oracle.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        self.assert_rejected()

    def test_archive_discovery_uses_source_entries_not_a_manual_object_list(self) -> None:
        """Adding a Java source file changes the discovered denominator without code edits."""

        archive = self.root / "oracle.tar.gz"
        with tarfile.open(archive, "w:gz") as bundle:
            for fqcn, declaration in (
                ("java.sql.Alpha", "package java.sql; public interface Alpha {}"),
                ("javax.sql.Beta", "package javax.sql; public final class Beta {}"),
            ):
                member = tarfile.TarInfo(
                    "jdk8u-test/jdk/src/share/classes/"
                    + fqcn.rsplit(".", 1)[0].replace(".", "/")
                    + "/"
                    + fqcn.rsplit(".", 1)[1]
                    + ".java"
                )
                source = declaration.encode("utf-8")
                member.size = len(source)
                bundle.addfile(member, BytesIO(source))
        first = discover_java_records(archive, enforce_jdbc42_counts=False)
        self.assertEqual(["java.sql.Alpha", "javax.sql.Beta"], [row.fqcn for row in first])

        with tarfile.open(archive, "w:gz") as bundle:
            for name in ("Alpha", "Gamma"):
                source = f"package java.sql; public interface {name} {{}}".encode("utf-8")
                member = tarfile.TarInfo(
                    f"jdk8u-test/jdk/src/share/classes/java/sql/{name}.java"
                )
                member.size = len(source)
                bundle.addfile(member, BytesIO(source))
        second = discover_java_records(archive, enforce_jdbc42_counts=False)
        self.assertEqual(["java.sql.Alpha", "java.sql.Gamma"], [row.fqcn for row in second])


if __name__ == "__main__":
    unittest.main(verbosity=2)
