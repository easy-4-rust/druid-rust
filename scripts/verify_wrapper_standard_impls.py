#!/usr/bin/env python3
"""Verify P04 Task 4 wrapper standard identity and typed runtime-trace evidence."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
import selectors
import signal
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import tokenize


DEFAULT_ADAPTERS = Path("docs/verification/rdbc-first/rdbc-adapter-implementations.tsv")
DEFAULT_RUNTIME_EVIDENCE = Path(
    "docs/verification/rdbc-first/rdbc-adapter-runtime-traces.tsv"
)
DEFAULT_COVERAGE_MANIFEST = Path(
    "docs/verification/rdbc-first/rdbc-adapter-coverage-regions.json"
)
DEFAULT_CONSUMERS = Path("docs/verification/rdbc-first/rdbc-production-consumers.tsv")
TASK = "P04_TASK_4_ADAPTER_CUTOVER"
LIVE_ADAPTER_STATE = "STANDARD_IMPL_LIVE"
LIVE_CONSUMER_STATE = "STANDARD_REFERENCE_CLOSED"
RUNTIME_TRACE_CATEGORY = "RUNTIME_TRACE"
COMPILE_IDENTITY_CATEGORY = "COMPILE_IDENTITY"
TRACE_VERSION = "task4-runtime-trace-v1"
TRACE_MARKER = "DRUID_TASK4_RUNTIME_TRACE"
TRACE_CHALLENGE_ENV = "DRUID_TASK4_TRACE_CHALLENGE"
COVERAGE_VERSION = "task4-compiler-coverage-v2"
LLVM_COVERAGE_JSON_TYPE = "llvm.coverage.json.export"
LLVM_COVERAGE_JSON_VERSION = "3.1.0"
DEFAULT_EVIDENCE_TIMEOUT_SECONDS = 600.0
DEFAULT_EVIDENCE_OUTPUT_LIMIT = 1_048_576
DEFAULT_COVERAGE_REPORT_LIMIT = 33_554_432
RUNTIME_EVIDENCE_HEADER = (
    "source_path",
    "target_trait",
    "behavior_evidence",
    "cargo_test_target",
    "cargo_test_id",
    "behavior_profile",
    "evidence_category",
    "trace_family",
    "trace_version",
    "trace_evidence_id",
    "coverage_version",
    "coverage_requirement_ids",
)
TRACE_FAMILIES = {
    "SQLX",
    "TOASTY",
    "RBDC",
    "DUCKDB",
    "HTTP_SQL",
    "LIBSQL",
    "JDBC_AGENT",
}
FAMILY_FACTORY_FILES = {
    "SQLX": "crates/druid-wrapper/src/sqlx/sqlx_connection_factory.rs",
    "TOASTY": "crates/druid-wrapper/src/toasty/toasty_connection_factory.rs",
    "RBDC": "crates/druid-wrapper/src/rbdc/rbdc_connection_factory.rs",
    "DUCKDB": "crates/druid-wrapper/src/duckdb/duckdb_connection_factory.rs",
    "HTTP_SQL": "crates/druid-wrapper/src/http_sql/http_sql_connection_factory.rs",
    "LIBSQL": "crates/druid-wrapper/src/libsql/libsql_connection_factory.rs",
    "JDBC_AGENT": "crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection_factory.rs",
}
TRACE_STAGES = (
    "FactoryCreate",
    "ConnectionCreate",
    "StatementCreate",
    "StatementUpdate",
    "PreparedCreate",
    "ParameterSet",
    "PreparedQuery",
    "ResultNext",
    "ResultSetMetadata",
    "DatabaseMetadata",
    "ConnectionClose",
)
COVERAGE_REQUIREMENT_IDS = (
    "EXACT_TEST_IDENTITY",
    "FAMILY_FACTORY_FUNCTION",
    "FAMILY_EXECUTOR_FUNCTION",
    "STAGE_FACTORY_CREATE",
    "STAGE_CONNECTION_CREATE",
    "STAGE_STATEMENT_CREATE",
    "STAGE_STATEMENT_UPDATE",
    "STAGE_PREPARED_CREATE",
    "STAGE_PARAMETER_SET",
    "STAGE_PREPARED_QUERY",
    "STAGE_RESULT_NEXT",
    "STAGE_RESULT_SET_METADATA",
    "STAGE_DATABASE_METADATA",
    "STAGE_CONNECTION_CLOSE",
    "TRACE_VALIDATION_SUCCESS",
)
COVERAGE_REQUIREMENT_SET = ",".join(COVERAGE_REQUIREMENT_IDS)
COVERAGE_FAMILY_ORDER = tuple(sorted(TRACE_FAMILIES))
COVERAGE_REQUIREMENT_KINDS = {
    "TEST_IDENTITY_FUNCTION",
    "FAMILY_FACTORY_FUNCTION",
    "FAMILY_EXECUTOR_FUNCTION",
    "STAGE_ANCHOR_FUNCTION",
    "TRACE_VALIDATION_SUCCESS_FUNCTION",
}
TRACE_STAGE_METHODS = {
    "FactoryCreate": "task4_trace_factory_create",
    "ConnectionCreate": "task4_trace_connection_create",
    "StatementCreate": "task4_trace_statement_create",
    "StatementUpdate": "task4_trace_statement_update",
    "PreparedCreate": "task4_trace_prepared_create",
    "ParameterSet": "task4_trace_parameter_set",
    "PreparedQuery": "task4_trace_prepared_query",
    "ResultNext": "task4_trace_result_next",
    "ResultSetMetadata": "task4_trace_result_set_metadata",
    "DatabaseMetadata": "task4_trace_database_metadata",
    "ConnectionClose": "task4_trace_connection_close",
}
TRACE_RECORD_METHODS = {
    f"task4_trace_{name}"
    for name in (
        "factory_create",
        "connection_create",
        "statement_create",
        "statement_update",
        "prepared_create",
        "parameter_set",
        "prepared_query",
        "result_next",
        "result_set_metadata",
        "database_metadata",
        "connection_close",
    )
}
TRACE_RECORD_CALLSITE_COUNTS = {
    (
        "crates/druid-wrapper/src/standard_adapter_statement.rs",
        "task4_trace_statement_create",
    ): 1,
    (
        "crates/druid-wrapper/src/standard_adapter_statement.rs",
        "task4_trace_statement_update",
    ): 2,
    (
        "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
        "task4_trace_prepared_create",
    ): 1,
    (
        "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
        "task4_trace_parameter_set",
    ): 2,
    (
        "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
        "task4_trace_prepared_query",
    ): 2,
    (
        "crates/druid-wrapper/src/standard_adapter_result_set.rs",
        "task4_trace_result_next",
    ): 1,
    (
        "crates/druid-wrapper/src/standard_adapter_result_set.rs",
        "task4_trace_result_set_metadata",
    ): 1,
    (
        "crates/druid-wrapper/src/standard_adapter_database_meta_data.rs",
        "task4_trace_database_metadata",
    ): 1,
    (
        "crates/druid-wrapper/src/standard_adapter_executor.rs",
        "task4_trace_connection_close",
    ): 1,
    **{
        ("crates/druid-wrapper/src/standard_adapter_executor.rs", method): 1
        for method in TRACE_RECORD_METHODS
        if method
        not in {
            "task4_trace_factory_create",
            "task4_trace_connection_create",
            "task4_trace_connection_close",
        }
    },
    (
        "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
        "task4_trace_statement_create",
    ): 1,
    (
        "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
        "task4_trace_statement_update",
    ): 1,
    (
        "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
        "task4_trace_prepared_create",
    ): 1,
    (
        "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
        "task4_trace_parameter_set",
    ): 2,
    (
        "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
        "task4_trace_prepared_query",
    ): 1,
    (
        "crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs",
        "task4_trace_result_next",
    ): 1,
    (
        "crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs",
        "task4_trace_result_set_metadata",
    ): 1,
    (
        "crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs",
        "task4_trace_database_metadata",
    ): 1,
    (
        "crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs",
        "task4_trace_connection_close",
    ): 1,
    (
        "crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs",
        "task4_trace_database_metadata",
    ): 2,
    **{
        ("crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs", method): 1
        for method in TRACE_RECORD_METHODS
        if method
        not in {
            "task4_trace_factory_create",
            "task4_trace_connection_create",
            "task4_trace_connection_close",
        }
    },
    **{
        (factory_file, "task4_trace_factory_create"): 1
        for factory_file in FAMILY_FACTORY_FILES.values()
    },
    **{
        (factory_file, "task4_trace_connection_create"): (
            2 if family == "LIBSQL" else 1
        )
        for family, factory_file in FAMILY_FACTORY_FILES.items()
    },
}
TRACE_RECORD_CALLSITE_COUNTS.update({
    ("crates/druid-wrapper/src/sqlx/sqlx_async_result_set.rs", "task4_trace_result_next"): 1,
    ("crates/druid-wrapper/src/sqlx/sqlx_async_result_set.rs", "task4_trace_result_set_metadata"): 1,
    ("crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs", "task4_trace_result_next"): 1,
    ("crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs", "task4_trace_result_set_metadata"): 1,
})
TRACE_BRANCH_CALLSITE_COUNTS = {
    **{
        (relative, function, "task4_trace_parameter_set"): 1
        for relative in (
            "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
            "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
        )
        for function in ("set_input_parameter", "set_parameter")
    },
    (
        "crates/druid-wrapper/src/standard_adapter_statement.rs",
        "execute_update",
        "task4_trace_statement_update",
    ): 1,
    (
        "crates/druid-wrapper/src/standard_adapter_statement.rs",
        "execute",
        "task4_trace_statement_update",
    ): 1,
    (
        "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
        "execute_prepared_query",
        "task4_trace_prepared_query",
    ): 1,
    (
        "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
        "execute_prepared",
        "task4_trace_prepared_query",
    ): 1,
    (
        "crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs",
        "database_meta_data",
        "task4_trace_database_metadata",
    ): 2,
}
COMMON_STAGE_FILES = {
    "StatementCreate": "crates/druid-wrapper/src/standard_adapter_statement.rs",
    "StatementUpdate": "crates/druid-wrapper/src/standard_adapter_statement.rs",
    "PreparedCreate": "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
    "ParameterSet": "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
    "PreparedQuery": "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
    "ResultNext": "crates/druid-wrapper/src/standard_adapter_result_set.rs",
    "ResultSetMetadata": "crates/druid-wrapper/src/standard_adapter_result_set.rs",
    "DatabaseMetadata": "crates/druid-wrapper/src/standard_adapter_database_meta_data.rs",
    "ConnectionClose": "crates/druid-wrapper/src/standard_adapter_executor.rs",
}
SQLX_STAGE_FILES = {
    "StatementCreate": "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
    "StatementUpdate": "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
    "PreparedCreate": "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
    "ParameterSet": "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
    "PreparedQuery": "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
    "ResultNext": "crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs",
    "ResultSetMetadata": "crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs",
    "DatabaseMetadata": "crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs",
    "ConnectionClose": "crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs",
}
COMMON_STAGE_FUNCTIONS = {
    "StatementCreate": "new",
    "StatementUpdate": "execute_update",
    "PreparedCreate": "new",
    "ParameterSet": "set_parameter",
    "PreparedQuery": "execute_prepared_query",
    "ResultNext": "next",
    "ResultSetMetadata": "meta_data",
    "DatabaseMetadata": "database_product_name",
    "ConnectionClose": "close",
}
SQLX_STAGE_FUNCTIONS = {
    "StatementCreate": "owned_statement",
    "StatementUpdate": "execute_update",
    "PreparedCreate": "owned_prepared_statement",
    "ParameterSet": "set_parameter",
    "PreparedQuery": "execute_prepared_query",
    "ResultNext": "next",
    "ResultSetMetadata": "meta_data",
    "DatabaseMetadata": "database_product_name",
    "ConnectionClose": "close",
}
TRACE_CALLSITE_RULES = {
    family: {
        "FactoryCreate": factory_file,
        "ConnectionCreate": factory_file,
        **(SQLX_STAGE_FILES if family == "SQLX" else COMMON_STAGE_FILES),
    }
    for family, factory_file in FAMILY_FACTORY_FILES.items()
}
TRACE_CALLSITE_FUNCTIONS = {
    family: {
        "FactoryCreate": "create_adapter_core",
        "ConnectionCreate": "create_adapter_core",
        **(SQLX_STAGE_FUNCTIONS if family == "SQLX" else COMMON_STAGE_FUNCTIONS),
    }
    for family in FAMILY_FACTORY_FILES
}
COVERAGE_STAGE_REQUIREMENTS = {
    "STAGE_FACTORY_CREATE": "FactoryCreate",
    "STAGE_CONNECTION_CREATE": "ConnectionCreate",
    "STAGE_STATEMENT_CREATE": "StatementCreate",
    "STAGE_STATEMENT_UPDATE": "StatementUpdate",
    "STAGE_PREPARED_CREATE": "PreparedCreate",
    "STAGE_PARAMETER_SET": "ParameterSet",
    "STAGE_PREPARED_QUERY": "PreparedQuery",
    "STAGE_RESULT_NEXT": "ResultNext",
    "STAGE_RESULT_SET_METADATA": "ResultSetMetadata",
    "STAGE_DATABASE_METADATA": "DatabaseMetadata",
    "STAGE_CONNECTION_CLOSE": "ConnectionClose",
}
FAMILY_EXECUTOR_RULES = {
    "SQLX": (
        "crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs",
        "execute_update_result",
    ),
    "TOASTY": (
        "crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs",
        "standard_execute_update_result_core",
    ),
    "RBDC": (
        "crates/druid-wrapper/src/rbdc/rbdc_connection_adapter.rs",
        "standard_execute_update_core",
    ),
    "DUCKDB": (
        "crates/druid-wrapper/src/duckdb/duckdb_connection_adapter.rs",
        "standard_execute_update_core",
    ),
    "HTTP_SQL": (
        "crates/druid-wrapper/src/http_sql/http_sql_connection_adapter.rs",
        "standard_execute_update_core",
    ),
    "LIBSQL": (
        "crates/druid-wrapper/src/libsql/libsql_connection_adapter.rs",
        "standard_execute_update_core",
    ),
    "JDBC_AGENT": (
        "crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection.rs",
        "standard_execute_update_core",
    ),
}
TRACE_COVERAGE_ANCHOR_FUNCTIONS = {
    "FactoryCreate": "task4_coverage_anchor_factory_create",
    "ConnectionCreate": "task4_coverage_anchor_connection_create",
    "StatementCreate": "task4_coverage_anchor_statement_create",
    "StatementUpdate": "task4_coverage_anchor_statement_update",
    "PreparedCreate": "task4_coverage_anchor_prepared_create",
    "ParameterSet": "task4_coverage_anchor_parameter_set",
    "PreparedQuery": "task4_coverage_anchor_prepared_query",
    "ResultNext": "task4_coverage_anchor_result_next",
    "ResultSetMetadata": "task4_coverage_anchor_result_set_metadata",
    "DatabaseMetadata": "task4_coverage_anchor_database_metadata",
    "ConnectionClose": "task4_coverage_anchor_connection_close",
}
TRACE_VALIDATION_SUCCESS_ANCHOR = "task4_coverage_anchor_validation_success"
OUTPUT_MACROS = {"print", "println", "eprint", "eprintln", "dbg"}
DIRECT_OUTPUT_IDENTIFIERS = {
    "stdout",
    "stderr",
    "write_all",
    "write_fmt",
    "from_raw_fd",
    "as_raw_fd",
}
CANONICAL_EVIDENCE_BINDINGS = {
    "SQLX": (
        "test:statement_executor_ownership_test",
        "sqlx_standard_factory_emits_typed_runtime_trace",
        "default",
        "statement_executor_ownership_test::sqlx_standard_factory_emits_typed_runtime_trace",
        "task4.sqlx.runtime-trace-v1",
    ),
    "TOASTY": (
        "test:toasty_connection_adapter_test",
        "toasty_standard_factory_emits_typed_runtime_trace",
        "default",
        "toasty_connection_adapter_test::toasty_standard_factory_emits_typed_runtime_trace",
        "task4.toasty.runtime-trace-v1",
    ),
    "RBDC": (
        "test:rbdc_connection_adapter_test",
        "rbdc_standard_factory_emits_typed_runtime_trace",
        "default",
        "rbdc_connection_adapter_test::rbdc_standard_factory_emits_typed_runtime_trace",
        "task4.rbdc.runtime-trace-v1",
    ),
    "DUCKDB": (
        "test:duckdb_connection_adapter_test",
        "duckdb_standard_factory_emits_typed_runtime_trace",
        "all-features",
        "duckdb_connection_adapter_test::duckdb_standard_factory_emits_typed_runtime_trace",
        "task4.duckdb.runtime-trace-v1",
    ),
    "HTTP_SQL": (
        "lib",
        "http_sql::http_sql_connection_factory::tests::http_sql_standard_factory_uses_deterministic_transport_owned_core",
        "all-features",
        "http_sql::http_sql_connection_factory::tests::http_sql_standard_factory_uses_deterministic_transport_owned_core",
        "task4.http-sql.runtime-trace-v1",
    ),
    "LIBSQL": (
        "lib",
        "libsql::libsql_connection_factory::tests::libsql_standard_factory_uses_deterministic_transport_owned_core",
        "all-features",
        "libsql::libsql_connection_factory::tests::libsql_standard_factory_uses_deterministic_transport_owned_core",
        "task4.libsql.runtime-trace-v1",
    ),
    "JDBC_AGENT": (
        "lib",
        "jdbc_agent::jdbc_agent_connection_factory::tests::jdbc_agent_standard_factory_uses_deterministic_runtime_owned_core",
        "all-features",
        "jdbc_agent::jdbc_agent_connection_factory::tests::jdbc_agent_standard_factory_uses_deterministic_runtime_owned_core",
        "task4.jdbc-agent.runtime-trace-v1",
    ),
}
IDENTITY_EVIDENCE_BINDING = (
    "test:standard_adapter_conformance_test",
    "catalog_data_sources_implement_canonical_wrapper_identity",
    "default",
    "standard_adapter_conformance_test::catalog_data_sources_implement_canonical_wrapper_identity",
)
SQL_EXECUTION_MARKERS = (
    "sqlx::query",
    ".fetch_all(",
    ".fetch_one(",
    ".execute(",
    "RdbcError::DriverError",
    "RdbcError::UnsupportedOperation",
)


class VerificationError(RuntimeError):
    """Raised when Task 4 evidence is absent, duplicated, forged, or stale."""


def fail(message: str) -> NoReturn:
    """Stop with one stable fail-closed diagnostic."""

    raise VerificationError(message)


@dataclass(frozen=True)
class AdapterRow:
    """One Task 4 production adapter obligation and its exact evidence binding."""

    source_path: Path
    current_trait: str
    implementation_type: str
    target_trait: str
    standard_implementation_type: str
    standard_source_path: Path
    standard_core_evidence: str
    behavior_evidence: str
    state: str
    behavior_test_target: str = "test:demo"
    cargo_test_id: str = "runs"
    behavior_profile: str = "default"
    evidence_category: str = RUNTIME_TRACE_CATEGORY
    trace_family: str = "HTTP_SQL"
    trace_version: str = TRACE_VERSION
    trace_evidence_id: str = "task4.fixture.runtime-trace-v1"
    coverage_version: str = COVERAGE_VERSION
    coverage_requirement_ids: str = COVERAGE_REQUIREMENT_SET


@dataclass(frozen=True, order=True)
class EvidenceSpec:
    """One unique exact Cargo test and its expected trace/identity protocol."""

    test_target: str
    test_id: str
    profile: str
    category: str
    family: str
    version: str
    evidence_id: str
    coverage_version: str = COVERAGE_VERSION


@dataclass(frozen=True)
class CoverageRequirement:
    """One exact compiler function obligation bound to checked Rust source."""

    requirement_id: str
    requirement_kind: str
    source_path: Path
    source_sha256: str
    source_function: str
    compiler_function_name: str
    anchor_id: str
    minimum_count: int


@dataclass(frozen=True)
class CoverageFamilySpec:
    """Compiler-coverage contract for one exact family evidence test."""

    family: str
    test_target: str
    test_id: str
    profile: str
    trace_version: str
    trace_evidence_id: str
    coverage_version: str
    requirements: tuple[CoverageRequirement, ...]


@dataclass(frozen=True)
class BoundedProcessResult:
    """One bounded child-process result with capped decoded output."""

    returncode: int
    stdout: str
    stderr: str


def read_tsv(path: Path) -> list[dict[str, str]]:
    """Read one canonical TSV while rejecting malformed rows."""

    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n") or b"\r" in raw:
        fail(f"TSV must have exactly one LF final newline: {path}")
    rows = list(csv.DictReader(raw.decode("utf-8").splitlines(), delimiter="\t"))
    if not rows or any(
        None in row or any(value == "" for value in row.values()) for row in rows
    ):
        fail(f"TSV contains an empty or malformed row: {path}")
    return rows


def task_adapter_rows(path: Path, evidence_path: Path) -> list[AdapterRow]:
    """Load exactly the production wrapper rows assigned to Task 4."""

    raw_rows = read_tsv(path)
    evidence_rows = read_tsv(evidence_path)
    if tuple(evidence_rows[0]) != RUNTIME_EVIDENCE_HEADER:
        fail("Task 4 runtime evidence TSV header is not exact")
    evidence_order = [
        (row["source_path"], row["target_trait"]) for row in evidence_rows
    ]
    if evidence_order != sorted(evidence_order):
        fail("Task 4 runtime evidence TSV is not canonically sorted")
    evidence_by_key = {
        (row["source_path"], row["target_trait"]): row for row in evidence_rows
    }
    if len(evidence_by_key) != 31 or len(evidence_rows) != 31:
        fail("Task 4 runtime evidence TSV must contain 31 unique source/target rows")
    selected = [
        row
        for row in raw_rows
        if row["target_task"] == TASK and row["owner"] == "WRAPPER_STANDARD"
    ]
    selected_keys = {(row["source_path"], row["target_trait"]) for row in selected}
    if set(evidence_by_key) != selected_keys:
        fail("Task 4 runtime evidence TSV does not exactly cover adapter rows")
    rows = [
        AdapterRow(
            source_path=Path(row["source_path"]),
            current_trait=row["current_trait"],
            implementation_type=row["implementation_type"],
            target_trait=row["target_trait"],
            standard_implementation_type=row["standard_implementation_type"],
            standard_source_path=Path(row["standard_source_path"]),
            standard_core_evidence=row["standard_core_evidence"],
            behavior_evidence=row["behavior_evidence"],
            state=row["state"],
            behavior_test_target=evidence_by_key[
                (row["source_path"], row["target_trait"])
            ]["cargo_test_target"],
            cargo_test_id=evidence_by_key[(row["source_path"], row["target_trait"])][
                "cargo_test_id"
            ],
            behavior_profile=evidence_by_key[(row["source_path"], row["target_trait"])][
                "behavior_profile"
            ],
            evidence_category=evidence_by_key[
                (row["source_path"], row["target_trait"])
            ]["evidence_category"],
            trace_family=evidence_by_key[(row["source_path"], row["target_trait"])][
                "trace_family"
            ],
            trace_version=evidence_by_key[(row["source_path"], row["target_trait"])][
                "trace_version"
            ],
            trace_evidence_id=evidence_by_key[
                (row["source_path"], row["target_trait"])
            ]["trace_evidence_id"],
            coverage_version=evidence_by_key[(row["source_path"], row["target_trait"])][
                "coverage_version"
            ],
            coverage_requirement_ids=evidence_by_key[
                (row["source_path"], row["target_trait"])
            ]["coverage_requirement_ids"],
        )
        for row in selected
    ]
    if len(rows) != 31:
        fail(f"Task 4 adapter denominator drift: expected 31 rows, found {len(rows)}")
    for row, raw_row in zip(rows, selected, strict=True):
        evidence = evidence_by_key[(raw_row["source_path"], raw_row["target_trait"])]
        if evidence["behavior_evidence"] != raw_row["behavior_evidence"]:
            fail(
                "runtime evidence TSV does not match adapter behavior evidence: "
                f"{row.source_path}"
            )
        if (
            evidence["cargo_test_id"] != row.behavior_evidence.rsplit("::", 1)[-1]
            and row.behavior_test_target != "lib"
        ):
            fail(f"integration evidence cargo id is not exact: {row.behavior_evidence}")
    return rows


def concrete_type(type_name: str) -> str:
    """Return the concrete Rust identifier recorded by the inventory."""

    return re.split(r"[<' ]", type_name, maxsplit=1)[0]


def extract_impl_bodies(source: str, trait_name: str, type_name: str) -> list[str]:
    """Extract explicit impl bodies with balanced braces."""

    leaf = trait_name.rsplit("::", 1)[-1]
    concrete = concrete_type(type_name)
    header = re.compile(
        rf"\bimpl(?:\s*<[^{{;]*>)?\s+(?:[A-Za-z_][\w]*::)*{re.escape(leaf)}"
        rf"\s+for\s+{re.escape(concrete)}(?:\s*<[^{{;]*>)?(?:\s+where\s+[^{{]*)?\s*\{{",
        re.MULTILINE,
    )
    bodies: list[str] = []
    for match in header.finditer(source):
        depth = 1
        index = match.end()
        while index < len(source) and depth:
            depth += source[index] == "{"
            depth -= source[index] == "}"
            index += 1
        if depth:
            fail(f"unbalanced impl block for {concrete}: {leaf}")
        bodies.append(source[match.end() : index - 1])
    return bodies


def code_tokens(source: str) -> str:
    """Return executable Rust tokens without comments or literal contents."""

    return " ".join(token.text for token in tokenize(source))


def core_calls(body: str) -> set[str]:
    """Return private/inherent core calls made by one impl body."""

    return set(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*_core)\s*\(", body))


def mask_rust_comments_and_literals(source: str) -> str:
    """Blank comments and literals while preserving byte offsets and newlines."""

    masked = list(source)

    def blank(start: int, end: int) -> None:
        for offset in range(start, end):
            if masked[offset] not in {"\n", "\r"}:
                masked[offset] = " "

    def quoted_end(start: int, quote: str) -> int:
        cursor = start + 1
        escaped = False
        while cursor < len(source):
            character = source[cursor]
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                return cursor + 1
            cursor += 1
        fail("unterminated Rust literal while resolving caller region")

    index = 0
    while index < len(source):
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            end = len(source) if end < 0 else end
            blank(index, end)
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            cursor = index + 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                fail("unterminated Rust block comment while resolving caller region")
            blank(index, cursor)
            index = cursor
            continue
        raw_prefix = None
        if source.startswith("br", index):
            raw_prefix = 2
        elif source.startswith("r", index):
            raw_prefix = 1
        if raw_prefix is not None:
            cursor = index + raw_prefix
            hashes = 0
            while cursor < len(source) and source[cursor] == "#":
                hashes += 1
                cursor += 1
            if cursor < len(source) and source[cursor] == '"':
                terminator = '"' + "#" * hashes
                end = source.find(terminator, cursor + 1)
                if end < 0:
                    fail("unterminated Rust raw string while resolving caller region")
                end += len(terminator)
                blank(index, end)
                index = end
                continue
        if source.startswith('b"', index):
            end = quoted_end(index + 1, '"')
            blank(index, end)
            index = end
            continue
        if source[index] == '"':
            end = quoted_end(index, '"')
            blank(index, end)
            index = end
            continue
        if source[index] == "'":
            cursor = index + 1
            if cursor < len(source) and (
                source[cursor].isalpha() or source[cursor] == "_"
            ):
                cursor += 1
                while cursor < len(source) and (
                    source[cursor].isalnum() or source[cursor] == "_"
                ):
                    cursor += 1
                if cursor >= len(source) or source[cursor] != "'":
                    index = cursor
                    continue
            end = quoted_end(index, "'")
            blank(index, end)
            index = end
            continue
        index += 1
    return "".join(masked)


def function_line_ranges(
    source: str, function_name: str
) -> tuple[tuple[int, int], ...]:
    """Return brace-balanced line regions for every function with one exact name."""

    masked = mask_rust_comments_and_literals(source)
    ranges: list[tuple[int, int]] = []
    pattern = re.compile(rf"\bfn\s+{re.escape(function_name)}\s*(?:<[^{{;]*>)?\(")
    for match in pattern.finditer(masked):
        opening = masked.find("{", match.end())
        if opening < 0:
            continue
        semicolon = masked.find(";", match.end(), opening)
        if semicolon >= 0:
            continue
        depth = 1
        cursor = opening + 1
        while cursor < len(masked) and depth:
            depth += masked[cursor] == "{"
            depth -= masked[cursor] == "}"
            cursor += 1
        if depth:
            fail(f"unbalanced function body while resolving {function_name}")
        start_line = masked.count("\n", 0, match.start()) + 1
        end_line = masked.count("\n", 0, cursor) + 1
        ranges.append((start_line, end_line))
    return tuple(ranges)


def evidence_test_source_path(family: str) -> Path:
    """Return the exact Rust source that owns one manifest-bound evidence test."""

    target, _, _, _, _ = CANONICAL_EVIDENCE_BINDINGS[family]
    if target == "lib":
        return Path(FAMILY_FACTORY_FILES[family])
    if not target.startswith("test:"):
        fail(f"coverage evidence target is unknown: {family}: {target}")
    return Path("crates/druid-wrapper/tests") / f"{target.removeprefix('test:')}.rs"


def canonical_coverage_requirement(
    family: str, requirement_id: str
) -> tuple[str, Path, str, str]:
    """Map one obligation to exact kind/source function/stable anchor identity."""

    if requirement_id == "EXACT_TEST_IDENTITY":
        return (
            "TEST_IDENTITY_FUNCTION",
            evidence_test_source_path(family),
            CANONICAL_EVIDENCE_BINDINGS[family][1].rsplit("::", 1)[-1],
            f"test_identity.{family.lower()}",
        )
    if requirement_id == "FAMILY_FACTORY_FUNCTION":
        return (
            "FAMILY_FACTORY_FUNCTION",
            Path(FAMILY_FACTORY_FILES[family]),
            "create_adapter_core",
            f"family_factory.{family.lower()}",
        )
    if requirement_id == "FAMILY_EXECUTOR_FUNCTION":
        path, source_function = FAMILY_EXECUTOR_RULES[family]
        return (
            "FAMILY_EXECUTOR_FUNCTION",
            Path(path),
            source_function,
            f"family_executor.{family.lower()}",
        )
    if requirement_id == "TRACE_VALIDATION_SUCCESS":
        return (
            "TRACE_VALIDATION_SUCCESS_FUNCTION",
            Path("crates/druid-wrapper/src/task4_runtime_trace.rs"),
            TRACE_VALIDATION_SUCCESS_ANCHOR,
            "typed_validation.success",
        )
    stage = COVERAGE_STAGE_REQUIREMENTS.get(requirement_id)
    if stage is None:
        fail(f"coverage requirement id is unknown: {requirement_id}")
    return (
        "STAGE_ANCHOR_FUNCTION",
        Path("crates/druid-wrapper/src/task4_runtime_trace.rs"),
        TRACE_COVERAGE_ANCHOR_FUNCTIONS[stage],
        f"typed_stage.{stage}",
    )


def validate_coverage_requirement_binding(
    family: str, requirement: CoverageRequirement
) -> None:
    """Reject any source/function alias for one canonical coverage obligation."""

    expected_kind, expected_path, expected_function, expected_anchor = (
        canonical_coverage_requirement(family, requirement.requirement_id)
    )
    if (
        requirement.requirement_kind != expected_kind
        or requirement.source_path != expected_path
        or requirement.source_function != expected_function
        or requirement.anchor_id != expected_anchor
    ):
        fail(
            "coverage requirement kind/source/function/anchor is not canonical: "
            f"{family}:{requirement.requirement_id}:"
            f"{requirement.requirement_kind}:{requirement.source_path}:"
            f"{requirement.source_function}:{requirement.anchor_id} != "
            f"{expected_kind}:{expected_path}:{expected_function}:{expected_anchor}"
        )


def canonical_source_sha256(raw: bytes) -> str:
    """Hash source identity independent of Git's LF/CRLF checkout conversion."""

    return hashlib.sha256(raw.replace(b"\r\n", b"\n")).hexdigest()


def validate_coverage_manifest(
    root: Path, family_specs: tuple[CoverageFamilySpec, ...]
) -> None:
    """Validate canonical order and exact compiler/source function identities."""

    families = tuple(spec.family for spec in family_specs)
    if families != COVERAGE_FAMILY_ORDER:
        fail(
            "coverage manifest family order or duplicate denominator is invalid: "
            f"{families} != {COVERAGE_FAMILY_ORDER}"
        )
    if len(set(families)) != len(families):
        fail("coverage manifest contains a duplicate family mapping")
    for family_spec in family_specs:
        family = family_spec.family
        target, test_id, profile, _, evidence_id = CANONICAL_EVIDENCE_BINDINGS[family]
        actual_binding = (
            family_spec.test_target,
            family_spec.test_id,
            family_spec.profile,
            family_spec.trace_version,
            family_spec.trace_evidence_id,
            family_spec.coverage_version,
        )
        expected_binding = (
            target,
            test_id,
            profile,
            TRACE_VERSION,
            evidence_id,
            COVERAGE_VERSION,
        )
        if actual_binding != expected_binding:
            fail(
                "coverage manifest exact test/profile/evidence binding is invalid: "
                f"{family}: {actual_binding!r} != {expected_binding!r}"
            )
        requirement_ids = tuple(
            requirement.requirement_id for requirement in family_spec.requirements
        )
        if requirement_ids != COVERAGE_REQUIREMENT_IDS:
            fail(
                "coverage manifest requirement order or duplicate mapping is invalid: "
                f"{family}: {requirement_ids} != {COVERAGE_REQUIREMENT_IDS}"
            )
        compiler_names: set[str] = set()
        for requirement in family_spec.requirements:
            if requirement.minimum_count != 1:
                fail(
                    "coverage requirement minimum count must be exactly one: "
                    f"{family}:{requirement.requirement_id}"
                )
            if requirement.requirement_kind not in COVERAGE_REQUIREMENT_KINDS:
                fail(
                    "coverage requirement kind is unknown: "
                    f"{family}:{requirement.requirement_id}:"
                    f"{requirement.requirement_kind}"
                )
            validate_coverage_requirement_binding(family, requirement)
            if not requirement.compiler_function_name or any(
                character.isspace() for character in requirement.compiler_function_name
            ):
                fail(
                    "coverage compiler function identity is empty or malformed: "
                    f"{family}:{requirement.requirement_id}"
                )
            if requirement.compiler_function_name in compiler_names:
                fail(
                    "coverage compiler function identity is duplicate or ambiguous: "
                    f"{family}:{requirement.compiler_function_name}"
                )
            compiler_names.add(requirement.compiler_function_name)
            path = root / requirement.source_path
            if not path.is_file():
                fail(
                    f"coverage requirement source is missing: {requirement.source_path}"
                )
            raw = path.read_bytes()
            observed_sha = canonical_source_sha256(raw)
            if observed_sha != requirement.source_sha256:
                fail(
                    "coverage requirement source hash is stale: "
                    f"{requirement.source_path}: {observed_sha} != "
                    f"{requirement.source_sha256}"
                )
            source_ranges = function_line_ranges(
                raw.decode("utf-8"), requirement.source_function
            )
            if len(source_ranges) != 1:
                fail(
                    "coverage source function is missing or ambiguous: "
                    f"{family}:{requirement.requirement_id}:"
                    f"{requirement.source_path}:{requirement.source_function}:"
                    f"{source_ranges}"
                )


def load_coverage_manifest(
    root: Path, path: Path = DEFAULT_COVERAGE_MANIFEST
) -> tuple[CoverageFamilySpec, ...]:
    """Load the canonical compiler-coverage manifest and validate all source identities."""

    manifest_path = root / path
    raw = manifest_path.read_bytes()
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n") or b"\r" in raw:
        fail(f"coverage manifest must have exactly one LF final newline: {path}")
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as error:
        fail(f"coverage manifest JSON is malformed: {error}")
    if not isinstance(document, dict) or set(document) != {
        "schema_version",
        "families",
    }:
        fail("coverage manifest root schema is not exact")
    if document["schema_version"] != COVERAGE_VERSION:
        fail("coverage manifest schema version is not canonical")
    raw_families = document["families"]
    if not isinstance(raw_families, list):
        fail("coverage manifest families must be an ordered list")
    family_specs: list[CoverageFamilySpec] = []
    for raw_family in raw_families:
        family_keys = {
            "family",
            "test_target",
            "test_id",
            "profile",
            "trace_version",
            "trace_evidence_id",
            "coverage_version",
            "requirements",
        }
        if not isinstance(raw_family, dict) or set(raw_family) != family_keys:
            fail("coverage manifest family schema is not exact")
        raw_requirements = raw_family["requirements"]
        if not isinstance(raw_requirements, list):
            fail("coverage manifest requirements must be an ordered list")
        requirements: list[CoverageRequirement] = []
        for raw_requirement in raw_requirements:
            requirement_keys = {
                "requirement_id",
                "requirement_kind",
                "source_path",
                "source_sha256",
                "source_function",
                "compiler_function_name",
                "anchor_id",
                "minimum_count",
            }
            if (
                not isinstance(raw_requirement, dict)
                or set(raw_requirement) != requirement_keys
            ):
                fail("coverage manifest requirement schema is not exact")
            if not re.fullmatch(r"[0-9a-f]{64}", raw_requirement["source_sha256"]):
                fail("coverage manifest source SHA-256 is malformed")
            requirements.append(
                CoverageRequirement(
                    requirement_id=raw_requirement["requirement_id"],
                    requirement_kind=raw_requirement["requirement_kind"],
                    source_path=Path(raw_requirement["source_path"]),
                    source_sha256=raw_requirement["source_sha256"],
                    source_function=raw_requirement["source_function"],
                    compiler_function_name=raw_requirement["compiler_function_name"],
                    anchor_id=raw_requirement["anchor_id"],
                    minimum_count=raw_requirement["minimum_count"],
                )
            )
        family_specs.append(
            CoverageFamilySpec(
                family=raw_family["family"],
                test_target=raw_family["test_target"],
                test_id=raw_family["test_id"],
                profile=raw_family["profile"],
                trace_version=raw_family["trace_version"],
                trace_evidence_id=raw_family["trace_evidence_id"],
                coverage_version=raw_family["coverage_version"],
                requirements=tuple(requirements),
            )
        )
    result = tuple(family_specs)
    validate_coverage_manifest(root, result)
    canonical = json.dumps(document, ensure_ascii=False, indent=2) + "\n"
    if raw != canonical.encode("utf-8"):
        fail("coverage manifest JSON bytes are not canonical")
    return result


def normalize_compiler_path(
    filename: object,
    known_paths: tuple[Path, ...],
    *,
    require_match: bool = False,
) -> Path | None:
    """Normalize only compiler-added path prefixes using exact path components."""

    if not isinstance(filename, str) or not filename:
        fail("compiler coverage filename is missing")
    components = tuple(part for part in filename.replace("\\", "/").split("/") if part)
    matches = [
        path
        for path in known_paths
        if len(components) >= len(path.parts)
        and components[-len(path.parts) :] == tuple(path.parts)
    ]
    if not matches and not require_match:
        return None
    if len(matches) != 1:
        fail(
            "compiler coverage source path does not exactly match one manifest path: "
            f"{filename!r}: {matches}"
        )
    return matches[0]


def coverage_requirement_count(
    root: Path,
    requirement: CoverageRequirement,
    functions: list[object],
    known_paths: tuple[Path, ...],
) -> int:
    """Return one exact compiler function count without any region/function fallback."""

    source = (root / requirement.source_path).read_text(encoding="utf-8")
    source_ranges = function_line_ranges(source, requirement.source_function)
    if len(source_ranges) != 1:
        fail(
            "coverage source function is missing or ambiguous: "
            f"{requirement.source_path}:{requirement.source_function}:{source_ranges}"
        )
    function_start, function_end = source_ranges[0]
    exact_matches: list[tuple[dict[str, object], tuple[Path | None, ...]]] = []
    for function in functions:
        if not isinstance(function, dict) or set(function) != {
            "name",
            "count",
            "filenames",
            "regions",
            "branches",
            "mcdc_records",
        }:
            fail("compiler coverage function schema is not exact")
        filenames = function["filenames"]
        regions = function["regions"]
        if (
            not isinstance(function["name"], str)
            or isinstance(function["count"], bool)
            or not isinstance(function["count"], int)
            or not isinstance(filenames, list)
            or not isinstance(regions, list)
        ):
            fail("compiler coverage function types are invalid")
        normalized = tuple(
            normalize_compiler_path(filename, known_paths) for filename in filenames
        )
        if function["name"] == requirement.compiler_function_name and normalized == (
            requirement.source_path,
        ):
            exact_matches.append((function, normalized))
        for region in regions:
            if not isinstance(region, list):
                fail("compiler coverage region is not an array")
            if len(region) != 8:
                fail("compiler coverage region width is not exact")
            file_id = region[5]
            if isinstance(file_id, bool) or not isinstance(file_id, int):
                fail("compiler coverage region file id is invalid")
            if file_id < 0 or file_id >= len(normalized):
                fail("compiler coverage region file id exceeds filenames")
    if not exact_matches:
        return 0
    if len(exact_matches) != 1:
        fail(
            "compiler coverage function identity is duplicate or ambiguous: "
            f"{requirement.anchor_id}:{requirement.compiler_function_name}:"
            f"matches={len(exact_matches)}"
        )
    function, normalized = exact_matches[0]
    source_regions: list[list[object]] = []
    for region in function["regions"]:
        file_id = region[5]
        if (
            normalized[file_id] == requirement.source_path
            and function_start <= region[0] <= region[2] <= function_end
        ):
            source_regions.append(region)
    if not source_regions:
        fail(
            "compiler function identity does not map to its exact source function: "
            f"{requirement.anchor_id}:{requirement.compiler_function_name}:"
            f"{requirement.source_path}:{requirement.source_function}"
        )
    function_count = function["count"]
    if function_count <= 0 or not any(region[4] > 0 for region in source_regions):
        return 0
    return function_count


def validate_compiler_coverage(
    root: Path,
    expected_spec: EvidenceSpec,
    family_specs: tuple[CoverageFamilySpec, ...],
    report: dict[str, object],
) -> None:
    """Require isolated compiler execution counts for one exact family evidence test."""

    if expected_spec.category != RUNTIME_TRACE_CATEGORY:
        fail("compiler coverage is only valid for Task 4 runtime family evidence")
    matching = [spec for spec in family_specs if spec.family == expected_spec.family]
    if len(matching) != 1:
        fail(f"compiler coverage family binding is not unique: {expected_spec.family}")
    family_spec = matching[0]
    expected_binding = (
        expected_spec.test_target,
        expected_spec.test_id,
        expected_spec.profile,
        expected_spec.version,
        expected_spec.evidence_id,
        expected_spec.coverage_version,
    )
    actual_binding = (
        family_spec.test_target,
        family_spec.test_id,
        family_spec.profile,
        family_spec.trace_version,
        family_spec.trace_evidence_id,
        family_spec.coverage_version,
    )
    if actual_binding != expected_binding:
        fail(
            "compiler coverage test/profile/evidence binding does not match the manifest: "
            f"{actual_binding!r} != {expected_binding!r}"
        )
    if not isinstance(report, dict) or set(report) != {
        "cargo_llvm_cov",
        "data",
        "type",
        "version",
    }:
        fail("compiler coverage report root schema is not exact")
    if (
        report["type"] != LLVM_COVERAGE_JSON_TYPE
        or report["version"] != LLVM_COVERAGE_JSON_VERSION
    ):
        fail("compiler coverage JSON type/version is unsupported")
    tool = report["cargo_llvm_cov"]
    if not isinstance(tool, dict) or set(tool) != {"version", "manifest_path"}:
        fail("cargo-llvm-cov report metadata schema is not exact")
    if not isinstance(tool["version"], str) or not re.fullmatch(
        r"\d+\.\d+\.\d+", tool["version"]
    ):
        fail("cargo-llvm-cov report version is invalid")
    normalize_compiler_path(
        tool["manifest_path"], (Path("Cargo.toml"),), require_match=True
    )
    data = report["data"]
    if not isinstance(data, list) or len(data) != 1 or not isinstance(data[0], dict):
        fail("compiler coverage report must contain one data object")
    if set(data[0]) != {"files", "functions", "totals"}:
        fail("compiler coverage data schema is not exact")
    functions = data[0]["functions"]
    if not isinstance(functions, list):
        fail("compiler coverage functions must be an array")
    known_paths = tuple(
        dict.fromkeys(
            requirement.source_path
            for spec in family_specs
            for requirement in spec.requirements
        )
    )
    positive_identities: list[str] = []
    for candidate in family_specs:
        identities = [
            requirement
            for requirement in candidate.requirements
            if requirement.requirement_id == "EXACT_TEST_IDENTITY"
        ]
        if len(identities) != 1:
            fail(f"coverage exact-test identity is not unique: {candidate.family}")
        count = coverage_requirement_count(root, identities[0], functions, known_paths)
        if count > 0:
            positive_identities.append(candidate.family)
    if positive_identities != [expected_spec.family]:
        fail(
            "compiler coverage report is stale, combined, or from another exact test/family: "
            f"positive identities={positive_identities}, expected={[expected_spec.family]}"
        )
    for requirement in family_spec.requirements:
        count = coverage_requirement_count(root, requirement, functions, known_paths)
        if count < requirement.minimum_count:
            fail(
                "compiler coverage requirement has zero or missing execution count: "
                f"{expected_spec.family}:{requirement.requirement_id}:"
                f"{requirement.requirement_kind}:{requirement.anchor_id}:"
                f"{requirement.source_path}:{requirement.source_function}:"
                f"{requirement.compiler_function_name}"
            )


def load_fresh_coverage_report(
    report_path: Path,
    *,
    run_directory: Path,
    started_at_ns: int,
) -> dict[str, object]:
    """Read one bounded report created after this unique exact-test run started."""

    expected_path = run_directory / "coverage.json"
    if report_path.resolve() != expected_path.resolve():
        fail("compiler coverage report path is not the unique run-owned location")
    if not report_path.is_file():
        fail("compiler coverage report is missing (NOT_RUN/ERROR)")
    stat = report_path.stat()
    if stat.st_mtime_ns < started_at_ns:
        fail("compiler coverage report is stale for this exact-test run")
    if stat.st_size <= 0 or stat.st_size > DEFAULT_COVERAGE_REPORT_LIMIT:
        fail(
            "compiler coverage report size is empty or exceeds the bounded limit: "
            f"{stat.st_size}"
        )
    try:
        report = json.loads(report_path.read_bytes())
    except json.JSONDecodeError as error:
        fail(f"compiler coverage report JSON is malformed: {error}")
    if not isinstance(report, dict):
        fail("compiler coverage report root is not an object")
    return report


def verify_coverage_profile_artifacts(run_directory: Path, started_at_ns: int) -> None:
    """Require fresh profraw/profdata artifacts beneath the unique run target only."""

    target = run_directory / "target" / "llvm-cov-target"
    profiles = tuple(sorted(target.glob("*.profraw")))
    merged = tuple(sorted(target.glob("*.profdata")))
    if not profiles or len(merged) != 1:
        fail(
            "compiler coverage profile data is absent or combined outside one run: "
            f"profraw={len(profiles)}, profdata={len(merged)}"
        )
    for artifact in (*profiles, *merged):
        if artifact.stat().st_mtime_ns < started_at_ns or not artifact.is_file():
            fail(f"compiler coverage profile artifact is stale: {artifact}")
        try:
            artifact.resolve().relative_to(run_directory.resolve())
        except ValueError:
            fail(f"compiler coverage profile escaped its unique run: {artifact}")


def validate_runtime_trace_output(
    output: str,
    expected_evidence_id: str,
    expected_family: str,
    expected_version: str,
    expected_challenge: str,
    root: Path | None = None,
) -> None:
    """Require exactly one complete, same-family, same-instance typed trace record."""

    prefix = f"{TRACE_MARKER}\t"
    marker_lines = [line for line in output.splitlines() if line.startswith(prefix)]
    if output.count(prefix) != 1 or len(marker_lines) != 1:
        fail(
            "runtime evidence must contain exactly one library-owned trace record: "
            f"found {len(marker_lines)} exact lines"
        )
    try:
        payload = json.loads(marker_lines[0][len(prefix) :])
    except json.JSONDecodeError as error:
        fail(f"runtime trace JSON is malformed: {error}")
    required_keys = {
        "evidence_id",
        "version",
        "challenge",
        "family",
        "instance_id",
        "events",
    }
    if not isinstance(payload, dict) or set(payload) != required_keys:
        fail("runtime trace payload schema is not exact")
    if payload["evidence_id"] != expected_evidence_id:
        fail("runtime trace evidence id does not match the TSV binding")
    if payload["version"] != expected_version or expected_version != TRACE_VERSION:
        fail("runtime trace protocol version is not the unique Task 4 version")
    if payload["challenge"] != expected_challenge:
        fail("runtime trace challenge does not match this verifier run")
    if payload["family"] != expected_family or expected_family not in TRACE_FAMILIES:
        fail("runtime trace family does not match the TSV binding")
    instance_id = payload["instance_id"]
    if (
        isinstance(instance_id, bool)
        or not isinstance(instance_id, int)
        or instance_id <= 0
    ):
        fail("runtime trace instance id must be a positive integer")
    events = payload["events"]
    if not isinstance(events, list) or len(events) != len(TRACE_STAGES):
        fail("runtime trace event denominator is incomplete or duplicated")
    observed_stages: list[str] = []
    for event in events:
        if not isinstance(event, dict) or set(event) != {
            "family",
            "instance_id",
            "stage",
            "caller_file",
            "caller_line",
        }:
            fail("runtime trace event schema is not exact")
        if event["family"] != expected_family or event["instance_id"] != instance_id:
            fail("runtime trace splices different family or connection instances")
        if not isinstance(event["stage"], str):
            fail("runtime trace stage is not typed as a string label")
        caller_file = event["caller_file"]
        caller_line = event["caller_line"]
        if not isinstance(caller_file, str) or not caller_file:
            fail("runtime trace caller file is missing")
        if (
            isinstance(caller_line, bool)
            or not isinstance(caller_line, int)
            or caller_line <= 0
        ):
            fail("runtime trace caller line is invalid")
        expected_file = TRACE_CALLSITE_RULES[expected_family].get(event["stage"])
        normalized_file = caller_file.replace("\\", "/")
        marker = "crates/druid-wrapper/"
        marker_index = normalized_file.find(marker)
        if marker_index >= 0:
            normalized_file = normalized_file[marker_index:]
        if normalized_file != expected_file:
            fail(
                "runtime trace caller file does not match its production category: "
                f"{event['stage']}: {normalized_file} != {expected_file}"
            )
        if root is not None:
            source_path = root / expected_file
            if not source_path.is_file():
                fail(f"runtime trace caller source is missing: {expected_file}")
            source_lines = source_path.read_text(encoding="utf-8").splitlines()
            if caller_line > len(source_lines):
                fail(
                    f"runtime trace caller line exceeds source: {expected_file}:{caller_line}"
                )
            method = TRACE_STAGE_METHODS[event["stage"]]
            if method not in source_lines[caller_line - 1]:
                fail(
                    "runtime trace caller line is outside its exact production callsite: "
                    f"{expected_file}:{caller_line}:{method}"
                )
            function_name = TRACE_CALLSITE_FUNCTIONS[expected_family][event["stage"]]
            ranges = function_line_ranges(
                source_path.read_text(encoding="utf-8"), function_name
            )
            if not any(start <= caller_line <= end for start, end in ranges):
                fail(
                    "runtime trace caller line is outside its production function region: "
                    f"{expected_file}:{caller_line}:{function_name}:{ranges}"
                )
        observed_stages.append(event["stage"])
    if tuple(observed_stages) != TRACE_STAGES:
        fail("runtime trace stages are missing, duplicated, or out of order")


def verify_runtime_trace_source_hygiene(path: Path, source: str) -> None:
    """Apply the structural untrusted-source policy to tests and I/O doubles."""

    verify_untrusted_evidence_source(path, source)


def verify_trace_record_callsites(root: Path) -> None:
    """Require the exact production callsite multiset, including method and UFCS syntax."""

    source_root = root / "crates/druid-wrapper/src"
    observed: dict[tuple[str, str], int] = {}
    for path in source_root.rglob("*.rs"):
        relative = path.relative_to(root).as_posix()
        source = path.read_text(encoding="utf-8")
        for method in TRACE_RECORD_METHODS:
            calls = len(
                re.findall(
                    rf"(?:\.|::)\s*{re.escape(method)}\s*\(",
                    source,
                )
            )
            if calls:
                observed[(relative, method)] = calls
    if observed != TRACE_RECORD_CALLSITE_COUNTS:
        missing = {
            key: count - observed.get(key, 0)
            for key, count in TRACE_RECORD_CALLSITE_COUNTS.items()
            if observed.get(key, 0) != count
        }
        extra = {
            key: count - TRACE_RECORD_CALLSITE_COUNTS.get(key, 0)
            for key, count in observed.items()
            if count != TRACE_RECORD_CALLSITE_COUNTS.get(key, 0)
        }
        fail(
            f"runtime trace record callsite multiset drift: missing={missing}, extra={extra}"
        )
    verify_trace_branch_callsites(root)

    track_caller_sources = (
        source_root / "task4_runtime_trace.rs",
        source_root / "standard_adapter_executor.rs",
        source_root / "sqlx/sqlx_connection_actor.rs",
    )
    combined = "\n".join(
        path.read_text(encoding="utf-8") for path in track_caller_sources
    )
    required_track_caller = (
        "record",
        *sorted(TRACE_RECORD_METHODS),
        *sorted(TRACE_COVERAGE_ANCHOR_FUNCTIONS.values()),
    )
    for method in required_track_caller:
        definitions = len(re.findall(rf"\bfn\s+{re.escape(method)}\s*\(", combined))
        tracked = len(
            re.findall(
                rf"#\s*\[\s*track_caller\s*\]\s*"
                rf"(?:pub(?:\([^)]*\))?\s+)?fn\s+{re.escape(method)}\s*\(",
                combined,
            )
        )
        if not definitions or tracked != definitions:
            fail(
                "runtime trace record chain lacks #[track_caller] on every hop: "
                f"{method}: tracked={tracked}, definitions={definitions}"
            )
    trace_source = (source_root / "task4_runtime_trace.rs").read_text(encoding="utf-8")
    for stage, anchor in TRACE_COVERAGE_ANCHOR_FUNCTIONS.items():
        inline_definitions = len(
            re.findall(
                rf"#\s*\[\s*inline\s*\(\s*never\s*\)\s*\]\s*"
                rf"#\s*\[\s*track_caller\s*\]\s*fn\s+{re.escape(anchor)}\s*\(",
                trace_source,
            )
        )
        calls = len(re.findall(rf"\.\s*{re.escape(anchor)}\s*\(", trace_source))
        if inline_definitions != 1 or calls != 1:
            fail(
                "typed stage coverage anchor is missing, duplicated, or bypassable: "
                f"{stage}:{anchor}:definitions={inline_definitions}:calls={calls}"
            )
    success_definitions = len(
        re.findall(
            rf"#\s*\[\s*inline\s*\(\s*never\s*\)\s*\]\s*"
            rf"fn\s+{re.escape(TRACE_VALIDATION_SUCCESS_ANCHOR)}\s*\(",
            trace_source,
        )
    )
    success_occurrences = len(
        re.findall(
            rf"\b{re.escape(TRACE_VALIDATION_SUCCESS_ANCHOR)}\s*\(",
            trace_source,
        )
    )
    if success_definitions != 1 or success_occurrences != 2:
        fail(
            "typed validation-success coverage anchor is missing, duplicated, or bypassable: "
            f"definitions={success_definitions}:occurrences={success_occurrences}"
        )


def verify_trace_branch_callsites(root: Path) -> None:
    """Pin duplicate trace calls to exact mutually exclusive production branches."""

    for (relative, function, method), expected in TRACE_BRANCH_CALLSITE_COUNTS.items():
        path = root / relative
        if not path.is_file():
            fail(f"runtime trace branch source is missing: {relative}")
        source = path.read_text(encoding="utf-8")
        ranges = function_line_ranges(source, function)
        if len(ranges) != 1:
            fail(
                "runtime trace branch function is missing/ambiguous: "
                f"{relative}:{function}:{ranges}"
            )
        start, end = ranges[0]
        body = "\n".join(source.splitlines()[start - 1 : end])
        observed = len(re.findall(rf"(?:\.|::)\s*{re.escape(method)}\s*\(", body))
        if observed != expected:
            fail(
                "runtime trace branch callsite multiplicity drift: "
                f"{relative}:{function}:{method}:{observed} != {expected}"
            )
    toasty_path = (
        root
        / "crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs"
    )
    toasty_source = toasty_path.read_text(encoding="utf-8")
    start, end = function_line_ranges(toasty_source, "database_meta_data")[0]
    toasty_body = code_tokens("\n".join(toasty_source.splitlines()[start - 1 : end]))
    mutually_exclusive = (
        "if let Some ( executor ) = & self . standard_executor",
        "else if let Some ( trace ) = & self . runtime_trace",
        "executor . task4_trace_database_metadata ( )",
        "trace . task4_trace_database_metadata ( )",
    )
    missing = [marker for marker in mutually_exclusive if marker not in toasty_body]
    if missing:
        fail(f"Toasty metadata trace branches are not mutually exclusive: {missing}")


def evidence_source(root: Path, spec: EvidenceSpec) -> tuple[Path, str]:
    """Resolve one exact Cargo target/test id to source used for anti-forgery scanning."""

    function_name = spec.test_id.rsplit("::", 1)[-1]
    if spec.test_target == "lib":
        candidates = [
            path
            for path in (root / "crates/druid-wrapper/src").rglob("*.rs")
            if re.search(
                rf"\bfn\s+{re.escape(function_name)}\s*\(",
                path.read_text(encoding="utf-8"),
            )
        ]
        if len(candidates) != 1:
            fail(
                f"runtime evidence lib test source is not exact: {spec.test_id}: {candidates}"
            )
        source = candidates[0].read_text(encoding="utf-8")
        function_offset = source.find(f"fn {function_name}")
        test_module_offset = source.rfind("#[cfg(test)]", 0, function_offset)
        if test_module_offset < 0:
            fail(f"runtime evidence lib test is not inside cfg(test): {spec.test_id}")
        return candidates[0], source[test_module_offset:]
    if not spec.test_target.startswith("test:"):
        fail(f"runtime evidence has unknown Cargo target: {spec.test_target}")
    target = spec.test_target.removeprefix("test:")
    path = root / "crates/druid-wrapper/tests" / f"{target}.rs"
    if not path.is_file():
        fail(f"runtime evidence integration target is missing: {target}")
    source = path.read_text(encoding="utf-8")
    if len(re.findall(rf"\bfn\s+{re.escape(function_name)}\s*\(", source)) != 1:
        fail(f"runtime evidence integration test id is not exact: {spec.test_id}")
    return path, source


def source_module_dependencies(path: Path, source: str) -> tuple[Path, ...]:
    """Resolve literal mod/#[path]/include! edges from one Rust source file."""

    if re.search(r"cfg_attr\s*\([^)]*\bpath\s*=", source, re.DOTALL):
        fail(f"evidence source contains conditional #[path] routing: {path}")
    dependencies: set[Path] = set()
    explicit_modules: set[str] = set()
    path_pattern = re.compile(
        r'#\s*\[\s*path\s*=\s*"([^"\r\n]+)"\s*\]\s*'
        r"(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;"
    )
    for relative, module_name in path_pattern.findall(source):
        explicit_modules.add(module_name)
        dependency = (path.parent / relative).resolve()
        if not dependency.is_file():
            fail(f"evidence #[path] module is missing: {path}: {relative}")
        dependencies.add(dependency)
    module_pattern = re.compile(
        r"(?m)^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+" r"([A-Za-z_][A-Za-z0-9_]*)\s*;"
    )
    for module_name in module_pattern.findall(source):
        if module_name in explicit_modules:
            continue
        if path.name in {"lib.rs", "main.rs", "mod.rs"} or "tests" in path.parts:
            bases = (path.parent, path.parent / path.stem)
        else:
            bases = (path.parent / path.stem, path.parent)
        candidates: list[Path] = []
        for base in bases:
            candidates.extend(
                (base / f"{module_name}.rs", base / module_name / "mod.rs")
            )
        existing = tuple(
            dict.fromkeys(
                candidate.resolve() for candidate in candidates if candidate.is_file()
            )
        )
        if len(existing) != 1:
            fail(
                f"evidence module edge is missing or ambiguous: {path}: mod {module_name}: {existing}"
            )
        dependencies.add(existing[0])
    literal_includes = re.findall(r'include\s*!\s*\(\s*"([^"\r\n]+)"\s*\)', source)
    include_invocations = len(re.findall(r"\binclude\s*!\s*\(", source))
    if include_invocations != len(literal_includes):
        fail(f"evidence source contains a non-literal include!: {path}")
    for relative in literal_includes:
        dependency = (path.parent / relative).resolve()
        if not dependency.is_file():
            fail(f"evidence include! source is missing: {path}: {relative}")
        dependencies.add(dependency)
    return tuple(sorted(dependencies))


def recursive_source_closure(seed: Path) -> tuple[Path, ...]:
    """Return the complete literal Rust module/include closure rooted at one source file."""

    pending = [seed.resolve()]
    visited: set[Path] = set()
    while pending:
        path = pending.pop()
        if path in visited:
            continue
        if not path.is_file():
            fail(f"evidence source closure file is missing: {path}")
        visited.add(path)
        source = path.read_text(encoding="utf-8")
        pending.extend(
            dependency
            for dependency in source_module_dependencies(path, source)
            if dependency not in visited
        )
    return tuple(sorted(visited))


def verify_untrusted_evidence_source(
    path: Path,
    source: str,
    *,
    allow_trace_api: bool = False,
) -> None:
    """Reject output, completion, trace internals, and record access in untrusted evidence code."""

    tokens = tuple(token.text.removeprefix("r#") for token in tokenize(source))
    violations: set[str] = set()
    for index, token in enumerate(tokens):
        if token in OUTPUT_MACROS:
            violations.add(f"output macro {token}!")
        if token in DIRECT_OUTPUT_IDENTIFIERS:
            violations.add(f"direct output API {token}")
        if token in TRACE_RECORD_METHODS:
            violations.add(f"trace record API {token}")
        if (
            token in {"verify_and_emit", "enable_task4_runtime_trace"}
            and not allow_trace_api
        ):
            violations.add(f"trace verification API {token}")
        if token in {"RuntimeTraceEmitter", "TraceStage", "TraceEvent"}:
            violations.add(f"trace internal {token}")
        if (
            token == "process"
            and index + 2 < len(tokens)
            and tokens[index + 1 : index + 3] == ("::", "exit")
        ):
            violations.add("process::exit")
        if token == "exit" and index > 0 and tokens[index - 1] == ".":
            violations.add("process exit method")
    allowed_trace_names = {"task4_runtime_trace", "verify_and_emit"}
    trace_names = {
        token
        for token in tokens
        if token.startswith("Trace")
        or token.startswith("RuntimeTrace")
        or token == "task4_runtime_trace"
    }
    unexpected_trace_names = trace_names.difference(allowed_trace_names)
    if unexpected_trace_names:
        violations.update(f"trace access {name}" for name in unexpected_trace_names)
    if TRACE_CHALLENGE_ENV in source:
        violations.add("trace challenge access")
    if violations:
        fail(
            f"untrusted evidence source can forge runtime completion: {path}: {sorted(violations)}"
        )


def verify_root_trace_api_regions(path: Path, source: str, function_name: str) -> None:
    """Keep trace setup/emission API calls inside the one exact manifest-bound test function."""

    ranges = function_line_ranges(source, function_name)
    if len(ranges) != 1:
        fail(
            f"evidence root does not contain one exact test function region: {path}:{function_name}"
        )
    for line_number, line in enumerate(source.splitlines(), start=1):
        if "verify_and_emit" not in line and "enable_task4_runtime_trace" not in line:
            continue
        if not any(start <= line_number <= end for start, end in ranges):
            fail(
                f"trace verification API escaped exact evidence function: {path}:{line_number}"
            )


def verify_production_trace_output_ownership(root: Path) -> None:
    """Keep marker/output ownership in the typed trace module across all production sources."""

    source_root = root / "crates/druid-wrapper/src"
    trace_owner = source_root / "task4_runtime_trace.rs"
    for path in recursive_source_closure(source_root / "lib.rs"):
        source = path.read_text(encoding="utf-8")
        tokens = tuple(token.text.removeprefix("r#") for token in tokenize(source))
        if path == trace_owner:
            output_calls = [
                token
                for index, token in enumerate(tokens[:-1])
                if token in OUTPUT_MACROS and tokens[index + 1] == "!"
            ]
            if output_calls != ["eprintln"]:
                fail(
                    f"typed trace owner must have one eprintln marker sink: {output_calls}"
                )
            continue
        if TRACE_MARKER in source:
            fail(
                f"runtime trace marker escaped its typed owner: {path.relative_to(root)}"
            )
        for index, token in enumerate(tokens[:-1]):
            if token in OUTPUT_MACROS and tokens[index + 1] == "!":
                fail(
                    f"production source may forge evidence output: {path.relative_to(root)}:{token}!"
                )
        if re.search(
            r"(?:std\s*::\s*io\s*::|\bio\s*::\s*)(?:stdout|stderr)\s*\(", source
        ):
            fail(
                f"production source may access evidence output streams: {path.relative_to(root)}"
            )
        if any(token in {"from_raw_fd", "as_raw_fd"} for token in tokens):
            fail(
                f"production source may access raw evidence descriptors: {path.relative_to(root)}"
            )
        if any(
            tokens[index : index + 3] == ("process", "::", "exit")
            for index in range(len(tokens) - 2)
        ):
            fail(
                f"production source may terminate evidence process: {path.relative_to(root)}"
            )


def verify_evidence_source_closure(root: Path, spec: EvidenceSpec) -> None:
    """Verify the exact test plus every reachable helper and the crate production closure."""

    verify_production_trace_output_ownership(root)
    path, root_source = evidence_source(root, spec)
    function_name = spec.test_id.rsplit("::", 1)[-1]
    verify_untrusted_evidence_source(
        path.relative_to(root), root_source, allow_trace_api=True
    )
    verify_root_trace_api_regions(path.relative_to(root), root_source, function_name)
    closure_seed = path
    for dependency in recursive_source_closure(closure_seed):
        if dependency == path.resolve():
            continue
        try:
            relative = dependency.relative_to(root)
        except ValueError:
            fail(f"evidence source closure escapes repository root: {dependency}")
        verify_untrusted_evidence_source(
            relative, dependency.read_text(encoding="utf-8"), allow_trace_api=False
        )


def cargo_evidence_command(
    spec: EvidenceSpec, coverage_report_path: Path | None = None
) -> list[str]:
    """Build the one exact Cargo command declared by a TSV evidence row."""

    if spec.category == RUNTIME_TRACE_CATEGORY:
        if coverage_report_path is None:
            fail(
                "runtime compiler coverage command requires one unique JSON report path"
            )
        command = [
            "cargo",
            "llvm-cov",
            "test",
            "--json",
            "--output-path",
            str(coverage_report_path),
            "--offline",
            "--locked",
            "--color",
            "never",
            "-p",
            "druid-wrapper",
        ]
    elif spec.category == COMPILE_IDENTITY_CATEGORY:
        if coverage_report_path is not None:
            fail("compile identity evidence must not claim a runtime coverage report")
        command = [
            "cargo",
            "test",
            "--offline",
            "--locked",
            "--color",
            "never",
            "-p",
            "druid-wrapper",
        ]
    else:
        fail(f"Task 4 evidence category is unknown: {spec.category}")
    if spec.profile == "all-features":
        command.append("--all-features")
    elif spec.profile != "default":
        fail(f"runtime evidence profile is unknown: {spec.profile}")
    if spec.test_target == "lib":
        command.append("--lib")
    elif spec.test_target.startswith("test:"):
        command.extend(["--test", spec.test_target.removeprefix("test:")])
    else:
        fail(f"runtime evidence Cargo target is unknown: {spec.test_target}")
    if spec.category == COMPILE_IDENTITY_CATEGORY:
        command.append("--no-run")
    else:
        command.extend([spec.test_id, "--", "--exact", "--nocapture"])
    return command


def kill_process_group(
    process: subprocess.Popen[bytes], process_group_id: int | None
) -> None:
    """Kill one evidence process and all children in its isolated process group."""

    if os.name == "posix" and process_group_id is not None:
        try:
            os.killpg(process_group_id, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except PermissionError:
            # macOS can retain a just-killed process group containing only a reaped
            # parent/zombie briefly and report EPERM on the second deterministic cleanup.
            # A live direct parent must never hide an authorization failure.
            if process.poll() is None:
                raise
    elif process.poll() is None:
        process.kill()


def run_bounded_process(
    command: list[str],
    *,
    cwd: Path,
    timeout_seconds: float,
    output_limit: int,
    env: dict[str, str] | None = None,
) -> BoundedProcessResult:
    """Run one isolated process with a deadline, process-group kill, and output cap."""

    if timeout_seconds <= 0 or output_limit <= 0:
        fail("bounded process requires positive timeout and output limit")
    if os.name == "nt":
        from windows_bounded_process import WindowsBoundedProcessError, run_windows_bounded_process
        try:
            return BoundedProcessResult(*run_windows_bounded_process(
                command, cwd=cwd, timeout_seconds=timeout_seconds,
                output_limit=output_limit, env=env,
            ))
        except WindowsBoundedProcessError as error:
            failure = VerificationError(str(error))
            failure.owned_descendants = error.owned_descendants
            failure.ownership_capture_error = error.ownership_capture_error
            raise failure from error
        except (OSError, RuntimeError, ValueError) as error:
            fail(str(error))
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        start_new_session=os.name == "posix",
    )
    process_group_id = process.pid if os.name == "posix" else None
    if process.stdout is None or process.stderr is None:
        kill_process_group(process, process_group_id)
        fail("bounded process did not expose stdout/stderr pipes")
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    deadline = time.monotonic() + timeout_seconds
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                kill_process_group(process, process_group_id)
                process.wait()
                fail(f"evidence process timed out after {timeout_seconds:.3f}s")
            events = selector.select(min(remaining, 0.05))
            for key, _ in events:
                chunk = os.read(key.fileobj.fileno(), 65_536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                buffers[key.data].extend(chunk)
                if sum(len(buffer) for buffer in buffers.values()) > output_limit:
                    kill_process_group(process, process_group_id)
                    process.wait()
                    fail(f"evidence process exceeded output limit {output_limit}")
            if process.poll() is not None and not events:
                # Pipes may still contain a final chunk; keep draining until EOF.
                continue
        returncode = process.wait()
    finally:
        selector.close()
        # The direct cargo parent may already be reaped while a descendant has closed
        # inherited pipes and continues running. Always target the start-time PGID;
        # `killpg` safely returns ESRCH when the group is already empty.
        kill_process_group(process, process_group_id)
        if process.poll() is None:
            process.wait()
        for stream in (process.stdout, process.stderr):
            if stream is not None and not stream.closed:
                stream.close()
    return BoundedProcessResult(
        returncode,
        bytes(buffers["stdout"]).decode("utf-8", errors="replace"),
        bytes(buffers["stderr"]).decode("utf-8", errors="replace"),
    )


def execute_evidence(
    root: Path,
    spec: EvidenceSpec,
    coverage_family_specs: tuple[CoverageFamilySpec, ...] = (),
) -> None:
    """Run one exact test; runtime completion is anchored only in fresh compiler coverage."""

    verify_evidence_source_closure(root, spec)
    environment = os.environ.copy()
    environment.pop(TRACE_CHALLENGE_ENV, None)
    if spec.category == COMPILE_IDENTITY_CATEGORY:
        try:
            result = run_bounded_process(
                cargo_evidence_command(spec),
                cwd=root,
                timeout_seconds=DEFAULT_EVIDENCE_TIMEOUT_SECONDS,
                output_limit=DEFAULT_EVIDENCE_OUTPUT_LIMIT,
                env=environment,
            )
        except (OSError, VerificationError) as error:
            fail(f"compile identity evidence NOT_RUN/ERROR: {spec.test_id}: {error}")
        output = f"{result.stdout}\n{result.stderr}"
        if result.returncode != 0:
            fail(f"compile identity evidence ERROR: {spec.test_id}: {output.strip()}")
        return
    if spec.category != RUNTIME_TRACE_CATEGORY:
        fail(f"Task 4 evidence category is unknown: {spec.category}")
    if not coverage_family_specs:
        fail("compiler coverage manifest is missing (NOT_RUN/ERROR)")
    with tempfile.TemporaryDirectory(
        prefix=f"druid-task4-{spec.family.lower()}-coverage-"
    ) as temporary:
        run_directory = Path(temporary)
        report_path = run_directory / "coverage.json"
        environment["CARGO_TARGET_DIR"] = str(run_directory / "target")
        started_at_ns = time.time_ns()
        try:
            result = run_bounded_process(
                cargo_evidence_command(spec, report_path),
                cwd=root,
                timeout_seconds=DEFAULT_EVIDENCE_TIMEOUT_SECONDS,
                output_limit=DEFAULT_EVIDENCE_OUTPUT_LIMIT,
                env=environment,
            )
        except (OSError, VerificationError) as error:
            fail(
                "compiler coverage exact run NOT_RUN/ERROR: " f"{spec.test_id}: {error}"
            )
        output = f"{result.stdout}\n{result.stderr}"
        if result.returncode != 0:
            fail(
                "compiler coverage exact run ERROR: "
                f"{spec.test_id}: {output.strip()}"
            )
        report = load_fresh_coverage_report(
            report_path,
            run_directory=run_directory,
            started_at_ns=started_at_ns,
        )
        verify_coverage_profile_artifacts(run_directory, started_at_ns)
        validate_compiler_coverage(root, spec, coverage_family_specs, report)
        # The marker remains useful diagnostic output, but neither its presence nor its
        # challenge text participates in acceptance. The covered final success region in
        # `verify_and_emit` is the trust anchor for typed sequence validation.


def adapter_row_family(row: AdapterRow) -> str:
    """Derive one immutable family identity from the adapter source owner."""

    source = row.source_path.as_posix()
    families = {
        "/sqlx/": "SQLX",
        "/toasty/": "TOASTY",
        "/rbdc/": "RBDC",
        "/duckdb/": "DUCKDB",
        "/http_sql/": "HTTP_SQL",
        "/libsql/": "LIBSQL",
        "/jdbc_agent/": "JDBC_AGENT",
        "/rdbc/": "IDENTITY",
    }
    matches = [family for fragment, family in families.items() if fragment in source]
    if len(matches) != 1:
        fail(f"Task 4 adapter row has no unique source family: {row.source_path}")
    return matches[0]


def verify_row_evidence_bindings(rows: list[AdapterRow]) -> None:
    """Compare all 31 source rows to their canonical family binding before deduplication."""

    for row in rows:
        family = adapter_row_family(row)
        if family == "IDENTITY":
            target, test_id, profile, behavior = IDENTITY_EVIDENCE_BINDING
            expected = (
                target,
                test_id,
                profile,
                COMPILE_IDENTITY_CATEGORY,
                "-",
                "-",
                "-",
                behavior,
                "-",
                "-",
            )
        else:
            target, test_id, profile, behavior, evidence_id = (
                CANONICAL_EVIDENCE_BINDINGS[family]
            )
            expected = (
                target,
                test_id,
                profile,
                RUNTIME_TRACE_CATEGORY,
                family,
                TRACE_VERSION,
                evidence_id,
                behavior,
                COVERAGE_VERSION,
                COVERAGE_REQUIREMENT_SET,
            )
        actual = (
            row.behavior_test_target,
            row.cargo_test_id,
            row.behavior_profile,
            row.evidence_category,
            row.trace_family,
            row.trace_version,
            row.trace_evidence_id,
            row.behavior_evidence,
            row.coverage_version,
            row.coverage_requirement_ids,
        )
        if actual != expected:
            fail(
                "Task 4 runtime evidence row is not bound to its source family: "
                f"{row.source_path}: expected={expected!r}, actual={actual!r}"
            )


def evidence_specs(rows: list[AdapterRow]) -> tuple[EvidenceSpec, ...]:
    """Validate row bindings and return one deterministic set of exact tests to execute."""

    verify_row_evidence_bindings(rows)
    specs = {
        EvidenceSpec(
            row.behavior_test_target,
            row.cargo_test_id,
            row.behavior_profile,
            row.evidence_category,
            row.trace_family,
            row.trace_version,
            row.trace_evidence_id,
            row.coverage_version,
        )
        for row in rows
        if row.state == LIVE_ADAPTER_STATE
    }
    runtime = [spec for spec in specs if spec.category == RUNTIME_TRACE_CATEGORY]
    identity = [spec for spec in specs if spec.category == COMPILE_IDENTITY_CATEGORY]
    if len(runtime) != 7 or {spec.family for spec in runtime} != TRACE_FAMILIES:
        fail(f"Task 4 must bind exactly seven family runtime traces: {runtime}")
    if len(identity) != 1:
        fail(
            f"Task 4 DataSource Wrapper identity must use one independent exact test: {identity}"
        )
    if sum(row.evidence_category == COMPILE_IDENTITY_CATEGORY for row in rows) != 2:
        fail(
            "Task 4 must classify exactly two DataSource rows as nonbehavior identity evidence"
        )
    for spec in runtime:
        if (
            spec.version != TRACE_VERSION
            or spec.family not in TRACE_FAMILIES
            or spec.coverage_version != COVERAGE_VERSION
        ):
            fail(f"runtime trace binding is not canonical: {spec}")
        if spec.evidence_id in {"", "-"}:
            fail(f"runtime trace evidence id is missing: {spec}")
    for spec in identity:
        if (spec.family, spec.version, spec.evidence_id, spec.coverage_version) != (
            "-",
            "-",
            "-",
            "-",
        ):
            fail(f"compile identity row must not claim runtime trace fields: {spec}")
    exact_bindings: dict[tuple[str, str, str], EvidenceSpec] = {}
    evidence_ids: set[str] = set()
    for spec in specs:
        key = (spec.test_target, spec.test_id, spec.profile)
        previous = exact_bindings.setdefault(key, spec)
        if previous != spec:
            fail(
                f"one exact Cargo test/profile is bound to conflicting evidence: {key}"
            )
        if spec.category == RUNTIME_TRACE_CATEGORY and spec.evidence_id in evidence_ids:
            fail(f"runtime trace evidence id is duplicated: {spec.evidence_id}")
        evidence_ids.add(spec.evidence_id)
    return tuple(sorted(specs))


def verify_coverage_manifest_bindings(
    specs: tuple[EvidenceSpec, ...],
    coverage_family_specs: tuple[CoverageFamilySpec, ...],
) -> None:
    """Bind every runtime EvidenceSpec to one field-exact coverage family contract."""

    runtime = tuple(spec for spec in specs if spec.category == RUNTIME_TRACE_CATEGORY)
    if tuple(sorted(spec.family for spec in runtime)) != COVERAGE_FAMILY_ORDER:
        fail("runtime evidence and compiler coverage family denominators differ")
    coverage_by_family = {spec.family: spec for spec in coverage_family_specs}
    if len(coverage_by_family) != len(coverage_family_specs):
        fail("compiler coverage manifest contains duplicate family bindings")
    for spec in runtime:
        coverage = coverage_by_family.get(spec.family)
        if coverage is None:
            fail(f"compiler coverage family mapping is missing: {spec.family}")
        actual = (
            coverage.test_target,
            coverage.test_id,
            coverage.profile,
            coverage.trace_version,
            coverage.trace_evidence_id,
            coverage.coverage_version,
        )
        expected = (
            spec.test_target,
            spec.test_id,
            spec.profile,
            spec.version,
            spec.evidence_id,
            spec.coverage_version,
        )
        if actual != expected:
            fail(
                "runtime row is not field-exact with compiler coverage manifest: "
                f"{spec.family}: {actual!r} != {expected!r}"
            )


def has_wrapper_macro(source: str, type_name: str, target_trait: str) -> bool:
    """Return whether the canonical Wrapper macro covers this implementation type."""

    concrete = concrete_type(type_name)
    leaf = target_trait.rsplit("::", 1)[-1]
    return (
        re.search(
            rf"impl_standard_wrapper!\s*\(\s*{re.escape(concrete)}(?:\s*<[^;)]*>)?\s*=>\s*{re.escape(leaf)}\s*\)",
            source,
        )
        is not None
    )


def verify_adapter_source(root: Path, row: AdapterRow) -> None:
    """Require one canonical implementation and a source-bound execution core."""

    path = root / row.source_path
    if not path.is_file():
        fail(f"adapter source is missing: {row.source_path}")
    source = path.read_text(encoding="utf-8")
    tokens = code_tokens(source)
    if row.state != LIVE_ADAPTER_STATE:
        fail(
            f"adapter inventory row is not canonical live: {row.implementation_type}: {row.state}"
        )
    if row.current_trait != row.target_trait:
        fail(
            f"adapter inventory retains a transitional trait: {row.implementation_type}: "
            f"{row.current_trait} != {row.target_trait}"
        )
    standard_path = root / row.standard_source_path
    if not standard_path.is_file():
        fail(f"canonical standard source is missing: {row.standard_source_path}")
    if row.standard_implementation_type == "StandardAdapterPreparedStatement":
        standard_source = standard_path.read_text(encoding="utf-8")
        required = (
            "impl PreparedStatement for StandardAdapterPreparedStatement",
            "set_parameter",
            "execute_prepared_query",
            "execute_prepared_update",
            "executor_core",
        )
        if any(marker not in standard_source for marker in required):
            fail(
                f"common canonical Prepared behavior is incomplete: {row.implementation_type}"
            )
        owner = row.standard_core_evidence.split("::", 1)[0]
        family_sources = list(
            (root / "crates/druid-wrapper/src").rglob("*connection*.rs")
        )
        owner_source = next(
            (
                candidate.read_text(encoding="utf-8")
                for candidate in family_sources
                if owner in candidate.read_text(encoding="utf-8")
            ),
            None,
        )
        if owner_source is None or any(
            marker not in owner_source
            for marker in (
                f"impl AdapterExecutionCore for {owner}",
                "standard_executor_core",
                "StandardAdapterPreparedStatement::new",
            )
        ):
            fail(
                f"family prepared executor provenance is missing: {row.implementation_type}"
            )
        return
    if row.standard_implementation_type == "StandardAdapterDatabaseMetaData":
        standard_source = standard_path.read_text(encoding="utf-8")
        required = (
            "impl DatabaseMetaData for StandardAdapterDatabaseMetaData",
            "database_product_name",
            "tables",
            "ensure_open_core",
        )
        if any(marker not in standard_source for marker in required):
            fail(
                f"common canonical metadata behavior is incomplete: {row.implementation_type}"
            )
        owner = row.standard_core_evidence.split("::", 1)[0]
        if owner not in source or "standard_executor_core" not in source:
            fail(
                f"family metadata executor provenance is missing: {row.implementation_type}"
            )
        return
    target_leaf = row.target_trait.rsplit("::", 1)[-1]
    standard = extract_impl_bodies(source, row.target_trait, row.implementation_type)
    wrapper_only = target_leaf == "Wrapper" and has_wrapper_macro(
        source, row.implementation_type, row.target_trait
    )
    if not standard and not wrapper_only:
        fail(
            f"canonical adapter implementation is missing: {row.implementation_type} "
            f"for {row.target_trait}"
        )
    canonical_path = row.target_trait.replace("druid_rdbc::", "druid_rdbc :: ").replace(
        "::", " :: "
    )
    canonical_import = (
        row.target_trait.rsplit("::", 1)[0].replace("::", " :: ") + f" :: {target_leaf}"
    )
    macro_provenance = "druid_rdbc :: impl_standard_wrapper" in tokens and (
        f"=> {target_leaf}" in tokens or f"= > {target_leaf}" in tokens
    )
    if (
        canonical_path not in tokens
        and canonical_import not in tokens
        and not macro_provenance
    ):
        fail(
            f"standard impl lacks canonical trait provenance: {row.implementation_type}"
        )
    if re.search(rf"\btrait\s+{re.escape(target_leaf)}\b", source):
        fail(f"local same-leaf trait shadows canonical identity: {target_leaf}")
    if re.search(r"#\s*\[\s*cfg\s*\(\s*any\s*\(\s*\)\s*\)\s*\]\s*impl", source):
        fail(f"canonical impl is production-disabled: {row.implementation_type}")
    for body in standard:
        if not code_tokens(body).strip():
            fail(
                f"canonical impl inherits only empty/default behavior: {row.implementation_type}"
            )
        markers = [marker for marker in SQL_EXECUTION_MARKERS if marker in body]
        delegates = "_core(" in body or "actor" in body.lower()
        if markers and not delegates:
            fail(
                "standard impl duplicates SQL/default/error logic instead of delegating one core: "
                f"{row.implementation_type}: {markers}"
            )
    if target_leaf == "Connection":
        required = ("create_statement", "prepare_statement")
        if not standard or any(name not in standard[0] for name in required):
            fail(
                f"connection lacks required standard handle behavior: {row.implementation_type}"
            )
        family_connections = {
            "RbdcConnectionAdapter",
            "ToastyConnectionAdapter",
            "DuckDbConnectionAdapter",
            "HttpSqlConnectionAdapter",
            "LibSqlConnectionAdapter",
            "JdbcAgentConnection",
        }
        if row.implementation_type == "SqlxConnectionAdapter":
            if "standard_actor_core" not in standard[0]:
                fail(
                    "SQLx connection does not create handles from its owned actor core"
                )
        elif concrete_type(row.implementation_type) in family_connections and (
            f"impl AdapterExecutionCore for {concrete_type(row.implementation_type)}"
            not in source
            or "StandardAdapterStatement::new" not in standard[0]
            or "StandardAdapterPreparedStatement::new" not in standard[0]
        ):
            fail(
                f"family connection lacks owned executor provenance: {row.implementation_type}"
            )
    required_methods = {
        "PreparedStatement": (
            "set_parameter",
            "execute_prepared_query",
            "execute_prepared_update",
        ),
        "ResultSet": ("next", "value", "close"),
        "DatabaseMetaData": ("database_product_name",),
        "ConnectionFactory": ("create",),
    }.get(target_leaf, ())
    if standard and any(method not in standard[0] for method in required_methods):
        fail(
            f"canonical impl lacks required behavior: {row.implementation_type}: {required_methods}"
        )
    owner, _, methods = row.standard_core_evidence.partition("::")
    production_sources = "\n".join(
        candidate.read_text(encoding="utf-8")
        for candidate in (root / "crates/druid-wrapper/src").rglob("*.rs")
    )
    if owner not in production_sources or any(
        method not in production_sources for method in methods.split("/") if method
    ):
        fail(f"adapter core evidence is not source-bound: {row.standard_core_evidence}")


def verify_consumer_rows(root: Path, path: Path) -> int:
    """Require every Task 4 wrapper consumer row to retain its pinned closed transition."""

    rows = [
        row
        for row in read_tsv(path)
        if row["target_task"] == TASK
        and row["source_path"].startswith("crates/druid-wrapper/")
    ]
    if len(rows) != 21:
        fail(
            f"Task 4 wrapper consumer denominator drift: expected 21 rows, found {len(rows)}"
        )
    for row in rows:
        if row["state"] != LIVE_CONSUMER_STATE:
            fail(
                f"Task 4 wrapper consumer is not CLOSED: {row['source_path']}: {row['symbol']}"
            )
        source = root / row["source_path"]
        if not source.is_file():
            fail(f"Task 4 wrapper consumer source is missing: {row['source_path']}")
        old_leaf = row["old_path"].rsplit("::", 1)[-1]
        if re.search(
            rf"\b{re.escape(old_leaf)}\b",
            code_tokens(source.read_text(encoding="utf-8")),
        ):
            fail(
                f"closed legacy consumer still exists in source: {row['source_path']}: {old_leaf}"
            )
    return len(rows)


def verify_sqlx_ownership(root: Path) -> None:
    """Require owned SQLx Statement/ResultSet/live metadata actor evidence."""

    actor = root / "crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs"
    if not actor.is_file():
        fail("SQLx statement-owned executor is missing")
    actor_source = code_tokens(actor.read_text(encoding="utf-8"))
    required_actor = (
        "struct SqlxConnectionActor",
        "Arc < SqlxConnectionActor >",
        "tokio :: sync :: Mutex",
        "Box < dyn ResultSet >",
        "fetch_rows_with_labels",
    )
    missing = [marker for marker in required_actor if marker not in actor_source]
    if missing:
        fail(f"SQLx actor ownership evidence is incomplete: {missing}")
    required_update_actor = (
        "async fn execute_update_result",
        ". execute_update_core ( sql , values )",
    )
    missing_update_actor = [
        marker for marker in required_update_actor if marker not in actor_source
    ]
    if missing_update_actor:
        fail(
            "SQLx canonical update actor/core call chain is incomplete: "
            f"{missing_update_actor}"
        )
    prepared = code_tokens(
        (root / "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs").read_text(
            encoding="utf-8"
        )
    )
    result_set = code_tokens(
        (root / "crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs").read_text(
            encoding="utf-8"
        )
    )
    metadata = code_tokens(
        (root / "crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs").read_text(
            encoding="utf-8"
        )
    )
    if "Arc < SqlxConnectionActor >" not in prepared:
        fail("SQLx Statement/PreparedStatement does not own its executor actor")
    if "actor . execute_update_result" not in prepared:
        fail("SQLx Statement bypasses the canonical update-result actor entry")
    if "Arc < SqlxConnectionActor >" not in result_set:
        fail("SQLx ResultSet does not retain its query actor/lease")
    if "Arc < SqlxConnectionActor >" not in metadata:
        fail("SQLx live DatabaseMetaData does not own its actor")
    if "ResultSetMetaDataSnapshot" not in result_set or not any(
        marker in result_set
        for marker in (
            "Arc < dyn ResultSetMetaData >",
            "Arc < dyn StandardResultSetMetaData >",
        )
    ):
        fail("SQLx ResultSet metadata is not an immutable Arc snapshot")
    if "Arc < dyn ParameterMetaData >" not in prepared:
        fail("SQLx parameter metadata is not an immutable Arc snapshot")


    async_source = code_tokens(
        (root / "crates/druid-wrapper/src/sqlx/sqlx_async_result_set.rs").read_text(
            encoding="utf-8"
        )
    )
    required_async = (
        "struct SqlxAsyncResultSet",
        "actor : Arc < SqlxConnectionActor >",
        "metadata : Option < Arc < ResultSetMetaDataSnapshot > >",
        "mpsc :: channel ( 1 )",
        "actor . spawn_cursor",
        "async fn next",
        ". try_send ( SqlxCursorRequest",
        "async fn close",
        "worker . await",
        "self . shutdown . send_replace ( true )",
    )
    missing_async = [marker for marker in required_async if marker not in async_source]
    if missing_async:
        fail(f"SQLx async ResultSet ownership/bounded pull evidence is incomplete: {missing_async}")
    if "Vec < Row >" in async_source:
        fail("SQLx async ResultSet retains an eager row vector")
    if "connection : Mutex < SqlxConnectionAdapter >" not in actor_source:
        fail("SQLx actor does not own the raw adapter under its Tokio mutex")


def verify_family_executor_call_chains(root: Path) -> None:
    """Bind coverage executors to the canonical update path used by evidence tests."""

    statement = code_tokens(
        (root / "crates/druid-wrapper/src/standard_adapter_statement.rs").read_text(
            encoding="utf-8"
        )
    )
    executor = code_tokens(
        (root / "crates/druid-wrapper/src/standard_adapter_executor.rs").read_text(
            encoding="utf-8"
        )
    )
    toasty = code_tokens(
        (
            root
            / "crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs"
        ).read_text(encoding="utf-8")
    )
    if "executor . execute_update_result" not in statement:
        fail("canonical Statement bypasses the shared update-result executor")
    if ". standard_execute_update_result_core ( sql , values )" not in executor:
        fail("shared executor bypasses the adapter update-result core")
    required_toasty = (
        "async fn standard_execute_update_result_core",
        "self . execute_update_core ( sql , values )",
    )
    missing_toasty = [marker for marker in required_toasty if marker not in toasty]
    if missing_toasty:
        fail(
            "Toasty canonical update-result executor/core call chain is incomplete: "
            f"{missing_toasty}"
        )


def verify_repository(
    root: Path,
    adapters_path: Path = DEFAULT_ADAPTERS,
    runtime_evidence_path: Path = DEFAULT_RUNTIME_EVIDENCE,
    coverage_manifest_path: Path = DEFAULT_COVERAGE_MANIFEST,
    consumers_path: Path = DEFAULT_CONSUMERS,
) -> tuple[int, int, int]:
    """Verify source identity, anti-forgery, and exact runtime evidence for Task 4."""

    adapters = task_adapter_rows(root / adapters_path, root / runtime_evidence_path)
    for row in adapters:
        verify_adapter_source(root, row)
    consumers = verify_consumer_rows(root, root / consumers_path)
    verify_sqlx_ownership(root)
    verify_family_executor_call_chains(root)
    verify_trace_record_callsites(root)
    specs = evidence_specs(adapters)
    coverage_family_specs = load_coverage_manifest(root, coverage_manifest_path)
    verify_coverage_manifest_bindings(specs, coverage_family_specs)
    for spec in specs:
        execute_evidence(root, spec, coverage_family_specs)
    return (
        sum(row.state == LIVE_ADAPTER_STATE for row in adapters),
        consumers,
        sum(spec.category == RUNTIME_TRACE_CATEGORY for spec in specs),
    )


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    arguments = parser.parse_args()
    try:
        adapter_count, consumer_count, trace_count = verify_repository(
            arguments.root.resolve()
        )
    except (OSError, UnicodeError, VerificationError) as error:
        print(f"WRAPPER_STANDARD_IMPLS_FAIL={error}")
        return 1
    print(f"WRAPPER_STANDARD_ADAPTERS_LIVE={adapter_count}")
    print(f"WRAPPER_STANDARD_CONSUMERS_CLOSED={consumer_count}")
    print("FAMILY_COMMON_PREPARED_CHAINS_LIVE=6")
    print("SQLX_OWNED_ACTOR_LIVE=1")
    print(f"RUNTIME_TRACE_FAMILIES_LIVE={trace_count}")
    print(f"RUNTIME_TRACE_PROTOCOL={TRACE_VERSION}")
    print(f"COMPILER_COVERAGE_FAMILIES_LIVE={trace_count}")
    print(f"COMPILER_COVERAGE_PROTOCOL={COVERAGE_VERSION}")
    print("NONBEHAVIOR_IDENTITY_ROWS=2")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
