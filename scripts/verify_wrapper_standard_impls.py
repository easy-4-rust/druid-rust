#!/usr/bin/env python3
"""Verify P04 Task 4 wrapper standard identity and typed runtime-trace evidence."""

from __future__ import annotations

import argparse
import csv
import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import tokenize


DEFAULT_ADAPTERS = Path("docs/verification/rdbc-first/rdbc-adapter-implementations.tsv")
DEFAULT_RUNTIME_EVIDENCE = Path(
    "docs/verification/rdbc-first/rdbc-adapter-runtime-traces.tsv"
)
DEFAULT_CONSUMERS = Path("docs/verification/rdbc-first/rdbc-production-consumers.tsv")
TASK = "P04_TASK_4_ADAPTER_CUTOVER"
LIVE_ADAPTER_STATE = "STANDARD_IMPL_LIVE"
LIVE_CONSUMER_STATE = "STANDARD_REFERENCE_CLOSED"
RUNTIME_TRACE_CATEGORY = "RUNTIME_TRACE"
COMPILE_IDENTITY_CATEGORY = "COMPILE_IDENTITY"
TRACE_VERSION = "task4-runtime-trace-v1"
TRACE_MARKER = "DRUID_TASK4_RUNTIME_TRACE"
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
TRACE_RECORD_ALLOWED_PATHS = {
    "crates/druid-wrapper/src/standard_adapter_executor.rs",
    "crates/druid-wrapper/src/standard_adapter_statement.rs",
    "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
    "crates/druid-wrapper/src/standard_adapter_result_set.rs",
    "crates/druid-wrapper/src/standard_adapter_database_meta_data.rs",
    "crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs",
    "crates/druid-wrapper/src/sqlx/sqlx_connection_factory.rs",
    "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
    "crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs",
    "crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs",
    "crates/druid-wrapper/src/toasty/toasty_connection_factory.rs",
    "crates/druid-wrapper/src/rbdc/rbdc_connection_factory.rs",
    "crates/druid-wrapper/src/duckdb/duckdb_connection_factory.rs",
    "crates/druid-wrapper/src/http_sql/http_sql_connection_factory.rs",
    "crates/druid-wrapper/src/libsql/libsql_connection_factory.rs",
    "crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection_factory.rs",
}
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


def read_tsv(path: Path) -> list[dict[str, str]]:
    """Read one canonical TSV while rejecting malformed rows."""

    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n") or b"\r" in raw:
        fail(f"TSV must have exactly one LF final newline: {path}")
    rows = list(csv.DictReader(raw.decode("utf-8").splitlines(), delimiter="\t"))
    if not rows or any(None in row or any(value == "" for value in row.values()) for row in rows):
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
        if row["target_task"] == TASK and row["owner"] == "WRAPPER_ADAPTER"
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
            behavior_test_target=evidence_by_key[(row["source_path"], row["target_trait"])][
                "cargo_test_target"
            ],
            cargo_test_id=evidence_by_key[(row["source_path"], row["target_trait"])][
                "cargo_test_id"
            ],
            behavior_profile=evidence_by_key[(row["source_path"], row["target_trait"])][
                "behavior_profile"
            ],
            evidence_category=evidence_by_key[(row["source_path"], row["target_trait"])][
                "evidence_category"
            ],
            trace_family=evidence_by_key[(row["source_path"], row["target_trait"])][
                "trace_family"
            ],
            trace_version=evidence_by_key[(row["source_path"], row["target_trait"])][
                "trace_version"
            ],
            trace_evidence_id=evidence_by_key[(row["source_path"], row["target_trait"])][
                "trace_evidence_id"
            ],
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


def validate_runtime_trace_output(
    output: str,
    expected_evidence_id: str,
    expected_family: str,
    expected_version: str,
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
    required_keys = {"evidence_id", "version", "family", "instance_id", "events"}
    if not isinstance(payload, dict) or set(payload) != required_keys:
        fail("runtime trace payload schema is not exact")
    if payload["evidence_id"] != expected_evidence_id:
        fail("runtime trace evidence id does not match the TSV binding")
    if payload["version"] != expected_version or expected_version != TRACE_VERSION:
        fail("runtime trace protocol version is not the unique Task 4 version")
    if payload["family"] != expected_family or expected_family not in TRACE_FAMILIES:
        fail("runtime trace family does not match the TSV binding")
    instance_id = payload["instance_id"]
    if isinstance(instance_id, bool) or not isinstance(instance_id, int) or instance_id <= 0:
        fail("runtime trace instance id must be a positive integer")
    events = payload["events"]
    if not isinstance(events, list) or len(events) != len(TRACE_STAGES):
        fail("runtime trace event denominator is incomplete or duplicated")
    observed_stages: list[str] = []
    for event in events:
        if not isinstance(event, dict) or set(event) != {"family", "instance_id", "stage"}:
            fail("runtime trace event schema is not exact")
        if event["family"] != expected_family or event["instance_id"] != instance_id:
            fail("runtime trace splices different family or connection instances")
        if not isinstance(event["stage"], str):
            fail("runtime trace stage is not typed as a string label")
        observed_stages.append(event["stage"])
    if tuple(observed_stages) != TRACE_STAGES:
        fail("runtime trace stages are missing, duplicated, or out of order")


def verify_runtime_trace_source_hygiene(path: Path, source: str) -> None:
    """Reject marker/event/record construction from evidence tests and I/O doubles."""

    forbidden_literals = (
        TRACE_MARKER,
        "RuntimeTraceEmitter",
        "TraceStage",
        "TraceEvent",
        "record_stage",
        *TRACE_STAGES,
    )
    found = [literal for literal in forbidden_literals if literal in source]
    tokens = tuple(token.text for token in tokenize(source))
    found.extend(method for method in TRACE_RECORD_METHODS if method in tokens)
    if found:
        fail(f"runtime evidence source may not forge trace internals: {path}: {sorted(set(found))}")


def verify_trace_record_callsites(root: Path) -> None:
    """Allow typed stage recording only at approved production execution modules."""

    source_root = root / "crates/druid-wrapper/src"
    for path in source_root.rglob("*.rs"):
        relative = path.relative_to(root).as_posix()
        source = path.read_text(encoding="utf-8")
        tokens = tuple(token.text for token in tokenize(source))
        calls = {
            tokens[index + 1]
            for index in range(len(tokens) - 2)
            if tokens[index] == "."
            and tokens[index + 1] in TRACE_RECORD_METHODS
            and tokens[index + 2] == "("
        }
        if calls and relative not in TRACE_RECORD_ALLOWED_PATHS:
            fail(f"runtime trace record callsite is outside production allowlist: {relative}: {sorted(calls)}")
        test_region = source.find("#[cfg(test)]")
        if test_region >= 0:
            verify_runtime_trace_source_hygiene(Path(relative), source[test_region:])
    trace_module = source_root / "task4_runtime_trace.rs"
    for path in source_root.rglob("*.rs"):
        if path == trace_module:
            continue
        if TRACE_MARKER in path.read_text(encoding="utf-8"):
            fail(f"runtime trace marker serialization escaped its owner module: {path.relative_to(root)}")


def evidence_source(root: Path, spec: EvidenceSpec) -> tuple[Path, str]:
    """Resolve one exact Cargo target/test id to source used for anti-forgery scanning."""

    function_name = spec.test_id.rsplit("::", 1)[-1]
    if spec.test_target == "lib":
        candidates = [
            path
            for path in (root / "crates/druid-wrapper/src").rglob("*.rs")
            if re.search(rf"\bfn\s+{re.escape(function_name)}\s*\(", path.read_text(encoding="utf-8"))
        ]
        if len(candidates) != 1:
            fail(f"runtime evidence lib test source is not exact: {spec.test_id}: {candidates}")
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


def cargo_evidence_command(spec: EvidenceSpec) -> list[str]:
    """Build the one exact Cargo command declared by a TSV evidence row."""

    command = ["cargo", "test", "-p", "druid-wrapper"]
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
    command.extend([spec.test_id, "--", "--exact", "--nocapture"])
    return command


def execute_evidence(root: Path, spec: EvidenceSpec) -> None:
    """Run one exact declared test and validate its runtime-trace or compile-identity outcome."""

    path, source = evidence_source(root, spec)
    verify_runtime_trace_source_hygiene(path.relative_to(root), source)
    result = subprocess.run(
        cargo_evidence_command(spec),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    output = f"{result.stdout}\n{result.stderr}"
    if result.returncode != 0:
        fail(f"exact Task 4 evidence test failed: {spec.test_id}: {output.strip()}")
    if not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", output):
        fail(f"exact Task 4 evidence test did not complete through libtest: {spec.test_id}")
    if spec.category == RUNTIME_TRACE_CATEGORY:
        validate_runtime_trace_output(output, spec.evidence_id, spec.family, spec.version)
    elif spec.category == COMPILE_IDENTITY_CATEGORY:
        if TRACE_MARKER in output:
            fail(f"nonbehavior identity evidence forged runtime behavior: {spec.test_id}")
    else:
        fail(f"Task 4 evidence category is unknown: {spec.category}")


def evidence_specs(rows: list[AdapterRow]) -> tuple[EvidenceSpec, ...]:
    """Validate row bindings and return one deterministic set of exact tests to execute."""

    specs = {
        EvidenceSpec(
            row.behavior_test_target,
            row.cargo_test_id,
            row.behavior_profile,
            row.evidence_category,
            row.trace_family,
            row.trace_version,
            row.trace_evidence_id,
        )
        for row in rows
        if row.state == LIVE_ADAPTER_STATE
    }
    runtime = [spec for spec in specs if spec.category == RUNTIME_TRACE_CATEGORY]
    identity = [spec for spec in specs if spec.category == COMPILE_IDENTITY_CATEGORY]
    if len(runtime) != 7 or {spec.family for spec in runtime} != TRACE_FAMILIES:
        fail(f"Task 4 must bind exactly seven family runtime traces: {runtime}")
    if len(identity) != 1:
        fail(f"Task 4 DataSource Wrapper identity must use one independent exact test: {identity}")
    if sum(row.evidence_category == COMPILE_IDENTITY_CATEGORY for row in rows) != 2:
        fail("Task 4 must classify exactly two DataSource rows as nonbehavior identity evidence")
    for spec in runtime:
        if spec.version != TRACE_VERSION or spec.family not in TRACE_FAMILIES:
            fail(f"runtime trace binding is not canonical: {spec}")
        if spec.evidence_id in {"", "-"}:
            fail(f"runtime trace evidence id is missing: {spec}")
    for spec in identity:
        if (spec.family, spec.version, spec.evidence_id) != ("-", "-", "-"):
            fail(f"compile identity row must not claim runtime trace fields: {spec}")
    exact_bindings: dict[tuple[str, str, str], EvidenceSpec] = {}
    evidence_ids: set[str] = set()
    for spec in specs:
        key = (spec.test_target, spec.test_id, spec.profile)
        previous = exact_bindings.setdefault(key, spec)
        if previous != spec:
            fail(f"one exact Cargo test/profile is bound to conflicting evidence: {key}")
        if spec.category == RUNTIME_TRACE_CATEGORY and spec.evidence_id in evidence_ids:
            fail(f"runtime trace evidence id is duplicated: {spec.evidence_id}")
        evidence_ids.add(spec.evidence_id)
    return tuple(sorted(specs))


def has_wrapper_macro(source: str, type_name: str, target_trait: str) -> bool:
    """Return whether the canonical Wrapper macro covers this implementation type."""

    concrete = concrete_type(type_name)
    leaf = target_trait.rsplit("::", 1)[-1]
    return re.search(
        rf"impl_standard_wrapper!\s*\(\s*{re.escape(concrete)}(?:\s*<[^;)]*>)?\s*=>\s*{re.escape(leaf)}\s*\)",
        source,
    ) is not None


def verify_adapter_source(root: Path, row: AdapterRow) -> None:
    """Require parallel Physical/standard impls and a single production execution core."""

    path = root / row.source_path
    if not path.is_file():
        fail(f"adapter source is missing: {row.source_path}")
    source = path.read_text(encoding="utf-8")
    tokens = code_tokens(source)
    current = extract_impl_bodies(source, row.current_trait, row.implementation_type)
    if not current and row.current_trait.rsplit("::", 1)[-1] != "Wrapper":
        fail(f"temporary Physical impl missing: {row.implementation_type} in {row.source_path}")
    if row.state != LIVE_ADAPTER_STATE:
        if row.state != "CURRENT_IMPL_TARGET_PLANNED":
            fail(f"adapter inventory row has unknown state: {row.implementation_type}: {row.state}")
        return
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
            fail(f"common canonical Prepared behavior is incomplete: {row.implementation_type}")
        owner = row.standard_core_evidence.split("::", 1)[0]
        family_sources = list((root / "crates/druid-wrapper/src").rglob("*connection*.rs"))
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
            fail(f"family prepared executor provenance is missing: {row.implementation_type}")
        return
    target_leaf = row.target_trait.rsplit("::", 1)[-1]
    standard = extract_impl_bodies(source, row.target_trait, row.implementation_type)
    wrapper_only = target_leaf == "Wrapper" and has_wrapper_macro(
        source, row.implementation_type, row.target_trait
    )
    if not standard and not wrapper_only:
        fail(
            f"canonical adapter implements only Physical*: {row.implementation_type} "
            f"missing {row.target_trait}"
        )
    canonical_path = row.target_trait.replace("druid_rdbc::", "druid_rdbc :: ").replace("::", " :: ")
    canonical_import = row.target_trait.rsplit("::", 1)[0].replace("::", " :: ") + f" :: {target_leaf}"
    macro_provenance = "druid_rdbc :: impl_standard_wrapper" in tokens and f"=> {target_leaf}" in tokens
    if canonical_path not in tokens and canonical_import not in tokens and not macro_provenance:
        fail(f"standard impl lacks canonical trait provenance: {row.implementation_type}")
    if re.search(rf"\btrait\s+{re.escape(target_leaf)}\b", source):
        fail(f"local same-leaf trait shadows canonical identity: {target_leaf}")
    if re.search(r"#\s*\[\s*cfg\s*\(\s*any\s*\(\s*\)\s*\)\s*\]\s*impl", source):
        fail(f"canonical impl is production-disabled: {row.implementation_type}")
    for body in standard:
        if not code_tokens(body).strip():
            fail(f"canonical impl inherits only empty/default behavior: {row.implementation_type}")
        if "Physical" in body:
            fail(f"standard impl delegates back to Physical: {row.implementation_type}")
        markers = [marker for marker in SQL_EXECUTION_MARKERS if marker in body]
        delegates = "_core(" in body or "actor" in body.lower()
        if markers and not delegates:
            fail(
                "standard impl duplicates SQL/default/error logic instead of delegating one core: "
                f"{row.implementation_type}: {markers}"
            )
    current_cores = set().union(*(core_calls(body) for body in current)) if current else set()
    standard_cores = set().union(*(core_calls(body) for body in standard)) if standard else set()
    if target_leaf in {"ConnectionFactory", "DatabaseMetaData", "ResultSet"}:
        transitive_factory_core = (
            target_leaf == "ConnectionFactory"
            and "create_adapter_core" in source
            and standard
            and "create_adapter_core" in standard[0]
        )
        if not current_cores.intersection(standard_cores) and not transitive_factory_core:
            fail(f"standard and Physical impls use split cores: {row.implementation_type}")
    if target_leaf == "Connection":
        required = ("create_statement", "prepare_statement")
        if not standard or any(name not in standard[0] for name in required):
            fail(f"connection lacks required standard handle behavior: {row.implementation_type}")
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
                fail("SQLx connection does not create handles from its owned actor core")
        elif concrete_type(row.implementation_type) in family_connections and (
            f"impl AdapterExecutionCore for {concrete_type(row.implementation_type)}" not in source
            or "StandardAdapterStatement::new" not in standard[0]
            or "StandardAdapterPreparedStatement::new" not in standard[0]
        ):
            fail(f"family connection lacks owned executor provenance: {row.implementation_type}")
    required_methods = {
        "PreparedStatement": ("set_parameter", "execute_prepared_query", "execute_prepared_update"),
        "ResultSet": ("next", "value", "close"),
        "DatabaseMetaData": ("database_product_name",),
        "ConnectionFactory": ("create",),
    }.get(target_leaf, ())
    if standard and any(method not in standard[0] for method in required_methods):
        fail(f"canonical impl lacks required behavior: {row.implementation_type}: {required_methods}")
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
        if row["target_task"] == TASK and row["source_path"].startswith("crates/druid-wrapper/")
    ]
    if len(rows) != 21:
        fail(f"Task 4 wrapper consumer denominator drift: expected 21 rows, found {len(rows)}")
    for row in rows:
        if row["state"] != LIVE_CONSUMER_STATE:
            fail(f"Task 4 wrapper consumer is not CLOSED: {row['source_path']}: {row['symbol']}")
        source = root / row["source_path"]
        if not source.is_file():
            fail(f"Task 4 wrapper consumer source is missing: {row['source_path']}")
        old_leaf = row["old_path"].rsplit("::", 1)[-1]
        if re.search(rf"\b{re.escape(old_leaf)}\b", code_tokens(source.read_text(encoding="utf-8"))):
            fail(f"closed legacy consumer still exists in source: {row['source_path']}: {old_leaf}")
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
    prepared = code_tokens(
        (root / "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs").read_text(encoding="utf-8")
    )
    result_set = code_tokens(
        (root / "crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs").read_text(encoding="utf-8")
    )
    metadata = code_tokens(
        (root / "crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs").read_text(encoding="utf-8")
    )
    if "Arc < SqlxConnectionActor >" not in prepared:
        fail("SQLx Statement/PreparedStatement does not own its executor actor")
    if "Arc < SqlxConnectionActor >" not in result_set:
        fail("SQLx ResultSet does not retain its query actor/lease")
    if "Arc < SqlxConnectionActor >" not in metadata:
        fail("SQLx live DatabaseMetaData does not own its actor")
    if "ResultSetMetaDataSnapshot" not in result_set or not any(
        marker in result_set
        for marker in ("Arc < dyn ResultSetMetaData >", "Arc < dyn StandardResultSetMetaData >")
    ):
        fail("SQLx ResultSet metadata is not an immutable Arc snapshot")
    if "Arc < dyn ParameterMetaData >" not in prepared:
        fail("SQLx parameter metadata is not an immutable Arc snapshot")


def verify_repository(
    root: Path,
    adapters_path: Path = DEFAULT_ADAPTERS,
    runtime_evidence_path: Path = DEFAULT_RUNTIME_EVIDENCE,
    consumers_path: Path = DEFAULT_CONSUMERS,
) -> tuple[int, int, int]:
    """Verify source identity, anti-forgery, and exact runtime evidence for Task 4."""

    adapters = task_adapter_rows(root / adapters_path, root / runtime_evidence_path)
    for row in adapters:
        verify_adapter_source(root, row)
    consumers = verify_consumer_rows(root, root / consumers_path)
    verify_sqlx_ownership(root)
    verify_trace_record_callsites(root)
    specs = evidence_specs(adapters)
    for spec in specs:
        execute_evidence(root, spec)
    return (
        sum(row.state == LIVE_ADAPTER_STATE for row in adapters),
        consumers,
        sum(spec.category == RUNTIME_TRACE_CATEGORY for spec in specs),
    )


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    arguments = parser.parse_args()
    try:
        adapter_count, consumer_count, trace_count = verify_repository(arguments.root.resolve())
    except (OSError, UnicodeError, VerificationError) as error:
        print(f"WRAPPER_STANDARD_IMPLS_FAIL={error}")
        return 1
    print(f"WRAPPER_STANDARD_ADAPTERS_LIVE={adapter_count}")
    print(f"WRAPPER_STANDARD_CONSUMERS_CLOSED={consumer_count}")
    print("FAMILY_COMMON_PREPARED_CHAINS_LIVE=6")
    print("SQLX_OWNED_ACTOR_LIVE=1")
    print(f"RUNTIME_TRACE_FAMILIES_LIVE={trace_count}")
    print(f"RUNTIME_TRACE_PROTOCOL={TRACE_VERSION}")
    print("NONBEHAVIOR_IDENTITY_ROWS=2")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
