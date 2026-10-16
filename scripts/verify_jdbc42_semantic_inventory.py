#!/usr/bin/env python3
"""Verify the source-bound JDBC 4.2 semantic inventory for P04 Task 2."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import unicodedata
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
    "MATCH",
    "SQL_FEATURE_UNSUPPORTED_0A000",
    "JAVA_DEFAULT_UNSUPPORTED_TYPED",
    "JAVA_RUNTIME_ONLY",
}
ADAPTER_STATES = {"PLANNED", "NOT_APPLICABLE", "LIVE"}
LOGICAL_STATES = {"PLANNED", "NOT_APPLICABLE", "LIVE"}
CAPABILITY_STATES = {"PLANNED", "NOT_APPLICABLE"}
TARGET_STATES = {"PLANNED_TARGET", "DECLARED"}
TASK3_STANDARD_ENTRIES = {
    "java.sql.Array": "druid_rdbc::resource::Array",
    "java.sql.BatchUpdateException": "druid_rdbc::error::BatchUpdateException",
    "java.sql.Blob": "druid_rdbc::resource::Blob",
    "java.sql.CallableStatement": "druid_rdbc::statement::CallableStatement",
    "java.sql.ClientInfoStatus": "druid_rdbc::connection::ClientInfoStatus",
    "java.sql.Clob": "druid_rdbc::resource::Clob",
    "java.sql.Connection": "druid_rdbc::connection::Connection",
    "java.sql.DataTruncation": "druid_rdbc::error::DataTruncation",
    "java.sql.DatabaseMetaData": "druid_rdbc::metadata::DatabaseMetaData",
    "java.sql.Date": "druid_rdbc::types::Date",
    "java.sql.Driver": "druid_rdbc::driver::Driver",
    "java.sql.DriverAction": "druid_rdbc::driver::DriverAction",
    "java.sql.DriverManager": "druid_rdbc::driver::DriverManager",
    "java.sql.DriverPropertyInfo": "druid_rdbc::driver::DriverPropertyInfo",
    "java.sql.JDBCType": "druid_rdbc::types::JdbcType",
    "java.sql.NClob": "druid_rdbc::resource::NClob",
    "java.sql.ParameterMetaData": "druid_rdbc::metadata::ParameterMetaData",
    "java.sql.PreparedStatement": "druid_rdbc::statement::PreparedStatement",
    "java.sql.PseudoColumnUsage": "druid_rdbc::metadata::PseudoColumnUsage",
    "java.sql.Ref": "druid_rdbc::resource::Ref",
    "java.sql.ResultSet": "druid_rdbc::result_set::ResultSet",
    "java.sql.ResultSetMetaData": "druid_rdbc::metadata::ResultSetMetaData",
    "java.sql.RowId": "druid_rdbc::types::RowId",
    "java.sql.RowIdLifetime": "druid_rdbc::metadata::RowIdLifetime",
    "java.sql.SQLClientInfoException": "druid_rdbc::error::SqlClientInfoException",
    "java.sql.SQLData": "druid_rdbc::resource::SqlData",
    "java.sql.SQLDataException": "druid_rdbc::error::SqlExceptionKind::Data",
    "java.sql.SQLException": "druid_rdbc::error::SqlException",
    "java.sql.SQLFeatureNotSupportedException": "druid_rdbc::error::SqlExceptionKind::FeatureNotSupported",
    "java.sql.SQLInput": "druid_rdbc::resource::SqlInput",
    "java.sql.SQLIntegrityConstraintViolationException": "druid_rdbc::error::SqlExceptionKind::IntegrityConstraintViolation",
    "java.sql.SQLInvalidAuthorizationSpecException": "druid_rdbc::error::SqlExceptionKind::InvalidAuthorizationSpec",
    "java.sql.SQLNonTransientConnectionException": "druid_rdbc::error::SqlExceptionKind::NonTransientConnection",
    "java.sql.SQLNonTransientException": "druid_rdbc::error::SqlExceptionKind::NonTransient",
    "java.sql.SQLOutput": "druid_rdbc::resource::SqlOutput",
    "java.sql.SQLPermission": "host::process_security_policy",
    "java.sql.SQLRecoverableException": "druid_rdbc::error::SqlExceptionKind::Recoverable",
    "java.sql.SQLSyntaxErrorException": "druid_rdbc::error::SqlExceptionKind::Syntax",
    "java.sql.SQLTimeoutException": "druid_rdbc::error::SqlExceptionKind::Timeout",
    "java.sql.SQLTransactionRollbackException": "druid_rdbc::error::SqlExceptionKind::TransactionRollback",
    "java.sql.SQLTransientConnectionException": "druid_rdbc::error::SqlExceptionKind::TransientConnection",
    "java.sql.SQLTransientException": "druid_rdbc::error::SqlExceptionKind::Transient",
    "java.sql.SQLType": "druid_rdbc::types::SqlType",
    "java.sql.SQLWarning": "druid_rdbc::error::SqlWarning",
    "java.sql.SQLXML": "druid_rdbc::resource::SqlXml",
    "java.sql.Savepoint": "druid_rdbc::connection::Savepoint",
    "java.sql.Statement": "druid_rdbc::statement::Statement",
    "java.sql.Struct": "druid_rdbc::resource::Struct",
    "java.sql.Time": "druid_rdbc::types::Time",
    "java.sql.Timestamp": "druid_rdbc::types::Timestamp",
    "java.sql.Types": "druid_rdbc::types::types",
    "java.sql.Wrapper": "druid_rdbc::wrapper::Wrapper",
    "javax.sql.CommonDataSource": "druid_rdbc::data_source::CommonDataSource",
    "javax.sql.ConnectionEvent": "druid_rdbc::data_source::ConnectionEvent",
    "javax.sql.ConnectionEventListener": "druid_rdbc::data_source::ConnectionEventListener",
    "javax.sql.ConnectionPoolDataSource": "druid_rdbc::data_source::ConnectionPoolDataSource",
    "javax.sql.DataSource": "druid_rdbc::data_source::DataSource",
    "javax.sql.PooledConnection": "druid_rdbc::data_source::PooledConnection",
    "javax.sql.RowSet": "druid_rdbc::extensions::RowSet",
    "javax.sql.RowSetEvent": "druid_rdbc::extensions::RowSetEvent",
    "javax.sql.RowSetInternal": "druid_rdbc::extensions::RowSetInternal",
    "javax.sql.RowSetListener": "druid_rdbc::extensions::RowSetListener",
    "javax.sql.RowSetMetaData": "druid_rdbc::extensions::RowSetMetaData",
    "javax.sql.RowSetReader": "druid_rdbc::extensions::RowSetReader",
    "javax.sql.RowSetWriter": "druid_rdbc::extensions::RowSetWriter",
    "javax.sql.StatementEvent": "druid_rdbc::data_source::StatementEvent",
    "javax.sql.StatementEventListener": "druid_rdbc::data_source::StatementEventListener",
    "javax.sql.XAConnection": "druid_rdbc::data_source::XaConnection",
    "javax.sql.XADataSource": "druid_rdbc::data_source::XaDataSource",
}
# 已核对的公开声明修正。冻结的 Task 2/3 来源摘要仍使用上方原始映射。
# 每项为（冻结形态，当前形态，当前公开路径）；其他行不得借此改变摘要。
CURRENT_DECLARATION_CORRECTIONS = {
    "java.sql.BatchUpdateException": ("ENUM_KIND", "DTO", TASK3_STANDARD_ENTRIES["java.sql.BatchUpdateException"]),
    "java.sql.DataTruncation": ("ENUM_KIND", "DTO", TASK3_STANDARD_ENTRIES["java.sql.DataTruncation"]),
    "java.sql.DriverManager": ("MODULE", "DTO", TASK3_STANDARD_ENTRIES["java.sql.DriverManager"]),
    "java.sql.SQLClientInfoException": ("ENUM_KIND", "DTO", TASK3_STANDARD_ENTRIES["java.sql.SQLClientInfoException"]),
    "java.sql.SQLException": ("ENUM_KIND", "DTO", TASK3_STANDARD_ENTRIES["java.sql.SQLException"]),
    "java.sql.SQLInvalidAuthorizationSpecException": (
        "ENUM_KIND", "ENUM_KIND", "druid_rdbc::error::SqlExceptionKind::InvalidAuthorization"
    ),
    "java.sql.SQLWarning": ("ENUM_KIND", "DTO", TASK3_STANDARD_ENTRIES["java.sql.SQLWarning"]),
}
TASK4_ADAPTER_LIVE_ENTRIES = {
    "java.sql.Array",
    "java.sql.Blob",
    "java.sql.Clob",
    "java.sql.NClob",
    "java.sql.Connection",
    "java.sql.DatabaseMetaData",
    "java.sql.ParameterMetaData",
    "java.sql.PreparedStatement",
    "java.sql.ResultSet",
    "java.sql.ResultSetMetaData",
    "java.sql.Statement",
    "java.sql.Wrapper",
}
TASK5_LOGICAL_LIVE_ENTRIES = {
    "java.sql.CallableStatement",
    "java.sql.Connection",
    "java.sql.DatabaseMetaData",
    "java.sql.ParameterMetaData",
    "java.sql.PreparedStatement",
    "java.sql.ResultSet",
    "java.sql.ResultSetMetaData",
    "java.sql.Statement",
    "java.sql.Wrapper",
}
TASK6_SERVICE_LIVE_ENTRIES = {"java.sql.DriverManager"}
PHASES = {"planned", "adapter-live", "logical-live"}
ORACLE_KEYS = {
    "schema_version",
    "oracle_tag",
    "oracle_commit",
    "archive_url",
    "archive_sha256",
    "archive_size",
    "source_manifest_sha256",
    "packages",
    "semantic_sha256",
}


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

    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(ch not in "0123456789abcdef" for ch in value)
    ):
        fail(f"{label} is not a lowercase SHA-256")
    return value


def reject_duplicate_json_keys(pairs: list[tuple[str, object]]) -> dict[str, object]:
    """Build one JSON object while rejecting duplicate keys at every nesting level."""

    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail(f"JDBC oracle duplicates JSON key {key!r}")
        result[key] = value
    return result


def canonical_json_bytes(payload: object) -> bytes:
    """Return the one accepted deterministic JSON representation."""

    return (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")


def parse_semantic_payload(payload: bytes) -> list[list[str]]:
    """Parse the exact canonical TSV schema independently of semantic validation."""

    if not payload.endswith(b"\n") or payload.endswith(b"\n\n"):
        fail("semantic inventory must have exactly one final newline")
    if b"\r" in payload or any(
        (byte < 0x20 and byte not in {0x09, 0x0A}) or byte == 0x7F for byte in payload
    ):
        fail("semantic inventory contains a forbidden control byte")
    try:
        lines = payload.decode("utf-8").split("\n")[:-1]
    except UnicodeDecodeError as error:
        fail(f"semantic inventory is not UTF-8: {error}")
    if not lines or lines[0] != SEMANTIC_HEADER:
        fail("semantic inventory header drift")
    rows = [line.split("\t") for line in lines[1:]]
    for line_number, fields in enumerate(rows, start=2):
        if len(fields) != 19:
            fail(f"semantic row {line_number} must have 19 fields")
        if any(not field for field in fields):
            fail(f"semantic row {line_number} contains an empty field")
        if any(
            unicodedata.category(character) == "Cc"
            for field in fields
            for character in field
        ):
            fail(f"semantic row {line_number} contains a Unicode control character")
    if rows != sorted(rows, key=lambda fields: fields[0]):
        fail("semantic inventory rows are not sorted by item_key")
    canonical = (
        SEMANTIC_HEADER + "\n" + "\n".join("\t".join(fields) for fields in rows) + "\n"
    ).encode("utf-8")
    if payload != canonical:
        fail("semantic inventory is not canonically serialized")
    return rows


def canonical_semantic_payload(rows: list[list[str]]) -> bytes:
    """Serialize already validated rows using the one canonical TSV representation."""

    return (
        SEMANTIC_HEADER + "\n" + "\n".join("\t".join(row) for row in rows) + "\n"
    ).encode("utf-8")


def verify_current_declaration_corrections(rows: list[list[str]]) -> None:
    """仅接受已审定的当前公开形态和路径，避免冻结摘要归一化掩盖漂移。"""

    for row in rows:
        correction = CURRENT_DECLARATION_CORRECTIONS.get(row[1])
        if correction and (row[3], row[5]) != correction[1:]:
            fail(f"current public declaration correction drift: {row[1]}")


def is_static_capability_not_applicable(row: list[str]) -> bool:
    """只接受无适配器/逻辑对象的值声明；DriverManager 是主动注册服务。"""

    return (
        row[1] != "java.sql.DriverManager"
        and row[3] in {"DTO", "ENUM_KIND", "MODULE"}
        and row[12:16] == ["-", "NOT_APPLICABLE", "-", "NOT_APPLICABLE"]
        and row[16] == "V0_STATIC"
    )


def verify_final_state_policy(rows: list[list[str]]) -> None:
    """检查最终状态自洽性；来源、公开类型及运行证据仍须独立验证。"""

    fields = SEMANTIC_HEADER.split("\t")
    for line_number, row in enumerate(rows, start=2):
        if len(row) != len(fields):
            fail(f"final semantic row {line_number} must have 19 fields")
        for field, value in zip(fields, row):
            if not value.strip():
                fail(f"final semantic row {line_number} has empty {field}")
            if re.search(r"\b(?:PLANNED(?:_[A-Z0-9_]+)?|MISSING|UNKNOWN|UNRESOLVED)\b", value, re.IGNORECASE):
                fail(f"final semantic row {line_number} has planned/unresolved {field}: {value}")
        if row[5] == "-":
            fail(f"final semantic row {line_number} has no canonical target")
        if row[6] not in {"DECLARED", "LIVE"}:
            fail(f"final semantic row {line_number} has invalid target state {row[6]}")
        for path_index, state_index, prefix in [
            (12, 13, "crates/druid-wrapper/"),
            (14, 15, "crates/druid/"),
        ]:
            path, state = row[path_index], row[state_index]
            if state not in {"NOT_APPLICABLE", "LIVE"}:
                fail(f"final semantic row {line_number} has invalid implementation state {state}")
            if state == "NOT_APPLICABLE" and path != "-":
                fail(f"final semantic row {line_number} has a non-applicable implementation path")
            matrix_reference = path_index == 12 and path == "crates/druid-wrapper::standard_adapter_matrix"
            if state == "LIVE" and not matrix_reference and (not path.startswith(prefix) or "::" not in path):
                fail(f"final semantic row {line_number} has invalid LIVE implementation path")
        if row[16] not in {"V0_STATIC", "V1_LIVE"}:
            fail(f"final semantic row {line_number} has invalid evidence level {row[16]}")
        if row[17] not in {"NOT_APPLICABLE", "LIVE"}:
            fail(f"final semantic row {line_number} has invalid capability state {row[17]}")
        if (
            row[17] == "NOT_APPLICABLE"
            and row[3] != "EXCLUDED_RUNTIME"
            and not is_static_capability_not_applicable(row)
        ):
            fail(f"final semantic row {line_number} has invalid static capability disposition")
        live = row[13] == "LIVE" or row[15] == "LIVE" or row[17] == "LIVE"
        if live and row[16] != "V1_LIVE":
            fail(f"final semantic row {line_number} lacks LIVE evidence")
        if (row[13] == "LIVE" or row[15] == "LIVE") and row[17] != "LIVE":
            fail(f"final semantic row {line_number} lacks LIVE capability evidence")


def verify_final_source_binding(
    rows: list[list[str]],
    records: dict[str, dict[str, object]],
    expected_semantic_hashes: set[str],
) -> str:
    """绑定 Java 来源与冻结映射；不将状态晋升视为 Rust 实现证据。"""

    from generate_jdbc42_semantic_inventory import (
        GenerationError,
        JavaRecord,
        render_semantic,
    )

    # 从已校验 oracle 的声明重建 Task2，并保留 Task3 已接受的路径与声明态。
    # 这里不能用当前 final 行的状态或 Rust 映射来生成期望摘要。
    try:
        source_records = [JavaRecord(**records[fqcn]) for fqcn in sorted(records)]
        baseline = parse_semantic_payload(render_semantic(source_records))
    except (GenerationError, TypeError, ValueError) as error:
        fail(f"cannot reconstruct frozen semantic source mapping: {error}")
    for row in baseline:
        if row[1] in TASK3_STANDARD_ENTRIES:
            row[5] = TASK3_STANDARD_ENTRIES[row[1]]
            row[6] = "DECLARED"
    digest = hashlib.sha256(canonical_semantic_payload(baseline)).hexdigest()
    if digest not in expected_semantic_hashes:
        fail("reconstructed frozen semantic mapping digest does not match oracle")
    counts = {
        package: sum(fqcn.startswith(package + ".") for fqcn in records)
        for package in ("java.sql", "javax.sql")
    }
    if counts != {"java.sql": 52, "javax.sql": 17}:
        fail("final semantic source coverage must remain java.sql 52 + javax.sql 17")
    by_fqcn = {row[1]: row for row in baseline}
    seen: set[str] = set()
    # Rust 形态、canonical entry 和实现证据另由最终 Rust/运行证据门禁核验。
    # 本组件只比较不可晋升的 Java 身份、成员、签名与声明类别。
    for line_number, row in enumerate(rows, start=2):
        if len(row) != 19:
            fail(f"final semantic row {line_number} must have 19 fields")
        fqcn = row[1]
        if fqcn in seen:
            fail(f"final semantic source coverage duplicates {fqcn}")
        seen.add(fqcn)
        expected = by_fqcn.get(fqcn)
        if expected is None:
            fail(f"final semantic source coverage has extra object {fqcn}")
        for index in (0, 1, 2, 7, 8):
            if row[index] != expected[index]:
                fail(f"final semantic row {line_number} has source declaration drift at field {index}")
    if seen != set(records):
        fail(f"final semantic source coverage missing objects: {sorted(set(records) - seen)}")
    return digest


def normalized_phase_digest(
    rows: list[list[str]],
    phase: str,
    adapter_live_entries: set[str],
    logical_live_entries: set[str],
) -> str:
    """Preserve the frozen denominator while advancing Task 4/5 evidence columns."""

    verify_current_declaration_corrections(rows)
    normalized = [row.copy() for row in rows]
    for row in normalized:
        correction = CURRENT_DECLARATION_CORRECTIONS.get(row[1])
        if correction:
            row[3] = correction[0]
            row[5] = TASK3_STANDARD_ENTRIES[row[1]]
        if row[1] in TASK6_SERVICE_LIVE_ENTRIES and row[17] == "LIVE":
            # 服务能力已由跨 crate 运行测试证明；来源摘要仍表示原计划态。
            row[16] = "V0_STATIC"
            row[17] = "PLANNED"
        if row[17] == "NOT_APPLICABLE" and is_static_capability_not_applicable(row):
            # 仅证据分类前进；冻结的 Java 来源与既有语义映射摘要保持不变。
            row[17] = "PLANNED"
    if phase in {"adapter-live", "logical-live"}:
        for row in normalized:
            if row[1] in adapter_live_entries:
                row[12] = "-"
                row[13] = "PLANNED"
                row[16] = "V0_STATIC"
                row[17] = "PLANNED"
    if phase in {"adapter-live", "logical-live"}:
        for row in normalized:
            if row[1] in logical_live_entries and row[15] == "LIVE":
                row[14] = "-"
                row[15] = "PLANNED"
                row[16] = "V0_STATIC"
                row[17] = "PLANNED"
    return hashlib.sha256(canonical_semantic_payload(normalized)).hexdigest()


def load_oracle(
    path: Path,
    expected_source_manifest_sha256: str,
) -> tuple[dict[str, dict[str, object]], set[str]]:
    """Load and validate the fixed OpenJDK oracle plus its source-derived object rows."""

    if not path.is_file():
        fail(f"JDBC oracle not found: {path}")
    try:
        raw = path.read_bytes()
        text = raw.decode("utf-8")
        payload = json.loads(text, object_pairs_hook=reject_duplicate_json_keys)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"cannot read JDBC oracle: {error}")
    if not isinstance(payload, dict) or set(payload) != ORACLE_KEYS:
        fail("JDBC oracle top-level schema drift")
    if raw != canonical_json_bytes(payload):
        fail("JDBC oracle is not canonically serialized")
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
        if set(package_record) != {"object_count", "objects"}:
            fail(f"JDBC oracle package schema drift: {package}")
        objects = package_record.get("objects")
        if (
            package_record.get("object_count") != expected_count
            or not isinstance(objects, list)
            or len(objects) != expected_count
        ):
            fail(f"JDBC oracle {package} count drift")
        for index, record in enumerate(objects):
            if not isinstance(record, dict):
                fail(f"JDBC oracle {package} object {index} is invalid")
            required = {
                "fqcn",
                "object_name",
                "declaration_kind",
                "normalized_signature",
                "source_path",
                "source_sha256",
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
            expected_suffix = (
                f"jdk/src/share/classes/{package.replace('.', '/')}/{name}.java"
            )
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
        (
            json.dumps(canonical_objects, sort_keys=True, separators=(",", ":")) + "\n"
        ).encode("utf-8")
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
    require_standard_entries: bool = False,
    phase: str = "planned",
    adapter_live_entries: set[str] | None = None,
    logical_live_entries: set[str] | None = None,
    require_adapter_evidence: bool = True,
    require_logical_evidence: bool = True,
    require_final_evidence: bool = False,
) -> InventorySummary:
    """Verify exact source coverage and honest planned/declared target phases."""

    if phase not in PHASES:
        fail(f"unknown semantic verification phase: {phase}")
    adapter_live_entries = (
        TASK4_ADAPTER_LIVE_ENTRIES
        if adapter_live_entries is None
        else adapter_live_entries
    )
    logical_live_entries = (
        TASK5_LOGICAL_LIVE_ENTRIES
        if logical_live_entries is None
        else logical_live_entries
    )
    semantic = resolve_path(root, semantic_path)
    oracle = resolve_path(root, oracle_path)
    oracle_payload = oracle.read_bytes() if require_final_evidence and oracle.is_file() else None
    records, expected_semantic_hashes = load_oracle(
        oracle,
        expected_source_manifest_sha256,
    )
    if not semantic.is_file():
        fail(f"JDBC semantic inventory not found: {semantic}")
    payload = semantic.read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    rows = parse_semantic_payload(payload)
    verify_current_declaration_corrections(rows)
    if require_final_evidence:
        if not require_adapter_evidence or not require_logical_evidence:
            fail("final semantic gate cannot skip adapter or logical evidence")
        # 最终阶段允许证据状态晋升，但 Java 来源身份仍绑定独立重建的冻结摘要。
        final_digest = verify_final_source_binding(rows, records, expected_semantic_hashes)
        verify_final_state_policy(rows)
        for line_number, row in enumerate(rows, start=2):
            if row[3] not in RUST_SHAPES:
                fail(f"final semantic row {line_number} has unknown rust_shape")
            if row[4] not in SEMANTIC_DISPOSITIONS:
                fail(f"final semantic row {line_number} has unknown disposition")
            if require_standard_entries and row[1] in TASK3_STANDARD_ENTRIES:
                expected_entry = CURRENT_DECLARATION_CORRECTIONS.get(row[1])
                canonical_entry = expected_entry[2] if expected_entry else TASK3_STANDARD_ENTRIES[row[1]]
                if row[5] != canonical_entry:
                    fail(f"final Task3 canonical entry drift: {row[1]}")
        verify_final_runtime_evidence(root, rows)
        if semantic.read_bytes() != payload:
            fail("final semantic inventory changed during evidence verification")
        if oracle.read_bytes() != oracle_payload:
            fail("final source oracle changed during evidence verification")
        return InventorySummary(52, 17, 0, final_digest)
    phase_digest = normalized_phase_digest(
        rows,
        phase,
        adapter_live_entries,
        logical_live_entries,
    )
    if phase_digest not in expected_semantic_hashes:
        fail("semantic inventory bytes do not match the source oracle digest")
    seen_keys: set[str] = set()
    seen_fqcns: set[str] = set()
    counts = {"java.sql": 0, "javax.sql": 0}
    standard_entries: dict[str, tuple[str, str]] = {}
    for line_number, fields in enumerate(rows, start=2):
        (
            item_key,
            fqcn,
            java_member,
            rust_shape,
            disposition,
            canonical,
            target_state,
            signature,
            call_kind,
            index_null_adapter,
            error_adapter,
            owner,
            adapter_implementation,
            adapter_state,
            logical_implementation,
            logical_state,
            evidence_level,
            capability_state,
            notes,
        ) = fields
        if item_key in seen_keys:
            fail(f"semantic row {line_number} duplicates item key {item_key}")
        if fqcn in seen_fqcns:
            fail(f"semantic row {line_number} duplicates Java object {fqcn}")
        seen_keys.add(item_key)
        seen_fqcns.add(fqcn)
        record = records.get(fqcn)
        if record is None:
            fail(
                f"semantic row {line_number} is not present in the source oracle: {fqcn}"
            )
        if item_key != f"java:{fqcn}#TYPE":
            fail(f"semantic row {line_number} has an invalid item key")
        if (
            java_member != record["object_name"]
            or signature != record["normalized_signature"]
        ):
            fail(f"semantic row {line_number} drifts from source declaration {fqcn}")
        package = "javax.sql" if fqcn.startswith("javax.sql.") else "java.sql"
        counts[package] += 1
        if rust_shape not in RUST_SHAPES:
            fail(f"semantic row {line_number} has unknown rust_shape {rust_shape}")
        if disposition not in SEMANTIC_DISPOSITIONS:
            fail(f"semantic row {line_number} has unknown disposition {disposition}")
        if target_state not in TARGET_STATES:
            fail(f"semantic row {line_number} has invalid target state {target_state}")
        if canonical == "-":
            fail(f"semantic row {line_number} has no canonical semantic target")
        if rust_shape == "EXCLUDED_RUNTIME":
            if disposition != "JAVA_RUNTIME_ONLY" or not canonical.startswith("host::"):
                fail(f"semantic row {line_number} has an invalid runtime exclusion")
        elif not canonical.startswith("druid_rdbc::"):
            fail(f"semantic row {line_number} has an invalid canonical Rust target")
        if adapter_state not in ADAPTER_STATES or logical_state not in LOGICAL_STATES:
            fail(f"semantic row {line_number} has unknown implementation state")
        if (
            capability_state == "NOT_APPLICABLE"
            and rust_shape != "EXCLUDED_RUNTIME"
            and not is_static_capability_not_applicable(fields)
        ):
            fail(f"semantic row {line_number} has invalid static capability disposition")
        adapter_live = fqcn in adapter_live_entries and phase in {
            "adapter-live",
            "logical-live",
        }
        logical_live = (
            fqcn in logical_live_entries
            and phase in {"adapter-live", "logical-live"}
            and logical_state == "LIVE"
        )
        service_live = (
            fqcn in TASK6_SERVICE_LIVE_ENTRIES
            and phase == "logical-live"
            and capability_state == "LIVE"
        )
        if (
            phase == "logical-live"
            and fqcn in logical_live_entries
            and not logical_live
        ):
            fail(f"semantic row {line_number} lacks Task 5 logical LIVE evidence")
        if adapter_live:
            if adapter_state != "LIVE" or adapter_implementation == "-":
                fail(f"semantic row {line_number} lacks Task 4 adapter LIVE evidence")
            if evidence_level != "V1_LIVE" or capability_state != "LIVE":
                fail(
                    f"semantic row {line_number} lacks Task 4 live capability evidence"
                )
        else:
            if (
                adapter_state not in {"PLANNED", "NOT_APPLICABLE"}
                or adapter_implementation != "-"
            ):
                fail(
                    f"semantic row {line_number} claims an implementation outside its phase"
                )
            if not logical_live and not service_live and evidence_level != "V0_STATIC":
                fail(f"semantic row {line_number} overstates evidence {evidence_level}")
        if service_live and evidence_level != "V1_LIVE":
            fail(f"semantic row {line_number} lacks service capability evidence")
        if logical_live:
            if logical_state != "LIVE" or logical_implementation == "-":
                fail(f"semantic row {line_number} lacks Task 5 logical LIVE evidence")
            if evidence_level != "V1_LIVE" or capability_state != "LIVE":
                fail(
                    f"semantic row {line_number} lacks Task 5 logical capability evidence"
                )
        else:
            if logical_state not in {"PLANNED", "NOT_APPLICABLE"}:
                fail(
                    f"semantic row {line_number} claims logical LIVE outside Task 5 scope"
                )
            if logical_implementation != "-":
                fail(
                    f"semantic row {line_number} claims a fake logical implementation path"
                )
        if (
            not adapter_live
            and not logical_live
            and not service_live
            and capability_state not in CAPABILITY_STATES
        ):
            fail(f"semantic row {line_number} has unknown capability state")
        if not call_kind.endswith("_DECLARATION"):
            fail(f"semantic row {line_number} has invalid call_kind")
        if (
            index_null_adapter == "UNKNOWN"
            or error_adapter == "UNKNOWN"
            or owner == "UNKNOWN"
            or notes == "UNKNOWN"
        ):
            fail(f"semantic row {line_number} contains UNKNOWN")
        if fqcn in TASK3_STANDARD_ENTRIES:
            standard_entries[fqcn] = (canonical, target_state)
    if seen_fqcns != set(records):
        missing = sorted(set(records) - seen_fqcns)
        extra = sorted(seen_fqcns - set(records))
        fail(f"semantic source coverage mismatch: missing={missing} extra={extra}")
    if (counts["java.sql"], counts["javax.sql"]) != (52, 17):
        fail(
            "semantic package count drift: "
            f"JAVA_SQL={counts['java.sql']} JAVAX_SQL={counts['javax.sql']}"
        )
    if require_standard_entries:
        missing = sorted(set(TASK3_STANDARD_ENTRIES) - set(standard_entries))
        if missing:
            fail(f"Task 3 standard entries are missing: {missing}")
        drift = sorted(
            fqcn
            for fqcn, expected in TASK3_STANDARD_ENTRIES.items()
            if standard_entries[fqcn] != (
                CURRENT_DECLARATION_CORRECTIONS[fqcn][2]
                if fqcn in CURRENT_DECLARATION_CORRECTIONS else expected,
                "DECLARED",
            )
        )
        if drift:
            fail(
                f"Task 3 standard entries are not DECLARED at canonical paths: {drift}"
            )
    if phase in {"adapter-live", "logical-live"} and require_adapter_evidence:
        try:
            from verify_wrapper_standard_impls import verify_repository

            verify_repository(root)
        except (ImportError, OSError, RuntimeError) as error:
            if phase in {
                "adapter-live",
                "logical-live",
            } and "coverage requirement source hash is stale" in str(error):
                try:
                    from verify_druid_single_standard import (
                        verify_task4_evidence_handoff,
                    )

                    verify_task4_evidence_handoff(root)
                except (ImportError, OSError, RuntimeError) as handoff_error:
                    fail(
                        f"Task 4 planned Task 5 evidence handoff failed: {handoff_error}"
                    )
            else:
                fail(f"Task 4 wrapper live evidence failed: {error}")
    if phase == "logical-live" and require_logical_evidence:
        try:
            from verify_druid_single_standard import verify_repository

            verify_repository(root)
        except (ImportError, OSError, RuntimeError) as error:
            fail(f"Task 5 Druid logical live evidence failed: {error}")
    return InventorySummary(counts["java.sql"], counts["javax.sql"], 0, phase_digest)


def repository_root() -> Path:
    """Return the repository root independently of the command's CWD."""

    return Path(__file__).resolve().parent.parent


def verify_final_runtime_evidence(root: Path, rows: list[list[str]]) -> None:
    """执行新鲜公开形态及原完整运行门禁，不接受仅状态晋升或历史收据。"""
    from verify_jdbc42_final_runtime import verify_runtime

    verify_runtime(root, rows)


def parse_arguments() -> argparse.Namespace:
    """Parse optional manifest paths for self-tests and the default live gate."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semantic", type=Path, default=DEFAULT_SEMANTIC)
    parser.add_argument("--oracle", type=Path, default=DEFAULT_ORACLE)
    parser.add_argument("--require-standard-entries", action="store_true")
    parser.add_argument("--require-final-evidence", action="store_true")
    parser.add_argument("--phase", choices=sorted(PHASES), default="planned")
    return parser.parse_args()


def main() -> int:
    """Run the live semantic gate."""

    arguments = parse_arguments()
    try:
        summary = verify_inventory(
            repository_root(),
            arguments.semantic,
            arguments.oracle,
            require_standard_entries=arguments.require_standard_entries,
            phase=arguments.phase,
            require_final_evidence=arguments.require_final_evidence,
        )
    except VerificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"JAVA_SQL={summary.java_sql}")
    print(f"JAVAX_SQL={summary.javax_sql}")
    print(f"UNKNOWN={summary.unknown}")
    print(f"SEMANTIC_SHA256={summary.semantic_sha256}")
    if arguments.phase in {"adapter-live", "logical-live"}:
        semantic = resolve_path(repository_root(), arguments.semantic)
        print(
            f"LIVE_SEMANTIC_SHA256={hashlib.sha256(semantic.read_bytes()).hexdigest()}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
