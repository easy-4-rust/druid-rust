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
        encoding="utf-8", newline="",
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
        (json.dumps(objects, sort_keys=True, separators=(",", ":")) + "\n").encode(
            "utf-8"
        )
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
                        "objects": [
                            item
                            for item in objects
                            if item["fqcn"].startswith("java.sql.")
                        ],
                    },
                    "javax.sql": {
                        "object_count": 17,
                        "objects": [
                            item
                            for item in objects
                            if item["fqcn"].startswith("javax.sql.")
                        ],
                    },
                },
                "semantic_sha256": hashlib.sha256(semantic.read_bytes()).hexdigest(),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8", newline="",
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
            encoding="utf-8", newline="",
        )
        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        payload["semantic_sha256"] = hashlib.sha256(
            self.semantic.read_bytes()
        ).hexdigest()
        self.oracle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )

    def write_rows_preserve_oracle(self, rows: list[list[str]]) -> None:
        """Write a later-phase fixture while retaining the frozen planned denominator hash."""

        self.semantic.write_text(
            SEMANTIC_HEADER + "\n" + "\n".join("\t".join(row) for row in rows) + "\n",
            encoding="utf-8", newline="",
        )

    def assert_rejected(self) -> None:
        """Assert the current fixture fails closed."""

        with self.assertRaises(VerificationError):
            verify_inventory(
                self.root,
                self.semantic,
                self.oracle,
                expected_source_manifest_sha256=self.source_manifest_sha256,
            )

    def test_fixture_writes_canonical_lf_bytes_on_every_platform(self) -> None:
        """Windows 夹具也必须写入 LF，避免正向控制在无关格式门禁失败。"""

        for path in (self.semantic, self.oracle):
            with self.subTest(path=path.name):
                self.assertNotIn(b"\r", path.read_bytes())

    def test_accepts_complete_source_bound_planned_inventory(self) -> None:
        """A complete source-bound 52 + 17 planned inventory is the positive control."""

        summary = verify_inventory(
            self.root,
            self.semantic,
            self.oracle,
            expected_source_manifest_sha256=self.source_manifest_sha256,
        )
        self.assertEqual(
            (52, 17, 0), (summary.java_sql, summary.javax_sql, summary.unknown)
        )

    def test_accepts_declared_target_without_live_implementation_claims(self) -> None:
        """Task 3 may declare a canonical target while adapter/logical states stay planned."""

        rows = self.rows()
        rows[0][6] = "DECLARED"
        self.write_rows(rows)
        summary = verify_inventory(
            self.root,
            self.semantic,
            self.oracle,
            expected_source_manifest_sha256=self.source_manifest_sha256,
        )
        self.assertEqual(
            (52, 17, 0), (summary.java_sql, summary.javax_sql, summary.unknown)
        )

    def test_adapter_live_phase_normalizes_to_frozen_denominator(self) -> None:
        """Task 4 may advance only its adapter evidence columns without rewriting the oracle hash."""

        rows = self.rows()
        rows[0][12] = "crates/druid-wrapper/src/live.rs::LiveAdapter"
        rows[0][13] = "LIVE"
        rows[0][16] = "V1_LIVE"
        rows[0][17] = "LIVE"
        self.write_rows_preserve_oracle(rows)
        summary = verify_inventory(
            self.root,
            self.semantic,
            self.oracle,
            expected_source_manifest_sha256=self.source_manifest_sha256,
            phase="adapter-live",
            adapter_live_entries={"java.sql.Type00"},
            require_adapter_evidence=False,
        )
        self.assertEqual(
            (52, 17, 0), (summary.java_sql, summary.javax_sql, summary.unknown)
        )

    def test_adapter_live_phase_rejects_missing_live_state_or_path(self) -> None:
        """A Task 4 row cannot be LIVE with a missing path or remain PLANNED."""

        with self.assertRaises(VerificationError):
            verify_inventory(
                self.root,
                self.semantic,
                self.oracle,
                expected_source_manifest_sha256=self.source_manifest_sha256,
                phase="adapter-live",
                adapter_live_entries={"java.sql.Type00"},
                require_adapter_evidence=False,
            )

    def test_logical_live_phase_normalizes_adapter_and_logical_columns(self) -> None:
        """Task 5 may advance only its exact logical rows over retained Task 4 evidence."""

        rows = self.rows()
        rows[0][15] = "PLANNED"
        self.write_rows(rows)
        rows = self.rows()
        rows[0][12] = "crates/druid-wrapper/src/live.rs::LiveAdapter"
        rows[0][13] = "LIVE"
        rows[0][14] = "crates/druid/src/core/live.rs::DruidLogical"
        rows[0][15] = "LIVE"
        rows[0][16] = "V1_LIVE"
        rows[0][17] = "LIVE"
        self.write_rows_preserve_oracle(rows)
        summary = verify_inventory(
            self.root,
            self.semantic,
            self.oracle,
            expected_source_manifest_sha256=self.source_manifest_sha256,
            phase="logical-live",
            adapter_live_entries={"java.sql.Type00"},
            logical_live_entries={"java.sql.Type00"},
            require_adapter_evidence=False,
            require_logical_evidence=False,
        )
        self.assertEqual(
            (52, 17, 0), (summary.java_sql, summary.javax_sql, summary.unknown)
        )
        adapter_summary = verify_inventory(
            self.root,
            self.semantic,
            self.oracle,
            expected_source_manifest_sha256=self.source_manifest_sha256,
            phase="adapter-live",
            adapter_live_entries={"java.sql.Type00"},
            logical_live_entries={"java.sql.Type00"},
            require_adapter_evidence=False,
            require_logical_evidence=False,
        )
        self.assertEqual(
            (52, 17, 0),
            (
                adapter_summary.java_sql,
                adapter_summary.javax_sql,
                adapter_summary.unknown,
            ),
        )

    def test_logical_live_rejects_planned_path_missing_and_unselected_live(
        self,
    ) -> None:
        """Logical LIVE requires one real path and is forbidden outside the exact Task 5 set."""

        rows = self.rows()
        rows[0][15] = "PLANNED"
        self.write_rows(rows)
        with self.assertRaises(VerificationError):
            verify_inventory(
                self.root,
                self.semantic,
                self.oracle,
                expected_source_manifest_sha256=self.source_manifest_sha256,
                phase="logical-live",
                adapter_live_entries=set(),
                logical_live_entries={"java.sql.Type00"},
                require_adapter_evidence=False,
                require_logical_evidence=False,
            )

        rows = self.rows()
        rows[1][14] = "crates/druid/src/core/fake.rs::FakeLogical"
        rows[1][15] = "LIVE"
        rows[1][16] = "V1_LIVE"
        rows[1][17] = "LIVE"
        self.write_rows_preserve_oracle(rows)
        with self.assertRaises(VerificationError):
            verify_inventory(
                self.root,
                self.semantic,
                self.oracle,
                expected_source_manifest_sha256=self.source_manifest_sha256,
                phase="logical-live",
                adapter_live_entries=set(),
                logical_live_entries={"java.sql.Type00"},
                require_adapter_evidence=False,
                require_logical_evidence=False,
            )

    def final_state_policy(self, rows: list[list[str]]) -> None:
        """调用真实最终状态策略；它不是 compiler/runtime 的替代证据。"""
        import verify_jdbc42_semantic_inventory as verifier

        policy = getattr(verifier, "verify_final_state_policy", None)
        self.assertTrue(callable(policy), "final semantic state policy is missing")
        policy(rows)

    def completed_state_rows(self) -> list[list[str]]:
        """仅构造状态策略正向控制，不宣称这些合成类型已编译。"""
        rows = self.rows()
        for row in rows:
            row[5] = "druid_rdbc::fixture::" + row[2]
            row[6] = "DECLARED"
            row[12:18] = ["-", "NOT_APPLICABLE", "-", "NOT_APPLICABLE", "V0_STATIC", "NOT_APPLICABLE"]
        return rows

    def test_final_state_policy_accepts_complete_declared_static_states(self) -> None:
        """纯值状态可以 DECLARED，但最终来源与编译证据仍由后续门禁验证。"""
        self.final_state_policy(self.completed_state_rows())

    def test_final_state_policy_rejects_each_planned_column(self) -> None:
        """每一计划态列独立暴露失败，避免被别列或 digest 错误遮蔽。"""
        for index, value in [(6, "PLANNED_TARGET"), (13, "PLANNED"), (15, "PLANNED"), (17, "PLANNED")]:
            with self.subTest(column=index):
                rows = self.completed_state_rows()
                rows[0][index] = value
                with self.assertRaisesRegex(VerificationError, "planned"):
                    self.final_state_policy(rows)

    def test_final_state_policy_rejects_unknown_missing_unresolved_any_field(self) -> None:
        """最终十九字段不能保留未知、缺失或未解析占位。"""
        for value in ["UNKNOWN", "MISSING", "unresolved", "PLANNED_FUTURE", "PLANNED_V2"]:
            with self.subTest(value=value):
                rows = self.completed_state_rows()
                rows[0][18] = value
                with self.assertRaisesRegex(VerificationError, "unresolved|planned"):
                    self.final_state_policy(rows)

    def test_final_state_policy_rejects_empty_canonical_and_invalid_target_state(self) -> None:
        for index, value in [(5, ""), (5, " "), (5, "-"), (6, "NOT_APPLICABLE")]:
            with self.subTest(column=index, value=value):
                rows = self.completed_state_rows()
                rows[0][index] = value
                with self.assertRaisesRegex(VerificationError, "canonical|target"):
                    self.final_state_policy(rows)

    def test_final_state_policy_rejects_fake_live_and_not_applicable_paths(self) -> None:
        for path_index, state_index in [(12, 13), (14, 15)]:
            for state, path in [("LIVE", "-"), ("LIVE", "outside/fake.rs::Fake"), ("NOT_APPLICABLE", "crates/fake.rs::Fake")]:
                with self.subTest(state=state, path=path, column=state_index):
                    rows = self.completed_state_rows()
                    rows[0][path_index], rows[0][state_index] = path, state
                    with self.assertRaisesRegex(VerificationError, "implementation"):
                        self.final_state_policy(rows)

    def test_final_state_policy_rejects_live_without_live_evidence(self) -> None:
        for variant in ["adapter", "logical", "capability"]:
            with self.subTest(variant=variant):
                rows = self.completed_state_rows()
                if variant == "adapter":
                    rows[0][12:14] = ["crates/druid-wrapper/src/live.rs::Live", "LIVE"]
                elif variant == "logical":
                    rows[0][14:16] = ["crates/druid/src/core/live.rs::Live", "LIVE"]
                rows[0][17] = "LIVE"
                with self.assertRaisesRegex(VerificationError, "evidence"):
                    self.final_state_policy(rows)

    def test_final_state_policy_accepts_live_states_without_certifying_paths(self) -> None:
        rows = self.completed_state_rows()
        rows[0][12:18] = ["crates/druid-wrapper/src/live.rs::Live", "LIVE", "crates/druid/src/core/live.rs::Live", "LIVE", "V1_LIVE", "LIVE"]
        self.final_state_policy(rows)

    def test_final_state_policy_accepts_existing_matrix_evidence_reference(self) -> None:
        """保留已接受矩阵引用的状态语法；真实矩阵证据须另行核验。"""
        rows = self.completed_state_rows()
        rows[0][12:18] = ["crates/druid-wrapper::standard_adapter_matrix", "LIVE", "-", "NOT_APPLICABLE", "V1_LIVE", "LIVE"]
        self.final_state_policy(rows)

    def test_final_state_policy_rejects_unknown_evidence_and_capability(self) -> None:
        for index, value in [(16, "V9_FAKE"), (17, "DISABLED"), (13, "SKIPPED"), (15, "SKIPPED")]:
            with self.subTest(column=index):
                rows = self.completed_state_rows()
                rows[0][index] = value
                with self.assertRaisesRegex(VerificationError, "state|evidence"):
                    self.final_state_policy(rows)

    def real_source_bound_inputs(self):
        """使用真实且已校验的 source oracle，保持冻结文件只读。"""
        import verify_jdbc42_semantic_inventory as verifier

        root = SCRIPTS.parent
        records, hashes = verifier.load_oracle(root / verifier.DEFAULT_ORACLE, verifier.SOURCE_MANIFEST_SHA256)
        rows = verifier.parse_semantic_payload((root / verifier.DEFAULT_SEMANTIC).read_bytes())
        return rows, records, hashes

    def final_source_binding(self, rows, records, hashes):
        import verify_jdbc42_semantic_inventory as verifier

        binding = getattr(verifier, "verify_final_source_binding", None)
        self.assertTrue(callable(binding), "final source binding is missing")
        return binding(rows, records, hashes)

    def test_final_source_binding_reconstructs_accepted_frozen_mapping(self) -> None:
        """恢复 Task3 接受的 DriverRegistry 映射，而非覆盖冻结摘要。"""
        rows, records, hashes = self.real_source_bound_inputs()
        self.assertIn(self.final_source_binding(rows, records, hashes), hashes)

    def test_final_source_binding_allows_evidence_progress_without_rewriting_oracle(self) -> None:
        """状态晋升不改变 Java 来源身份；状态与真实证据由其他组件核验。"""
        rows, records, hashes = self.real_source_bound_inputs()
        expected = self.final_source_binding(rows, records, hashes)
        rows[0][6] = "LIVE"
        rows[0][12:18] = ["crates/druid-wrapper/src/live.rs::Live", "LIVE", "crates/druid/src/core/live.rs::Live", "LIVE", "V1_LIVE", "LIVE"]
        self.assertEqual(expected, self.final_source_binding(rows, records, hashes))

    def test_final_source_binding_rejects_each_source_identity_drift(self) -> None:
        for index in [0, 1, 2, 7, 8]:
            with self.subTest(column=index):
                rows, records, hashes = self.real_source_bound_inputs()
                rows[0][index] += "_FORGED"
                with self.assertRaisesRegex(VerificationError, "source|coverage"):
                    self.final_source_binding(rows, records, hashes)

    def test_final_source_binding_rejects_missing_extra_and_duplicate_objects(self) -> None:
        for variant in ["missing", "extra", "duplicate"]:
            with self.subTest(variant=variant):
                rows, records, hashes = self.real_source_bound_inputs()
                if variant == "missing":
                    rows.pop()
                elif variant == "extra":
                    row = rows[-1].copy()
                    row[0], row[1] = "java:javax.sql.Fake#TYPE", "javax.sql.Fake"
                    rows.append(row)
                else:
                    rows[-1] = rows[0].copy()
                with self.assertRaisesRegex(VerificationError, "source|coverage|duplicate"):
                    self.final_source_binding(rows, records, hashes)

    def test_final_source_binding_rejects_frozen_semantic_digest_drift(self) -> None:
        rows, records, _ = self.real_source_bound_inputs()
        with self.assertRaisesRegex(VerificationError, "frozen.*digest"):
            self.final_source_binding(rows, records, {"0" * 64})

    def test_final_source_binding_rejects_modified_oracle_declaration(self) -> None:
        rows, records, hashes = self.real_source_bound_inputs()
        records[rows[0][1]]["normalized_signature"] += "_FORGED"
        with self.assertRaisesRegex(VerificationError, "frozen.*digest"):
            self.final_source_binding(rows, records, hashes)

    def test_final_source_binding_rejects_malformed_field_count(self) -> None:
        rows, records, hashes = self.real_source_bound_inputs()
        rows[0].pop()
        with self.assertRaisesRegex(VerificationError, "19 fields"):
            self.final_source_binding(rows, records, hashes)

    def test_rejects_unknown_phase_fail_closed(self) -> None:
        """No unimplemented future phase may silently reuse logical validation."""

        with self.assertRaisesRegex(
            VerificationError, "unknown semantic verification phase"
        ):
            verify_inventory(
                self.root,
                self.semantic,
                self.oracle,
                expected_source_manifest_sha256=self.source_manifest_sha256,
                phase="final-live",
            )

    def test_require_standard_entries_rejects_task2_only_inventory(self) -> None:
        """The Task 3 gate fails while every canonical standard remains only planned."""

        with self.assertRaises(VerificationError):
            verify_inventory(
                self.root,
                self.semantic,
                self.oracle,
                expected_source_manifest_sha256=self.source_manifest_sha256,
                require_standard_entries=True,
            )

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
        self.oracle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )
        self.assert_rejected()

    def test_rejects_oracle_source_manifest_drift(self) -> None:
        """Changing one recorded OpenJDK source hash must invalidate the oracle."""

        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        payload["source_manifest_sha256"] = "0" * 64
        self.oracle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )
        self.assert_rejected()

    def test_rejects_extra_and_duplicate_json_keys(self) -> None:
        """Every JSON object layer has an exact schema and duplicate keys are invalid."""

        original = self.oracle.read_text(encoding="utf-8")
        payload = json.loads(original)
        payload["extra"] = "not part of the oracle schema"
        self.oracle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )
        self.assert_rejected()

        self.oracle.write_text(
            original.replace("{\n", '{\n  "archive_size": 94004963,\n', 1),
            encoding="utf-8", newline="",
        )
        self.assert_rejected()

    def test_rejects_noncanonical_json_serialization(self) -> None:
        """Semantically equal JSON with alternate whitespace is not canonical oracle evidence."""

        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        self.oracle.write_text(
            json.dumps(payload, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )
        self.assert_rejected()

    def test_rejects_noncanonical_or_control_bearing_semantic_tsv(self) -> None:
        """CRLF and embedded control data cannot become canonical semantic rows."""

        canonical = self.semantic.read_bytes()
        self.semantic.write_bytes(canonical.replace(b"\n", b"\r\n"))
        payload = json.loads(self.oracle.read_text(encoding="utf-8"))
        payload["semantic_sha256"] = hashlib.sha256(
            self.semantic.read_bytes()
        ).hexdigest()
        self.oracle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )
        self.assert_rejected()

        self.semantic.write_bytes(
            canonical.replace(b"synthetic verifier fixture", b"synthetic\x01fixture", 1)
        )
        payload["semantic_sha256"] = hashlib.sha256(
            self.semantic.read_bytes()
        ).hexdigest()
        self.oracle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )
        self.assert_rejected()

        self.semantic.write_bytes(
            canonical.replace(b"synthetic verifier fixture", b"synthetic\x7ffixture", 1)
        )
        payload["semantic_sha256"] = hashlib.sha256(
            self.semantic.read_bytes()
        ).hexdigest()
        self.oracle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )
        self.assert_rejected()

        self.semantic.write_bytes(
            canonical.replace(
                b"synthetic verifier fixture",
                "synthetic\u0085fixture".encode("utf-8"),
                1,
            )
        )
        payload["semantic_sha256"] = hashlib.sha256(
            self.semantic.read_bytes()
        ).hexdigest()
        self.oracle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline=""
        )
        self.assert_rejected()

    def test_archive_discovery_uses_source_entries_not_a_manual_object_list(
        self,
    ) -> None:
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
        self.assertEqual(
            ["java.sql.Alpha", "javax.sql.Beta"], [row.fqcn for row in first]
        )

        with tarfile.open(archive, "w:gz") as bundle:
            for name in ("Alpha", "Gamma"):
                source = f"package java.sql; public interface {name} {{}}".encode(
                    "utf-8"
                )
                member = tarfile.TarInfo(
                    f"jdk8u-test/jdk/src/share/classes/java/sql/{name}.java"
                )
                member.size = len(source)
                bundle.addfile(member, BytesIO(source))
        second = discover_java_records(archive, enforce_jdbc42_counts=False)
        self.assertEqual(
            ["java.sql.Alpha", "java.sql.Gamma"], [row.fqcn for row in second]
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
