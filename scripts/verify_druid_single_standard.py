#!/usr/bin/env python3
"""Verify P04 Task 5 Druid logical proxies and Druid-only runtime cutover."""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict, deque
import hashlib
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import VerificationError as RustLexError
from verify_rdbc_public_paths import tokenize
from verify_rdbc_standardization_denominator import (
    AdapterRow,
    DEFAULT_ADAPTERS,
    DEFAULT_CONSUMERS,
    P03_MANIFEST,
    ConsumerRow,
    VerificationError as DenominatorVerificationError,
    derive_live_manifests,
    parse_consumer_payload,
    parse_adapter_payload,
    render_adapter_rows,
    render_consumer_rows,
)


TASK5_BASE = "d1ccc8e3005a3a71638766ee701af99594fc43ed"
TASK5 = "P04_TASK_5_DRUID_INTERNALIZATION"
TASK5_CLOSED = "DRUID_INTERNAL_REFERENCE_CLOSED"
TASK6_PENDING = "TASK6_COMPATIBILITY_PENDING"
TASK7_PENDING = "TASK7_RESOURCE_OPERATIONS_PENDING"
TASK5_BASE_CONSUMER_SHA256 = (
    "241a2d42b74e27ae2b0e41729d3088e94f464bfe7984699073a20f834ce908bc"
)
DRUID_PARTITION_NORMALIZED_SHA256 = (
    "cd2ecd715546f1f2da5fe51cbf0f79a47dc73b240252120e8d841aa305ed6b82"
)
TASK6_SHIMMED_FROZEN_NORMALIZED_SHA256 = (
    "9d6556397b230e5e76aa8acc006747d24e54fcdd4c79c11ed9cc8246b400f5f9"
)
TASK6_ADDITION_NORMALIZED_SHA256 = (
    "773722c3231615e5227eea118f046b47c0fa73f7dddfd9cc1486a4edfb2f64c7"
)
TASK4_COVERAGE_MANIFEST_SHA256 = (
    "f7b3d4b429fe9e5c4bfe954082f0518d8e78635f6f7cc21bde1ffcd40f2307c9"
)
TASK4_RUNTIME_MANIFEST_SHA256 = (
    "106323e17904d377172986ce7464a54de60b4bb575e5f67e333e37f16188a3ec"
)
TASK5_RBDC_EVIDENCE_SOURCE_SHA256 = (
    "b3fc8750e3cfaf552096cd3c0db0501f0bc312a52b0ad6b4b9af9e94d9daf96c"
)
DRUID_TASK6_COMPAT_PATHS = {
    "crates/druid/src/core/mod.rs",
    "crates/druid/src/core/physical_connection_info.rs",
    "crates/druid/src/core/prepared_statement_key.rs",
    "crates/druid/src/spi/mod.rs",
    "crates/druid/src/spi/rdbc_resource_capabilities.rs",
    "crates/druid/src/spi/rdbc_resource_context.rs",
    "crates/druid/src/spi/rdbc_resource_factory.rs",
    "crates/druid/src/spi/rdbc_resource_id.rs",
    "crates/druid/src/spi/rdbc_resource_kind.rs",
    "crates/druid/src/spi/rdbc_resource_owner.rs",
    "crates/druid/src/spi/rdbc_resource_state.rs",
}

FIXED_FILES = (
    "crates/druid/src/core/druid_connection_holder.rs",
    "crates/druid/src/core/druid_pooled_connection.rs",
    "crates/druid/src/core/druid_pooled_statement.rs",
    "crates/druid/src/core/druid_pooled_prepared_statement.rs",
    "crates/druid/src/core/druid_pooled_callable_statement.rs",
    "crates/druid/src/core/druid_pooled_result_set.rs",
    "crates/druid/src/core/database_meta_data_proxy_impl.rs",
    "crates/druid/src/core/physical_connection_info.rs",
    "crates/druid/src/core/prepared_statement_key.rs",
    "crates/druid/src/core/prepared_statement_method_type.rs",
    "crates/druid/src/core/resource_context.rs",
    "crates/druid/src/core/resource_factory.rs",
    "crates/druid/src/core/resource_owner.rs",
    "crates/druid/src/core/resource_id.rs",
    "crates/druid/src/core/resource_kind.rs",
    "crates/druid/src/core/resource_state.rs",
    "crates/druid/src/core/resource_runtime_capabilities.rs",
    "crates/druid/src/core/wrapper_proxy_hooks.rs",
    "crates/druid/src/core/mysql_metadata_compatibility.rs",
    "crates/druid/src/core/standard_prepared_statement_cache.rs",
    "crates/druid/tests/single_standard_logical_contract_test.rs",
    "crates/druid/tests/raw_logical_ownership_test.rs",
    "crates/druid/tests/standard_metadata_ownership_test.rs",
    "docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv",
    "docs/verification/rdbc-first/rdbc-production-consumers.tsv",
)

CONCRETE_OBJECTS = {
    "crates/druid/src/core/physical_connection_info.rs": "PhysicalConnectionInfo",
    "crates/druid/src/core/prepared_statement_key.rs": "PreparedStatementKey",
    "crates/druid/src/core/prepared_statement_method_type.rs": "PreparedStatementMethodType",
    "crates/druid/src/core/resource_context.rs": "ResourceContext",
    "crates/druid/src/core/resource_factory.rs": "ResourceFactory",
    "crates/druid/src/core/resource_owner.rs": "ResourceOwner",
    "crates/druid/src/core/resource_id.rs": "ResourceId",
    "crates/druid/src/core/resource_kind.rs": "ResourceKind",
    "crates/druid/src/core/resource_state.rs": "ResourceState",
    "crates/druid/src/core/resource_runtime_capabilities.rs": "ResourceRuntimeCapabilities",
    "crates/druid/src/core/wrapper_proxy_hooks.rs": "WrapperProxyHooks",
    "crates/druid/src/core/mysql_metadata_compatibility.rs": "MysqlMetadataCompatibility",
    "crates/druid/src/core/standard_prepared_statement_cache.rs": "StandardPreparedStatementCache",
}

STANDARD_IMPLS = {
    "crates/druid/src/core/druid_pooled_connection.rs": (
        "druid_rdbc::connection::Connection",
        "DruidPooledConnection",
        {
            "create_statement",
            "prepare_statement",
            "database_meta_data",
            "close",
            "is_closed",
        },
    ),
    "crates/druid/src/core/druid_pooled_statement.rs": (
        "druid_rdbc::statement::Statement",
        "DruidPooledStatement",
        {"execute_query", "execute_update", "close", "is_closed"},
    ),
    "crates/druid/src/core/druid_pooled_prepared_statement.rs": (
        "druid_rdbc::statement::PreparedStatement",
        "DruidPooledPreparedStatement",
        {"set_parameter", "execute_prepared_query", "execute_prepared_update"},
    ),
    "crates/druid/src/core/druid_pooled_callable_statement.rs": (
        "druid_rdbc::statement::CallableStatement",
        "DruidPooledCallableStatement",
        {"register_out_parameter", "out_value"},
    ),
    "crates/druid/src/core/druid_pooled_result_set.rs": (
        "druid_rdbc::result_set::ResultSet",
        "DruidPooledResultSet",
        {"next", "value", "meta_data", "close", "is_closed"},
    ),
    "crates/druid/src/core/database_meta_data_proxy_impl.rs": (
        "druid_rdbc::metadata::DatabaseMetaData",
        "DatabaseMetaDataProxyImpl",
        {"database_product_name", "tables"},
    ),
}


class VerificationError(RuntimeError):
    """Raised when Task 5 evidence is missing, stale, or fail-open."""


@dataclass(frozen=True)
class VerificationSummary:
    """Current live/closed Task 5 consumer and proxy counts."""

    consumer_rows: int
    closed_rows: int
    task6_pending_rows: int
    task7_pending_rows: int
    standard_impls: int
    concrete_objects: int


def fail(message: str) -> NoReturn:
    raise VerificationError(message)


def repository_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _tokens(source: str) -> list[object]:
    try:
        return tokenize(source)
    except RustLexError as error:
        fail(f"cannot tokenize Rust source: {error}")


def _balanced_block(tokens: list[object], opening: int) -> tuple[int, int]:
    if opening >= len(tokens) or tokens[opening].text != "{":
        fail("expected a Rust block")
    depth = 1
    cursor = opening + 1
    while cursor < len(tokens) and depth:
        depth += tokens[cursor].text == "{"
        depth -= tokens[cursor].text == "}"
        cursor += 1
    if depth:
        fail("unterminated Rust block")
    return opening, cursor - 1


def _normalized(tokens: list[object], start: int, end: int) -> str:
    return "".join(token.text for token in tokens[start:end])


def _impl_blocks(
    source: str, trait_leaf: str, type_name: str
) -> list[tuple[list[object], int, int, int]]:
    tokens = _tokens(source)
    blocks: list[tuple[list[object], int, int, int]] = []
    for index, token in enumerate(tokens):
        if token.text != "impl":
            continue
        cursor = index + 1
        header: list[str] = []
        while cursor < len(tokens) and tokens[cursor].text not in {"{", ";"}:
            header.append(tokens[cursor].text)
            cursor += 1
        compact = "".join(header)
        if cursor >= len(tokens) or tokens[cursor].text != "{":
            continue
        if "for" not in header or type_name not in compact:
            continue
        before_for = compact.split("for", 1)[0]
        if before_for != trait_leaf and not before_for.endswith(f"::{trait_leaf}"):
            continue
        opening, closing = _balanced_block(tokens, cursor)
        blocks.append((tokens, index, opening, closing))
    return blocks


def _method_bodies(tokens: list[object], opening: int, closing: int) -> dict[str, str]:
    methods: dict[str, str] = {}
    cursor = opening + 1
    while cursor < closing:
        if tokens[cursor].text not in {"fn", "async"}:
            cursor += 1
            continue
        fn_index = cursor + 1 if tokens[cursor].text == "async" else cursor
        if (
            fn_index >= closing
            or tokens[fn_index].text != "fn"
            or fn_index + 1 >= closing
        ):
            cursor += 1
            continue
        name = tokens[fn_index + 1].text
        body_open = fn_index + 2
        paren = bracket = 0
        while body_open < closing:
            text = tokens[body_open].text
            paren += text == "("
            paren -= text == ")"
            bracket += text == "["
            bracket -= text == "]"
            if text == "{" and paren == bracket == 0:
                break
            body_open += 1
        if body_open >= closing:
            cursor += 1
            continue
        _, body_close = _balanced_block(tokens, body_open)
        methods[name] = _normalized(tokens, body_open + 1, body_close)
        cursor = body_close + 1
    return methods


def require_concrete_object(source: str, type_name: str, label: str) -> None:
    """Reject aliases/re-exports/stubs in a required one-object Druid module."""

    if re.search(
        rf"\b(?:type\s+{re.escape(type_name)}\b|pub\s+use\b[^;]*\b{re.escape(type_name)}\b)",
        source,
    ):
        fail(
            f"{label} must define a concrete object for {type_name}, not an alias/facade"
        )
    match = re.search(
        rf"\b(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum|trait)\s+{re.escape(type_name)}\b",
        source,
    )
    if match is None:
        fail(f"{label} must define a concrete object for {type_name}")
    tail = source[match.end() :]
    if not re.search(r"(?:\([^)]*\)|\{[^}]+\})", tail, re.DOTALL):
        fail(f"{label} concrete object {type_name} is empty")


def require_direct_standard_impl(
    source: str,
    trait_path: str,
    type_name: str,
    required_methods: set[str],
    label: str,
) -> None:
    """Require one enabled, non-default, Physical-free direct canonical impl."""

    trait_leaf = trait_path.rsplit("::", 1)[-1]
    blocks = _impl_blocks(source, trait_leaf, type_name)
    if len(blocks) != 1:
        fail(
            f"{label} must contain exactly one direct {trait_path} impl for {type_name}"
        )
    tokens, impl_index, opening, closing = blocks[0]
    prefix = _normalized(tokens, max(0, impl_index - 20), impl_index)
    if "cfg(any())" in prefix or "cfg(test)" in prefix or "cfg_attr(any()" in prefix:
        fail(f"{label} canonical identity is cfg-disabled")
    methods = _method_bodies(tokens, opening, closing)
    missing = sorted(required_methods - set(methods))
    if missing:
        fail(f"{label} direct canonical impl misses methods: {missing}")
    physical_aliases = set(
        re.findall(r"\bPhysical[A-Za-z0-9_]*\s+as\s+([A-Za-z_][A-Za-z0-9_]*)", source)
    )
    body = _normalized(tokens, opening + 1, closing)
    if "Physical" in body or any(
        re.search(rf"\b{re.escape(alias)}\b", body) for alias in physical_aliases
    ):
        fail(f"{label} canonical impl depends on hidden Physical identity")
    default_success = {
        "Ok(())",
        "Ok(false)",
        "Ok(true)",
        "Ok(Vec::new())",
        "Ok(Default::default())",
        "Default::default()",
    }
    for method in required_methods:
        normalized = methods[method]
        if (
            not normalized
            or normalized in default_success
            or "todo!" in normalized
            or "unimplemented!" in normalized
        ):
            fail(f"{label} method {method} is empty/default-success")
    for physical_trait in (
        "PhysicalConnection",
        "PhysicalStatement",
        "PhysicalPreparedStatement",
        "PhysicalCallableStatement",
        "PhysicalResultSet",
        "PhysicalDatabaseMetaData",
    ):
        for other_tokens, _, other_open, other_close in _impl_blocks(
            source, physical_trait, type_name
        ):
            other_methods = _method_bodies(other_tokens, other_open, other_close)
            for method in required_methods & set(other_methods):
                shared = methods[method]
                one_core_delegation = re.fullmatch(
                    r"self\.[a-z_][A-Za-z0-9_]*\([^;{}]*\)(?:\.await)?\??",
                    shared,
                )
                if (
                    shared == other_methods[method]
                    and len(shared) > 12
                    and one_core_delegation is None
                ):
                    fail(
                        f"{label} duplicated core for method {method}; delegate one private core"
                    )


def require_holder_contract(source: str, label: str) -> None:
    """Require the canonical connection slot and a private closed owner strategy."""

    compact = re.sub(r"\s+", "", source)
    if not re.search(
        r"connection:(?:Option<)?Box<dyn(?:druid_rdbc::connection::)?Connection>(?:>)?",
        compact,
    ):
        fail(f"{label} must own Box<dyn Connection>")
    owner = re.search(
        r"(?m)^\s*(?P<public>pub(?:\([^)]*\))?\s+)?enum\s+(?P<name>[A-Za-z0-9_]*Owner[A-Za-z0-9_]*)\s*\{(?P<body>[^}]*)\}",
        source,
    )
    if owner is None or {"NativePool", "ExternalLease"} - set(
        re.findall(r"\b[A-Z][A-Za-z0-9_]*\b", owner.group("body"))
    ):
        fail(f"{label} lacks NativePool | ExternalLease owner policy")
    if owner.group("public") is not None:
        fail(f"{label} owner strategy must remain a private owner policy")


def require_owner_close_contract(source: str, label: str) -> None:
    """Reject logical raw-close and external-to-native-pool ownership leaks."""

    compact = re.sub(r"\s+", "", source)
    if re.search(
        r"(?:fnclose_logical|impldruid_rdbc::connection::Connection).*?(?:close_standard_connection|close_raw_connection|raw\.close)",
        compact,
        re.DOTALL,
    ) or re.search(
        r"fnreturn_external.*?(?:native_pool|return_connection|return_native)",
        compact,
        re.DOTALL,
    ):
        fail(f"{label} violates logical/raw close ownership")
    owner_branch = ("NativePool" in source and "ExternalLease" in source) or (
        "is_external_lease" in source and "with_external_standard_connection" in source
    )
    if not owner_branch:
        fail(f"{label} does not branch on both owner policies")
    if not any(
        name in source
        for name in ("recycle_once", "release_logical_once", "logical_close_once")
    ):
        fail(f"{label} lacks exactly-once logical recycle core")


def require_fixed_files(root: Path) -> None:
    for relative in FIXED_FILES:
        if not (root / relative).is_file():
            fail(f"required Task 5 file is missing: {relative}")


def verify_task5_consumer_states(
    payload: bytes,
    *,
    expected_rows: int | None,
) -> tuple[ConsumerRow, ...]:
    """Require every dynamic Task 5 row to be exact LIVE or CLOSED evidence."""

    try:
        rows = tuple(parse_consumer_payload(payload))
    except DenominatorVerificationError as error:
        fail(str(error))
    task5 = tuple(row for row in rows if row.target_task == TASK5)
    if expected_rows is not None and len(task5) != expected_rows:
        fail(
            "dynamic Task 5 denominator mismatch: "
            f"expected {expected_rows}, found {len(task5)}"
        )
    invalid = sorted(
        {row.state for row in task5} - {TASK5_CLOSED, TASK6_PENDING, TASK7_PENDING}
    )
    if invalid:
        fail(f"invalid Task 5 consumer state: {invalid}")
    return task5


def _git_show(root: Path, revision: str, path: Path) -> bytes:
    result = subprocess.run(
        ["git", "show", f"{revision}:{path.as_posix()}"],
        cwd=root,
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        fail(f"cannot load Task 5 baseline consumer denominator: {detail}")
    return result.stdout


def classify_frozen_task5_row(row: ConsumerRow) -> str:
    """Apply the approved 180/69/108 partition to one frozen identity."""

    if row.source_path.startswith("crates/druid-rdbc/src/resource/"):
        if row.old_path == "druid_rdbc::resource::RdbcResourceCapabilities":
            return TASK7_PENDING
        return TASK5_CLOSED
    if row.source_path.startswith("crates/druid-rdbc/src/spi/"):
        return TASK6_PENDING
    if row.source_path.startswith("crates/druid-wrapper/"):
        return TASK6_PENDING
    if row.source_path.startswith("crates/druid/"):
        return (
            TASK6_PENDING
            if row.source_path in DRUID_TASK6_COMPAT_PATHS
            else TASK5_CLOSED
        )
    fail(f"Task 5 frozen row has an unclassified source path: {row.source_path}")


def require_frozen_partition(rows: tuple[ConsumerRow, ...]) -> None:
    """Pin the source bytes plus exact disposition/family counts."""

    druid_partition = []
    for row in rows:
        if not row.source_path.startswith("crates/druid/"):
            continue
        disposition = (
            "TASK5_MUST_CLOSE"
            if classify_frozen_task5_row(row) == TASK5_CLOSED
            else "TASK6_COMPAT"
        )
        druid_partition.append(
            "\t".join(
                (
                    disposition,
                    row.source_path,
                    row.occurrence_kind,
                    _symbol_identity(row.symbol),
                    row.old_path,
                    row.target_path,
                    row.old_path.rsplit("::", 1)[-1],
                )
            )
        )
    partition_payload = ("\n".join(sorted(druid_partition)) + "\n").encode()
    if (
        hashlib.sha256(partition_payload).hexdigest()
        != DRUID_PARTITION_NORMALIZED_SHA256
    ):
        fail("Task 5 pinned Druid 61/20 identity partition hash drift")

    counts = Counter(classify_frozen_task5_row(row) for row in rows)
    expected = Counter({TASK5_CLOSED: 180, TASK6_PENDING: 69, TASK7_PENDING: 108})
    if counts != expected:
        fail(f"Task 5 frozen partition drift: expected={expected} found={counts}")
    families = Counter(
        (classify_frozen_task5_row(row), row.old_path.rsplit("::", 1)[-1])
        for row in rows
        if row.source_path.startswith("crates/druid/")
    )
    expected_druid = Counter(
        {
            (TASK5_CLOSED, "PhysicalConnectionInfo"): 8,
            (TASK5_CLOSED, "PreparedStatementKey"): 40,
            (TASK5_CLOSED, "PreparedStatementMethodType"): 13,
            (TASK6_PENDING, "PhysicalConnectionInfo"): 2,
            (TASK6_PENDING, "PreparedStatementKey"): 2,
            (TASK6_PENDING, "PreparedStatementMethodType"): 2,
            (TASK6_PENDING, "RdbcResourceCapabilities"): 2,
            (TASK6_PENDING, "RdbcResourceContext"): 2,
            (TASK6_PENDING, "RdbcResourceFactory"): 2,
            (TASK6_PENDING, "RdbcResourceId"): 2,
            (TASK6_PENDING, "RdbcResourceKind"): 2,
            (TASK6_PENDING, "RdbcResourceOwner"): 2,
            (TASK6_PENDING, "RdbcResourceState"): 2,
        }
    )
    if families != expected_druid:
        fail(f"Task 5 Druid 61/20 family partition drift: {families}")


def verify_task4_evidence_handoff(root: Path) -> None:
    """Accept the one planned Task5 rename while keeping Task4 manifests frozen."""

    coverage_path = (
        root / "docs/verification/rdbc-first/rdbc-adapter-coverage-regions.json"
    )
    runtime_path = root / "docs/verification/rdbc-first/rdbc-adapter-runtime-traces.tsv"
    if (
        hashlib.sha256(coverage_path.read_bytes()).hexdigest()
        != TASK4_COVERAGE_MANIFEST_SHA256
    ):
        fail("Task 4 frozen compiler-coverage manifest changed during Task 5")
    if (
        hashlib.sha256(runtime_path.read_bytes()).hexdigest()
        != TASK4_RUNTIME_MANIFEST_SHA256
    ):
        fail("Task 4 frozen runtime-trace manifest changed during Task 5")
    payload = json.loads(coverage_path.read_text(encoding="utf-8"))
    mismatches: set[tuple[str, str, str]] = set()
    for family in payload["families"]:
        for requirement in family["requirements"]:
            source_path = requirement["source_path"]
            source = root / source_path
            current = hashlib.sha256(source.read_bytes()).hexdigest()
            expected = requirement["source_sha256"]
            if current != expected:
                mismatches.add((source_path, expected, current))
    expected_mismatch = {
        (
            "crates/druid-wrapper/tests/rbdc_connection_adapter_test.rs",
            "f5e8df13bed46fe23a274dea188cb02d0ff8827e3cb5ba8852e759a02b0edb1e",
            TASK5_RBDC_EVIDENCE_SOURCE_SHA256,
        )
    }
    if mismatches != expected_mismatch:
        fail(f"Task 4 evidence handoff has unexpected stale sources: {mismatches}")
    rbdc_path = next(iter(expected_mismatch))[0]
    rbdc_source = (root / rbdc_path).read_text(encoding="utf-8")
    if any(
        old in rbdc_source
        for old in ("RdbcResourceCapabilities", "RdbcResourceFactory")
    ) or not all(
        new in rbdc_source for new in ("ResourceOperations", "ResourceHandleFactory")
    ):
        fail(
            "Task 4 RBDC evidence source drift is not the approved Task 5 resource rename"
        )
    try:
        result = subprocess.run(
            [
                "cargo",
                "test",
                "--offline",
                "--locked",
                "-p",
                "druid-wrapper",
                "--test",
                "rbdc_connection_adapter_test",
                "rbdc_standard_factory_emits_typed_runtime_trace",
                "--",
                "--exact",
            ],
            cwd=root,
            check=False,
            capture_output=True,
            timeout=600,
        )
    except subprocess.TimeoutExpired as error:
        fail(f"Task 4 RBDC exact runtime handoff timed out: {error}")
    if result.returncode != 0:
        detail = (result.stdout + result.stderr).decode("utf-8", errors="replace")[
            -4000:
        ]
        fail(f"Task 4 RBDC exact runtime handoff failed: {detail}")


def _symbol_identity(symbol: str) -> str:
    return re.sub(r"@(?:qualified-)?token=.*$", "", symbol)


def _semantic_consumer_identity(
    row: ConsumerRow,
) -> tuple[str, str, str, str, str, str]:
    return (
        row.source_path,
        row.occurrence_kind,
        _symbol_identity(row.symbol),
        row.old_path,
        row.target_path,
        row.target_task,
    )


def _identity_counter_sha256(
    identities: Counter[tuple[str, str, str, str, str, str]],
) -> str:
    """Hash a multiset of stable consumer identities without source coordinates."""

    lines: list[str] = []
    for identity, count in sorted(identities.items()):
        lines.extend("\t".join(identity) for _ in range(count))
    payload = ("\n".join(lines) + ("\n" if lines else "")).encode()
    return hashlib.sha256(payload).hexdigest()


def require_identity_counter_hash(
    identities: Counter[tuple[str, str, str, str, str, str]],
    expected_sha256: str,
    label: str,
) -> None:
    """Reject one omitted, duplicated, or fabricated stable identity."""

    actual = _identity_counter_sha256(identities)
    if actual != expected_sha256:
        fail(f"{label} identity hash drift: expected={expected_sha256} found={actual}")


def _require_consumer_source_proof(
    root: Path,
    checked: tuple[ConsumerRow, ...],
    baseline: tuple[ConsumerRow, ...],
) -> tuple[int, int]:
    live = derive_live_manifests(root, P03_MANIFEST, phase="task5").consumers
    live_task5 = tuple(row for row in live if row.target_task == TASK5)
    checked_closed = tuple(row for row in checked if row.state == TASK5_CLOSED)
    baseline_identities = Counter(_semantic_consumer_identity(row) for row in baseline)
    checked_identities = Counter(_semantic_consumer_identity(row) for row in checked)
    live_identities = Counter(_semantic_consumer_identity(row) for row in live_task5)
    missing = baseline_identities - checked_identities
    if missing:
        fail("Task 5 LIVE/CLOSED rows omit the frozen dynamic denominator")
    additions = checked_identities - baseline_identities
    live_additions = live_identities - baseline_identities
    require_identity_counter_hash(
        additions,
        TASK6_ADDITION_NORMALIZED_SHA256,
        "Task 6 checked compatibility additions",
    )
    require_identity_counter_hash(
        live_additions,
        TASK6_ADDITION_NORMALIZED_SHA256,
        "Task 6 live compatibility additions",
    )
    frozen_task6 = Counter(
        _semantic_consumer_identity(row)
        for row in baseline
        if classify_frozen_task5_row(row) == TASK6_PENDING
    )
    shimmed_frozen = frozen_task6 - live_identities
    require_identity_counter_hash(
        shimmed_frozen,
        TASK6_SHIMMED_FROZEN_NORMALIZED_SHA256,
        "Task 6 local shim transitions",
    )
    for identity in shimmed_frozen:
        if not (root / identity[0]).is_file():
            fail(f"Task 6 local shim source is missing: {identity[0]}")
    live_identity_set = set(live_identities)
    for row in checked_closed:
        if _semantic_consumer_identity(row) in live_identity_set:
            fail("Task 5 CLOSED consumer still exists in fresh production source")
    baseline_by_identity = {
        _semantic_consumer_identity(row): classify_frozen_task5_row(row)
        for row in baseline
    }
    for row in checked:
        identity = _semantic_consumer_identity(row)
        expected_state = baseline_by_identity.get(identity, TASK6_PENDING)
        if row.state != expected_state:
            fail(
                f"Task 5 consumer has wrong disposition: {identity} "
                f"expected={expected_state} found={row.state}"
            )
    for row in live_task5:
        identity = _semantic_consumer_identity(row)
        expected_state = baseline_by_identity.get(identity, TASK6_PENDING)
        if expected_state in {TASK5_CLOSED, TASK7_PENDING}:
            fail(
                f"{expected_state} old identity still exists in production source: {identity}"
            )
    return (
        sum(row.state == TASK5_CLOSED for row in checked),
        sum(row.state == TASK6_PENDING for row in checked),
        sum(row.state == TASK7_PENDING for row in checked),
    )


def verify_repository(root: Path) -> VerificationSummary:
    """Verify the complete current Task 5 source, manifest, and ownership contract."""

    require_fixed_files(root)
    for relative, type_name in CONCRETE_OBJECTS.items():
        require_concrete_object((root / relative).read_text(), type_name, relative)
    holder_path = "crates/druid/src/core/druid_connection_holder.rs"
    require_holder_contract((root / holder_path).read_text(), holder_path)
    for relative, (trait_path, type_name, methods) in STANDARD_IMPLS.items():
        require_direct_standard_impl(
            (root / relative).read_text(), trait_path, type_name, methods, relative
        )
    owner_path = "crates/druid/src/core/druid_pooled_connection.rs"
    require_owner_close_contract((root / owner_path).read_text(), owner_path)

    baseline_payload = _git_show(root, TASK5_BASE, DEFAULT_CONSUMERS)
    if hashlib.sha256(baseline_payload).hexdigest() != TASK5_BASE_CONSUMER_SHA256:
        fail("Task 5 pinned consumer source hash drift")
    baseline_rows = tuple(
        row
        for row in parse_consumer_payload(baseline_payload)
        if row.target_task == TASK5
    )
    require_frozen_partition(baseline_rows)
    checked_payload = (root / DEFAULT_CONSUMERS).read_bytes()
    checked_rows = verify_task5_consumer_states(
        checked_payload,
        expected_rows=None,
    )
    closed_rows, task6_rows, task7_rows = _require_consumer_source_proof(
        root, checked_rows, baseline_rows
    )
    return VerificationSummary(
        len(checked_rows),
        closed_rows,
        task6_rows,
        task7_rows,
        len(STANDARD_IMPLS),
        len(CONCRETE_OBJECTS),
    )


def emit_task5_consumers(root: Path) -> None:
    """Refresh exact source coordinates and advance only Task 5 LIVE/CLOSED states."""

    destination = root / DEFAULT_CONSUMERS
    current = tuple(parse_consumer_payload(destination.read_bytes()))
    fresh = derive_live_manifests(root, P03_MANIFEST, phase="task5").consumers
    baseline = tuple(
        row
        for row in parse_consumer_payload(
            _git_show(root, TASK5_BASE, DEFAULT_CONSUMERS)
        )
        if row.target_task == TASK5
    )
    baseline_identities = Counter(_semantic_consumer_identity(row) for row in baseline)
    fresh_task5 = tuple(row for row in fresh if row.target_task == TASK5)
    fresh_identities = Counter(_semantic_consumer_identity(row) for row in fresh_task5)
    require_identity_counter_hash(
        fresh_identities - baseline_identities,
        TASK6_ADDITION_NORMALIZED_SHA256,
        "Task 6 live compatibility additions",
    )
    frozen_task6 = Counter(
        _semantic_consumer_identity(row)
        for row in baseline
        if classify_frozen_task5_row(row) == TASK6_PENDING
    )
    require_identity_counter_hash(
        frozen_task6 - fresh_identities,
        TASK6_SHIMMED_FROZEN_NORMALIZED_SHA256,
        "Task 6 local shim transitions",
    )

    prior_states: dict[tuple[str, str, str, str, str, str], deque[str]] = defaultdict(
        deque
    )
    for row in current:
        prior_states[_semantic_consumer_identity(row)].append(row.state)

    emitted: list[ConsumerRow] = []
    for row in fresh:
        if row.target_task == TASK5:
            continue
        identity = _semantic_consumer_identity(row)
        state = "CURRENT_REFERENCE_TARGET_PLANNED"
        if "STANDARD_REFERENCE_LIVE" in prior_states.get(identity, ()):
            state = "STANDARD_REFERENCE_LIVE"
        emitted.append(
            ConsumerRow(
                row.source_path,
                row.occurrence_kind,
                row.symbol,
                row.old_path,
                row.target_path,
                row.target_task,
                state,
            )
        )

    emitted.extend(row for row in current if row.state == "STANDARD_REFERENCE_CLOSED")
    fresh_buckets: dict[tuple[str, str, str, str, str, str], deque[ConsumerRow]] = (
        defaultdict(deque)
    )
    for row in fresh:
        if row.target_task == TASK5:
            fresh_buckets[_semantic_consumer_identity(row)].append(row)
    for baseline_row in baseline:
        identity = _semantic_consumer_identity(baseline_row)
        state = classify_frozen_task5_row(baseline_row)
        source_row = (
            fresh_buckets[identity].popleft()
            if state == TASK6_PENDING and fresh_buckets[identity]
            else baseline_row
        )
        emitted.append(
            ConsumerRow(
                source_row.source_path,
                source_row.occurrence_kind,
                source_row.symbol,
                source_row.old_path,
                source_row.target_path,
                source_row.target_task,
                state,
            )
        )
    for identity, additions in fresh_buckets.items():
        while additions:
            row = additions.popleft()
            if not (
                row.source_path.startswith("crates/druid/src/core/compat/")
                or row.source_path in DRUID_TASK6_COMPAT_PATHS
            ):
                fail(f"unapproved Task 5 consumer addition: {identity}")
            emitted.append(
                ConsumerRow(
                    row.source_path,
                    row.occurrence_kind,
                    row.symbol,
                    row.old_path,
                    row.target_path,
                    row.target_task,
                    TASK6_PENDING,
                )
            )
    destination.write_bytes(render_consumer_rows(tuple(sorted(emitted))))


def emit_task5_adapters(root: Path) -> None:
    """Refresh adapter rows while retaining Task 4 LIVE evidence exactly."""

    destination = root / DEFAULT_ADAPTERS
    current = tuple(parse_adapter_payload(destination.read_bytes()))
    live = derive_live_manifests(root, P03_MANIFEST, phase="task5").adapters
    live_keys = {
        (
            row.source_path,
            row.current_trait,
            row.implementation_type,
            row.target_trait,
            row.target_task,
        )
        for row in current
        if row.state == "STANDARD_IMPL_LIVE"
    }
    emitted = tuple(
        sorted(
            AdapterRow(
                row.source_path,
                row.current_trait,
                row.implementation_type,
                row.owner,
                row.target_trait,
                row.target_task,
                row.standard_implementation_type,
                row.standard_source_path,
                row.standard_core_evidence,
                row.behavior_evidence,
                (
                    "STANDARD_IMPL_LIVE"
                    if (
                        row.source_path,
                        row.current_trait,
                        row.implementation_type,
                        row.target_trait,
                        row.target_task,
                    )
                    in live_keys
                    else "CURRENT_IMPL_TARGET_PLANNED"
                ),
            )
            for row in live
        )
    )
    destination.write_bytes(render_adapter_rows(emitted))


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emit-live", action="store_true")
    return parser.parse_args()


def main() -> int:
    arguments = parse_arguments()
    try:
        if arguments.emit_live:
            emit_task5_consumers(repository_root())
            emit_task5_adapters(repository_root())
        summary = verify_repository(repository_root())
    except (OSError, VerificationError, DenominatorVerificationError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"TASK5_CONSUMER_ROWS={summary.consumer_rows}")
    print(f"TASK5_CONSUMER_CLOSED={summary.closed_rows}")
    print(f"TASK6_COMPATIBILITY_PENDING={summary.task6_pending_rows}")
    print(f"TASK7_RESOURCE_OPERATIONS_PENDING={summary.task7_pending_rows}")
    print(f"DRUID_STANDARD_IMPLS={summary.standard_impls}")
    print(f"DRUID_INTERNAL_OBJECTS={summary.concrete_objects}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
