#!/usr/bin/env python3
"""Generate the JDBC 4.2 semantic denominator from the pinned OpenJDK source archive."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
import tarfile
import tempfile
import urllib.request
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import NoReturn


ORACLE_TAG = "jdk8u502-b07"
ORACLE_COMMIT = "7373384725418fa645fb4508ec05a869bc7fc3e7"
ARCHIVE_URL = f"https://github.com/openjdk/jdk8u/archive/{ORACLE_COMMIT}.tar.gz"
ARCHIVE_SHA256 = "5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b"
ARCHIVE_SIZE = 94_004_963
SOURCE_MANIFEST_SHA256 = "94c4bcaeebd1681f0f2de0048ae4d95f77a96ef8d03ee4453c71358825b0d1e5"
JAVA_SOURCE_PATTERN = re.compile(
    r"(?:^|/)jdk/src/share/classes/(?P<package>java/sql|javax/sql)/"
    r"(?P<name>[A-Za-z_$][A-Za-z0-9_$]*)\.java$"
)


class GenerationError(RuntimeError):
    """Raised when the pinned source oracle or its semantic mapping drifts."""


@dataclass(frozen=True)
class JavaRecord:
    """One top-level JDBC object discovered directly from the OpenJDK archive."""

    fqcn: str
    object_name: str
    declaration_kind: str
    normalized_signature: str
    source_path: str
    source_sha256: str


@dataclass(frozen=True)
class SemanticPolicy:
    """The approved Rust disposition for one source-derived Java object."""

    rust_shape: str
    canonical_rust_entry: str
    adapter_state: str = "NOT_APPLICABLE"
    logical_state: str = "NOT_APPLICABLE"


def fail(message: str) -> NoReturn:
    """Stop generation with a stable fail-closed diagnostic."""

    raise GenerationError(message)


def strip_java_comments_and_literals(source: str) -> str:
    """Replace comments and literals with spaces while retaining declaration tokens."""

    output: list[str] = []
    index = 0
    state = "CODE"
    while index < len(source):
        if state == "CODE" and source.startswith("//", index):
            output.extend((" ", " "))
            index += 2
            state = "LINE_COMMENT"
            continue
        if state == "CODE" and source.startswith("/*", index):
            output.extend((" ", " "))
            index += 2
            state = "BLOCK_COMMENT"
            continue
        character = source[index]
        if state == "LINE_COMMENT":
            output.append("\n" if character == "\n" else " ")
            index += 1
            if character == "\n":
                state = "CODE"
            continue
        if state == "BLOCK_COMMENT":
            if source.startswith("*/", index):
                output.extend((" ", " "))
                index += 2
                state = "CODE"
            else:
                output.append("\n" if character == "\n" else " ")
                index += 1
            continue
        if state in {"STRING", "CHAR"}:
            terminator = '"' if state == "STRING" else "'"
            if character == "\\" and index + 1 < len(source):
                output.extend((" ", " "))
                index += 2
            else:
                output.append(" ")
                index += 1
                if character == terminator:
                    state = "CODE"
            continue
        if character == '"':
            output.append(" ")
            index += 1
            state = "STRING"
            continue
        if character == "'":
            output.append(" ")
            index += 1
            state = "CHAR"
            continue
        output.append(character)
        index += 1
    if state in {"BLOCK_COMMENT", "STRING", "CHAR"}:
        fail(f"unterminated Java {state.lower().replace('_', ' ')}")
    return "".join(output)


def parse_java_record(member_path: str, source: bytes) -> JavaRecord:
    """Parse one root-package JDBC source member into its top-level declaration."""

    match = JAVA_SOURCE_PATTERN.search(member_path)
    if match is None:
        fail(f"not a JDBC root-package Java source: {member_path}")
    package_name = match.group("package").replace("/", ".")
    object_name = match.group("name")
    try:
        text = source.decode("utf-8")
    except UnicodeDecodeError as error:
        fail(f"Java source is not UTF-8: {member_path}: {error}")
    stripped = strip_java_comments_and_literals(text)
    package_match = re.search(r"\bpackage\s+([A-Za-z0-9_.]+)\s*;", stripped)
    if package_match is None or package_match.group(1) != package_name:
        fail(f"Java package declaration drift: {member_path}")
    declaration = re.search(
        rf"\bpublic\s+(?:(?:abstract|final|strictfp)\s+)*"
        rf"(?P<kind>class|interface|enum)\s+{re.escape(object_name)}\b"
        rf"(?P<tail>[^{{;]*)",
        stripped,
    )
    if declaration is None:
        fail(f"public top-level declaration not found: {member_path}")
    prefix = stripped[declaration.start(): declaration.end()]
    normalized_signature = " ".join(prefix.split())
    normalized_path = member_path[member_path.index("jdk/src/share/classes/"):]
    return JavaRecord(
        fqcn=f"{package_name}.{object_name}",
        object_name=object_name,
        declaration_kind=declaration.group("kind"),
        normalized_signature=normalized_signature,
        source_path=normalized_path,
        source_sha256=hashlib.sha256(source).hexdigest(),
    )


def discover_java_records(
    archive: Path,
    *,
    enforce_jdbc42_counts: bool = True,
) -> list[JavaRecord]:
    """Discover the denominator from archive members; no object list drives enumeration."""

    if not archive.is_file():
        fail(f"OpenJDK archive not found: {archive}")
    records: list[JavaRecord] = []
    seen: set[str] = set()
    try:
        with tarfile.open(archive, "r:gz") as bundle:
            members = sorted(
                (member for member in bundle.getmembers() if JAVA_SOURCE_PATTERN.search(member.name)),
                key=lambda member: member.name,
            )
            for member in members:
                extracted = bundle.extractfile(member)
                if extracted is None:
                    fail(f"cannot read archive member: {member.name}")
                record = parse_java_record(member.name, extracted.read())
                if record.fqcn in seen:
                    fail(f"duplicate source object in oracle: {record.fqcn}")
                seen.add(record.fqcn)
                records.append(record)
    except (tarfile.TarError, OSError) as error:
        fail(f"cannot read OpenJDK archive: {error}")
    if not records:
        fail("OpenJDK archive produced no java.sql/javax.sql root-package objects")
    records.sort(key=lambda record: record.fqcn)
    if enforce_jdbc42_counts:
        java_sql = sum(record.fqcn.startswith("java.sql.") for record in records)
        javax_sql = sum(record.fqcn.startswith("javax.sql.") for record in records)
        if (java_sql, javax_sql) != (52, 17):
            fail(
                "OpenJDK JDBC root-package denominator drift: "
                f"JAVA_SQL={java_sql} JAVAX_SQL={javax_sql}"
            )
    return records


def file_digest_and_size(path: Path) -> tuple[str, int]:
    """Return a streaming SHA-256 and byte size for one archive."""

    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def verify_archive(archive: Path, requested_sha256: str) -> None:
    """Require the approved commit archive hash and size before reading source."""

    if requested_sha256 != ARCHIVE_SHA256:
        fail(
            "requested archive SHA-256 does not match the approved oracle: "
            f"{requested_sha256}"
        )
    digest, size = file_digest_and_size(archive)
    if digest != ARCHIVE_SHA256 or size != ARCHIVE_SIZE:
        fail(
            "OpenJDK archive drift: "
            f"SHA256={digest} SIZE={size}; expected {ARCHIVE_SHA256}/{ARCHIVE_SIZE}"
        )


def download_archive(destination: Path) -> None:
    """Download the immutable commit archive without adding it to the repository."""

    request = urllib.request.Request(
        ARCHIVE_URL,
        headers={"User-Agent": "druid-rust-jdbc42-inventory/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=180) as response:
            with destination.open("wb") as output:
                shutil.copyfileobj(response, output, length=1024 * 1024)
    except OSError as error:
        fail(f"cannot download pinned OpenJDK archive: {error}")


STANDARD_TRAITS = {
    "Driver", "Connection", "Statement", "PreparedStatement", "CallableStatement",
    "ResultSet", "ResultSetMetaData", "ParameterMetaData", "DatabaseMetaData",
    "SQLType", "Array", "Blob", "Clob", "NClob", "Ref", "SQLXML", "Struct",
    "SQLData", "SQLInput", "SQLOutput", "Wrapper", "CommonDataSource", "DataSource",
    "ConnectionPoolDataSource", "PooledConnection", "RowSet", "RowSetInternal",
    "RowSetMetaData", "XAConnection", "XADataSource",
}
CALLBACKS = {
    "DriverAction", "ConnectionEventListener", "StatementEventListener", "RowSetListener",
    "RowSetReader", "RowSetWriter",
}
ENUM_KINDS = {
    "ClientInfoStatus", "JDBCType", "RowIdLifetime", "PseudoColumnUsage",
    "SQLException", "SQLWarning", "BatchUpdateException", "DataTruncation",
    "SQLClientInfoException", "SQLNonTransientException", "SQLDataException",
    "SQLIntegrityConstraintViolationException", "SQLInvalidAuthorizationSpecException",
    "SQLNonTransientConnectionException", "SQLSyntaxErrorException",
    "SQLTransientException", "SQLTransientConnectionException",
    "SQLTransactionRollbackException", "SQLTimeoutException", "SQLRecoverableException",
    "SQLFeatureNotSupportedException",
}
MODULES = {"DriverManager", "Types"}


JAVA_SQL_TARGETS = {
    "Driver": "druid_rdbc::driver::Driver",
    "DriverAction": "druid_rdbc::driver::DriverAction",
    "DriverManager": "druid_rdbc::driver::driver_manager",
    "DriverPropertyInfo": "druid_rdbc::driver::DriverPropertyInfo",
    "Connection": "druid_rdbc::connection::Connection",
    "Savepoint": "druid_rdbc::connection::Savepoint",
    "ClientInfoStatus": "druid_rdbc::connection::ClientInfoStatus",
    "Statement": "druid_rdbc::statement::Statement",
    "PreparedStatement": "druid_rdbc::statement::PreparedStatement",
    "CallableStatement": "druid_rdbc::statement::CallableStatement",
    "ResultSet": "druid_rdbc::result_set::ResultSet",
    "ResultSetMetaData": "druid_rdbc::metadata::ResultSetMetaData",
    "ParameterMetaData": "druid_rdbc::metadata::ParameterMetaData",
    "DatabaseMetaData": "druid_rdbc::metadata::DatabaseMetaData",
    "SQLType": "druid_rdbc::types::SqlType",
    "JDBCType": "druid_rdbc::types::JdbcType",
    "Types": "druid_rdbc::types::types",
    "Date": "druid_rdbc::types::Date",
    "Time": "druid_rdbc::types::Time",
    "Timestamp": "druid_rdbc::types::Timestamp",
    "RowId": "druid_rdbc::types::RowId",
    "RowIdLifetime": "druid_rdbc::metadata::RowIdLifetime",
    "PseudoColumnUsage": "druid_rdbc::metadata::PseudoColumnUsage",
    "Array": "druid_rdbc::resource::Array",
    "Blob": "druid_rdbc::resource::Blob",
    "Clob": "druid_rdbc::resource::Clob",
    "NClob": "druid_rdbc::resource::NClob",
    "Ref": "druid_rdbc::resource::Ref",
    "SQLXML": "druid_rdbc::resource::SqlXml",
    "Struct": "druid_rdbc::resource::Struct",
    "SQLData": "druid_rdbc::resource::SqlData",
    "SQLInput": "druid_rdbc::resource::SqlInput",
    "SQLOutput": "druid_rdbc::resource::SqlOutput",
    "Wrapper": "druid_rdbc::wrapper::Wrapper",
    "SQLException": "druid_rdbc::error::SqlException",
    "SQLWarning": "druid_rdbc::error::SqlWarning",
    "BatchUpdateException": "druid_rdbc::error::BatchUpdateException",
    "DataTruncation": "druid_rdbc::error::DataTruncation",
    "SQLClientInfoException": "druid_rdbc::error::SqlClientInfoException",
    "SQLNonTransientException": "druid_rdbc::error::SqlExceptionKind::NonTransient",
    "SQLDataException": "druid_rdbc::error::SqlExceptionKind::Data",
    "SQLIntegrityConstraintViolationException": "druid_rdbc::error::SqlExceptionKind::IntegrityConstraintViolation",
    "SQLInvalidAuthorizationSpecException": "druid_rdbc::error::SqlExceptionKind::InvalidAuthorizationSpec",
    "SQLNonTransientConnectionException": "druid_rdbc::error::SqlExceptionKind::NonTransientConnection",
    "SQLSyntaxErrorException": "druid_rdbc::error::SqlExceptionKind::Syntax",
    "SQLTransientException": "druid_rdbc::error::SqlExceptionKind::Transient",
    "SQLTransientConnectionException": "druid_rdbc::error::SqlExceptionKind::TransientConnection",
    "SQLTransactionRollbackException": "druid_rdbc::error::SqlExceptionKind::TransactionRollback",
    "SQLTimeoutException": "druid_rdbc::error::SqlExceptionKind::Timeout",
    "SQLRecoverableException": "druid_rdbc::error::SqlExceptionKind::Recoverable",
    "SQLFeatureNotSupportedException": "druid_rdbc::error::SqlExceptionKind::FeatureNotSupported",
    "SQLPermission": "host::process_security_policy",
}


JAVAX_SQL_TARGETS = {
    "CommonDataSource": "druid_rdbc::data_source::CommonDataSource",
    "DataSource": "druid_rdbc::data_source::DataSource",
    "ConnectionPoolDataSource": "druid_rdbc::data_source::ConnectionPoolDataSource",
    "PooledConnection": "druid_rdbc::data_source::PooledConnection",
    "ConnectionEvent": "druid_rdbc::data_source::ConnectionEvent",
    "ConnectionEventListener": "druid_rdbc::data_source::ConnectionEventListener",
    "StatementEvent": "druid_rdbc::data_source::StatementEvent",
    "StatementEventListener": "druid_rdbc::data_source::StatementEventListener",
    "RowSet": "druid_rdbc::extensions::RowSet",
    "RowSetEvent": "druid_rdbc::extensions::RowSetEvent",
    "RowSetInternal": "druid_rdbc::extensions::RowSetInternal",
    "RowSetListener": "druid_rdbc::extensions::RowSetListener",
    "RowSetMetaData": "druid_rdbc::extensions::RowSetMetaData",
    "RowSetReader": "druid_rdbc::extensions::RowSetReader",
    "RowSetWriter": "druid_rdbc::extensions::RowSetWriter",
    "XAConnection": "druid_rdbc::data_source::XaConnection",
    "XADataSource": "druid_rdbc::data_source::XaDataSource",
}


ADAPTER_PLANNED = {
    "Driver", "Connection", "Statement", "PreparedStatement", "CallableStatement",
    "ResultSet", "DatabaseMetaData", "ResultSetMetaData", "ParameterMetaData", "Array",
    "Blob", "Clob", "NClob", "Ref", "SQLXML", "Struct", "SQLData", "SQLInput",
    "SQLOutput", "Wrapper", "CommonDataSource", "DataSource", "ConnectionPoolDataSource",
    "PooledConnection", "RowSet", "RowSetInternal", "RowSetMetaData", "XAConnection",
    "XADataSource",
}
LOGICAL_PLANNED = {
    "Connection", "Statement", "PreparedStatement", "CallableStatement", "ResultSet",
    "DatabaseMetaData", "ResultSetMetaData", "ParameterMetaData", "Wrapper",
    "CommonDataSource", "DataSource", "ConnectionPoolDataSource", "PooledConnection",
    "ConnectionEvent", "ConnectionEventListener", "StatementEvent", "StatementEventListener",
    "RowSet", "RowSetEvent", "RowSetInternal", "RowSetListener", "RowSetMetaData",
    "RowSetReader", "RowSetWriter", "XAConnection", "XADataSource",
}


def semantic_policy(record: JavaRecord) -> SemanticPolicy:
    """Classify one discovered source object against the approved single-standard design."""

    targets = JAVA_SQL_TARGETS if record.fqcn.startswith("java.sql.") else JAVAX_SQL_TARGETS
    target = targets.get(record.object_name)
    if target is None:
        fail(f"source-derived JDBC object has no approved semantic mapping: {record.fqcn}")
    if record.object_name == "SQLPermission":
        shape = "EXCLUDED_RUNTIME"
    elif record.object_name in CALLBACKS:
        shape = "CALLBACK"
    elif record.object_name in ENUM_KINDS:
        shape = "ENUM_KIND"
    elif record.object_name in MODULES:
        shape = "MODULE"
    elif record.object_name in STANDARD_TRAITS:
        shape = "TRAIT"
    else:
        shape = "DTO"
    return SemanticPolicy(
        rust_shape=shape,
        canonical_rust_entry=target,
        adapter_state="PLANNED" if record.object_name in ADAPTER_PLANNED else "NOT_APPLICABLE",
        logical_state="PLANNED" if record.object_name in LOGICAL_PLANNED else "NOT_APPLICABLE",
    )


def lifecycle_owner(record: JavaRecord) -> str:
    """Return the named lifecycle owner for the semantic object family."""

    name = record.object_name
    if name in {"Driver", "DriverAction", "DriverManager", "DriverPropertyInfo"}:
        return "DRIVER_REGISTRY"
    if name in {"Connection", "Savepoint", "ClientInfoStatus"}:
        return "CONNECTION_OWNER"
    if name in {"Statement", "PreparedStatement", "CallableStatement"}:
        return "STATEMENT_OWNER"
    if name in {"ResultSet", "DatabaseMetaData", "ResultSetMetaData", "ParameterMetaData"}:
        return "CURSOR_OR_METADATA_OWNER"
    if name in {"Array", "Blob", "Clob", "NClob", "Ref", "SQLXML", "Struct", "SQLData", "SQLInput", "SQLOutput"}:
        return "RESOURCE_OWNER"
    if record.fqcn.startswith("javax.sql."):
        return "DATA_SOURCE_OR_EVENT_OWNER"
    if name == "SQLPermission":
        return "HOST_SECURITY_OWNER"
    return "VALUE_OWNER"


def render_semantic(records: list[JavaRecord]) -> bytes:
    """Render deterministic Task 2 semantic TSV bytes."""

    from verify_jdbc42_semantic_inventory import SEMANTIC_HEADER, parse_semantic_payload

    lines = [SEMANTIC_HEADER]
    for record in records:
        policy = semantic_policy(record)
        disposition = "JAVA_RUNTIME_ONLY" if record.object_name == "SQLPermission" else "MATCH"
        indexed = record.object_name in {
            "Statement", "PreparedStatement", "CallableStatement", "ResultSet",
            "ResultSetMetaData", "ParameterMetaData", "DatabaseMetaData", "Array",
            "Blob", "Clob", "NClob", "Ref", "SQLXML", "Struct", "SQLData", "SQLInput",
            "SQLOutput", "RowSet", "RowSetInternal", "RowSetMetaData",
        }
        lines.append(
            "\t".join(
                (
                    f"java:{record.fqcn}#TYPE",
                    record.fqcn,
                    record.object_name,
                    policy.rust_shape,
                    disposition,
                    policy.canonical_rust_entry,
                    "PLANNED_TARGET",
                    record.normalized_signature,
                    f"{record.declaration_kind.upper()}_DECLARATION",
                    "STANDARD_1_BASED_AND_SQL_NULL" if indexed else "NOT_APPLICABLE",
                    "HOST_SECURITY_REJECTION" if record.object_name == "SQLPermission" else "RDBC_ERROR",
                    lifecycle_owner(record),
                    "-",
                    policy.adapter_state,
                    "-",
                    policy.logical_state,
                    "V0_STATIC",
                    "NOT_APPLICABLE" if record.object_name == "SQLPermission" else "PLANNED",
                    f"OpenJDK source-derived object: {record.source_path}",
                )
            )
        )
    payload = ("\n".join(lines) + "\n").encode("utf-8")
    parse_semantic_payload(payload)
    return payload


def render_oracle(records: list[JavaRecord], semantic_payload: bytes) -> bytes:
    """Render deterministic oracle metadata tied to source objects and semantic bytes."""

    packages: dict[str, dict[str, object]] = {}
    for package in ("java.sql", "javax.sql"):
        objects = [asdict(record) for record in records if record.fqcn.startswith(package + ".")]
        packages[package] = {"object_count": len(objects), "objects": objects}
    source_objects = [
        item
        for package in ("java.sql", "javax.sql")
        for item in packages[package]["objects"]
    ]
    source_manifest_sha256 = hashlib.sha256(
        (json.dumps(source_objects, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    ).hexdigest()
    if source_manifest_sha256 != SOURCE_MANIFEST_SHA256:
        fail(
            "OpenJDK source manifest drift: "
            f"{source_manifest_sha256}; expected {SOURCE_MANIFEST_SHA256}"
        )
    payload = {
        "schema_version": 1,
        "oracle_tag": ORACLE_TAG,
        "oracle_commit": ORACLE_COMMIT,
        "archive_url": ARCHIVE_URL,
        "archive_sha256": ARCHIVE_SHA256,
        "archive_size": ARCHIVE_SIZE,
        "source_manifest_sha256": source_manifest_sha256,
        "packages": packages,
        "semantic_sha256": hashlib.sha256(semantic_payload).hexdigest(),
    }
    return (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")


def write_outputs(semantic: Path, oracle: Path, records: list[JavaRecord]) -> None:
    """Write both inventories only after the complete source and mapping pass succeeds."""

    semantic_payload = render_semantic(records)
    oracle_payload = render_oracle(records, semantic_payload)
    semantic.parent.mkdir(parents=True, exist_ok=True)
    oracle.parent.mkdir(parents=True, exist_ok=True)
    semantic.write_bytes(semantic_payload)
    oracle.write_bytes(oracle_payload)
    print("JAVA_SQL=52")
    print("JAVAX_SQL=17")
    print(f"SEMANTIC_SHA256={hashlib.sha256(semantic_payload).hexdigest()}")


def parse_arguments() -> argparse.Namespace:
    """Parse the documented generator command and an optional test/cache archive."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive-sha256", required=True)
    parser.add_argument("--archive", type=Path, help="Use an already downloaded exact archive")
    parser.add_argument("--semantic", required=True, type=Path)
    parser.add_argument("--oracle", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    """Generate the checked-in source-derived semantic and oracle manifests."""

    arguments = parse_arguments()
    try:
        if arguments.archive is not None:
            archive = arguments.archive.resolve()
            verify_archive(archive, arguments.archive_sha256)
            records = discover_java_records(archive)
        else:
            with tempfile.TemporaryDirectory(prefix="druid-jdbc42-oracle-") as directory:
                archive = Path(directory) / f"jdk8u-{ORACLE_COMMIT}.tar.gz"
                download_archive(archive)
                verify_archive(archive, arguments.archive_sha256)
                records = discover_java_records(archive)
        write_outputs(arguments.semantic, arguments.oracle, records)
    except GenerationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
