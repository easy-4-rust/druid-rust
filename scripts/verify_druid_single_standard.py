#!/usr/bin/env python3
"""Verify P04 Task 5 Druid logical proxies and Druid-only runtime cutover."""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict, deque
import hashlib
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
from verify_wrapper_standard_impls import (
    CoverageFamilySpec,
    DEFAULT_ADAPTERS as TASK4_ADAPTER_MANIFEST,
    DEFAULT_RUNTIME_EVIDENCE as TASK4_RUNTIME_EVIDENCE_MANIFEST,
    VerificationError as WrapperVerificationError,
    evidence_specs,
    execute_evidence,
    function_line_ranges,
    load_coverage_manifest,
    task_adapter_rows,
    verify_coverage_manifest_bindings,
)


TASK5_BASE = "d1ccc8e3005a3a71638766ee701af99594fc43ed"
TASK5 = "P04_TASK_5_DRUID_INTERNALIZATION"
TASK5_CLOSED = "DRUID_INTERNAL_REFERENCE_CLOSED"
TASK6_PENDING = "TASK6_COMPATIBILITY_PENDING"
TASK6_CLOSED = "TASK6_COMPATIBILITY_CLOSED"
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
    "37e8bd1f82fb220b43283af173ff879a4287a499b25d59fd867404ba67599ad5"
)
TASK4_RUNTIME_MANIFEST_SHA256 = (
    "ee792f09a7e79a797978a16e40ebab4441489d4455c4a7a5762b1fe9dfc553c9"
)
TASK5_RBDC_EVIDENCE_SOURCE_SHA256 = (
    "79b16d6a8965ec21cfebf68a3e40b1013b853f7db53a1fb6f320b68c6808e5c0"
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
    "crates/druid/src/core/druid_execution_filter_policy.rs",
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
    "crates/druid/src/core/standard_resource_admission.rs",
    "crates/druid/tests/single_standard_logical_contract_test.rs",
    "crates/druid/tests/raw_logical_ownership_test.rs",
    "crates/druid/tests/standard_metadata_ownership_test.rs",
    "crates/druid/tests/standard_filter_stat_semantics_test.rs",
    "crates/druid-wrapper/tests/druid_standard_pool_admission_test.rs",
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
        {
            "execute_query",
            "execute_update",
            "execute",
            "add_batch",
            "clear_batch",
            "execute_batch",
            "close",
            "is_closed",
        },
    ),
    "crates/druid/src/core/druid_pooled_prepared_statement.rs": (
        "druid_rdbc::statement::PreparedStatement",
        "DruidPooledPreparedStatement",
        {
            "set_parameter",
            "execute_prepared_query",
            "execute_prepared_update",
            "execute_prepared",
        },
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

STANDARD_INHERITED_IMPLS = (
    (
        "crates/druid/src/core/druid_pooled_prepared_statement.rs",
        "druid_rdbc::statement::Statement",
        "DruidPooledPreparedStatement",
        {"add_batch", "clear_batch", "execute_batch"},
    ),
    (
        "crates/druid/src/core/druid_pooled_callable_statement.rs",
        "druid_rdbc::statement::Statement",
        "DruidPooledCallableStatement",
        {"add_batch", "clear_batch", "execute_batch"},
    ),
    (
        "crates/druid/src/core/druid_pooled_callable_statement.rs",
        "druid_rdbc::statement::PreparedStatement",
        "DruidPooledCallableStatement",
        {
            "set_parameter",
            "clear_parameters",
            "execute_prepared_query",
            "execute_prepared_update",
            "execute_prepared",
        },
    ),
)


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


@dataclass(frozen=True)
class ExactCargoTestSpec:
    """One exact Task 5 behavioral test bound to its real Rust definition."""

    source_path: Path
    test_target: str
    test_id: str


TASK5_REQUIRED_EXACT_TESTS = (
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_prepare_rewrite_cache_parameters_batch_events_and_timeout_match_raw",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_prepared_setter_failure_and_clear_keep_filter_snapshot_truthful",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_wall_prepare_and_metadata_rejection_prevent_raw_calls",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_result_set_open_initializes_wall_tenant_hidden_mapping",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_result_set_open_rejection_closes_unregistered_raw_once",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_result_set_value_metadata_was_null_filters_modify_and_reject_once",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_prepared_generic_execute_wraps_registered_result_with_typed_params",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_callable_generic_execute_uses_same_typed_policy_and_wraps_result",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_prepared_generic_fatal_error_runs_after_once_and_discards",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_prepared_multi_result_open_failure_closes_every_raw_once",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_multi_result_cleanup_failure_preserves_primary_and_retries",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_callable_multi_result_open_failure_closes_every_raw_once",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_statement_multi_result_open_failure_closes_every_raw_once",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_statement_batch_rewrites_orders_clears_and_filters_once",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "canonical_statement_batch_denial_and_partial_fatal_error_are_classified_once",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
        "test:standard_filter_stat_semantics_test",
        "fatal_standard_create_prepare_call_and_metadata_errors_force_discard",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::late_registration_after_close_is_rejected_and_closed",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::explicit_close_unregisters_strong_entries_during_long_lease",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::concurrent_close_all_waits_for_one_exact_drain",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::aborted_close_all_caller_does_not_cancel_registry_owned_drain",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::dropping_registry_cancels_blocked_coordinator_task",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::pending_admission_cancellation_unblocks_close",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::multiple_pending_admissions_all_finish_before_close_returns",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::close_all_waits_for_rejected_late_resource_close_completion",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::cancelled_rejected_registration_keeps_registry_cleanup_ownership",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/src/core/standard_resource_registry.rs"),
        "lib",
        "core::standard_resource_registry::tests::late_registration_close_failure_stays_old_generation_and_blocks_reopen",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_metadata_ownership_test.rs"),
        "test:standard_metadata_ownership_test",
        "late_metadata_result_registration_is_closed_before_next_lease",
    ),
    ExactCargoTestSpec(
        Path("crates/druid/tests/standard_metadata_ownership_test.rs"),
        "test:standard_metadata_ownership_test",
        "late_result_close_failure_discards_old_generation_without_reopen",
    ),
)


def fail(message: str) -> NoReturn:
    raise VerificationError(message)


def require_admission_composition_source(source: str) -> None:
    """Require every local Task5 admission identity without exposing remote test seams."""

    required = {
        "sqlx_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool",
        "toasty_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool",
        "rbdc_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool",
        "duckdb_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool",
        "DruidPool::new_standard",
    }
    missing = sorted(token for token in required if token not in source)
    if missing:
        fail(f"Task 5 standard admission composition is incomplete: {missing}")


def require_shared_execution_policy(
    policy_source: str, standard_source: str, physical_source: str
) -> None:
    """Require one named Filter policy used by both canonical and transitional paths."""

    policy_tokens = (
        "struct DruidExecutionFilterPolicy",
        "before_execute(context)",
        "raw(effective_sql.clone()).await",
        "after_execute(context",
    )
    if any(token not in policy_source for token in policy_tokens):
        fail("Task 5 shared Filter execution policy is incomplete")
    call = "DruidExecutionFilterPolicy::execute"
    if call not in standard_source or physical_source.count(call) < 4:
        fail(
            "standard and Physical execution paths do not share Druid execution policy"
        )


def run_cargo_gate(
    root: Path, label: str, command: list[str]
) -> subprocess.CompletedProcess[bytes]:
    """Run one exact offline Cargo evidence gate and preserve actionable failure output."""

    try:
        result = subprocess.run(
            command,
            cwd=root,
            check=False,
            capture_output=True,
            timeout=600,
        )
    except subprocess.TimeoutExpired as error:
        fail(f"{label} timed out: {error}")
    if result.returncode != 0:
        detail = (result.stdout + result.stderr).decode("utf-8", errors="replace")[
            -4000:
        ]
        fail(f"{label} failed: {detail}")
    return result


def require_exact_test_source(root: Path, spec: ExactCargoTestSpec) -> None:
    """Reject comments, duplicate definitions, and missing exact-test identities."""

    path = root / spec.source_path
    if not path.is_file():
        fail(f"Task 5 exact test source is missing: {spec.source_path}")
    function_name = spec.test_id.rsplit("::", 1)[-1]
    ranges = function_line_ranges(path.read_text(encoding="utf-8"), function_name)
    if len(ranges) != 1:
        fail(
            "Task 5 exact test function is missing or ambiguous: "
            f"{spec.source_path}:{function_name}:{ranges}"
        )


def run_exact_cargo_test(root: Path, label: str, spec: ExactCargoTestSpec) -> None:
    """Run one exact test and reject Cargo's successful zero-test outcome."""

    require_exact_test_source(root, spec)
    command = [
        "cargo",
        "test",
        "--offline",
        "--locked",
        "-p",
        "druid",
    ]
    if spec.test_target == "lib":
        command.append("--lib")
    elif spec.test_target.startswith("test:"):
        command.extend(["--test", spec.test_target.removeprefix("test:")])
    else:
        fail(f"Task 5 exact test target is invalid: {spec.test_target}")
    command.extend([spec.test_id, "--", "--exact", "--nocapture"])
    result = run_cargo_gate(root, label, command)
    output = result.stdout + result.stderr
    summary = b"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;"
    if output.count(b"running 1 test") != 1 or output.count(summary) != 1:
        fail(f"{label} did not execute exactly one passing test")


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


def require_method_delegation(
    source: str,
    trait_path: str,
    type_name: str,
    required_markers: dict[str, str],
    label: str,
) -> None:
    """Bind load-bearing canonical methods to named shared-policy cores."""

    blocks = _impl_blocks(source, trait_path.rsplit("::", 1)[-1], type_name)
    if len(blocks) != 1:
        fail(f"{label} shared policy delegation impl is missing or ambiguous")
    tokens, _, opening, closing = blocks[0]
    methods = _method_bodies(tokens, opening, closing)
    for method, marker in required_markers.items():
        body = methods.get(method)
        if body is None or marker not in body:
            fail(
                f"{label} shared policy delegation is missing: "
                f"{type_name}::{method}->{marker}"
            )


def require_pending_admission_policy(
    registry_source: str,
    admission_source: str,
    producer_sources: dict[str, str],
) -> None:
    """Require pre-raw permits at every standard live-resource producer."""

    registry = "".join(token.text for token in _tokens(registry_source))
    admission = "".join(token.text for token in _tokens(admission_source))
    registry_markers = (
        "fnbegin_admission",
        "pending",
        "close_error",
        "fnregister_batch",
        "fnclose_registered_after_admission",
        "fnspawn_coordinator",
        "fnrun_coordinator",
        "coordinator_running",
        "shutting_down",
        "shutdown.notify_waiters()",
        "StandardResourceCleanupState::Retry",
        "notified.as_mut().enable()",
        "fnfinish_admission",
    )
    admission_markers = (
        "structStandardResourceAdmission",
        "implDropforStandardResourceAdmission",
        "finish_admission",
    )
    if any(marker not in registry for marker in registry_markers) or any(
        marker not in admission for marker in admission_markers
    ):
        fail("Task 5 pending admission registry/RAII policy is incomplete")

    expected = {
        "druid_pooled_connection.rs": ("begin_admission", 8),
        "druid_pooled_statement.rs": ("begin_standard_admission", 5),
        "druid_pooled_prepared_statement.rs": ("begin_standard_admission", 3),
        "druid_pooled_callable_statement.rs": ("begin_standard_admission", 3),
        "database_meta_data_proxy_impl.rs": ("begin_admission", 2),
    }
    if set(producer_sources) != set(expected) | {"druid_pooled_result_set.rs"}:
        fail("Task 5 pending admission producer denominator is incomplete")
    for label, (symbol, count) in expected.items():
        observed = sum(
            token.text == symbol for token in _tokens(producer_sources[label])
        )
        if observed != count:
            fail(
                f"Task 5 pending admission producer drift: "
                f"{label}:{symbol}:{observed}!={count}"
            )
    result_set = "".join(
        token.text for token in _tokens(producer_sources["druid_pooled_result_set.rs"])
    )
    staged_markers = (
        "fnnew_standard_results",
        ".register_batch(admission,handles)",
        ".close_registered_after_admission(&registrations,admission)",
    )
    if any(marker not in result_set for marker in staged_markers):
        fail("Task 5 pending admission staged ResultSet ownership is incomplete")

    ordered_methods = (
        (
            "druid_pooled_connection.rs",
            "Connection",
            "DruidPooledConnection",
            "create_statement",
            (
                "begin_admission",
                "create_statement(options).await",
                ".register(&admission",
            ),
        ),
        (
            "druid_pooled_connection.rs",
            "Connection",
            "DruidPooledConnection",
            "prepare_statement_with_generated_keys",
            (
                "begin_admission",
                "prepare_statement_with_generated_keys(&effective_sql,options,generated_keys).await",
                "DruidPooledPreparedStatement::new_standard",
            ),
        ),
        (
            "druid_pooled_connection.rs",
            "Connection",
            "DruidPooledConnection",
            "prepare_call",
            (
                "begin_admission",
                "prepare_call(&effective_sql,options).await",
                "DruidPooledCallableStatement::new_standard",
            ),
        ),
        (
            "druid_pooled_connection.rs",
            "Connection",
            "DruidPooledConnection",
            "create_blob",
            ("begin_admission", "create_blob().await", "DruidBlob::register"),
        ),
        (
            "druid_pooled_connection.rs",
            "Connection",
            "DruidPooledConnection",
            "create_clob",
            ("begin_admission", "create_clob().await", "DruidClob::register"),
        ),
        (
            "druid_pooled_connection.rs",
            "Connection",
            "DruidPooledConnection",
            "create_n_clob",
            ("begin_admission", "create_n_clob().await", "DruidNClob::register"),
        ),
        (
            "druid_pooled_connection.rs",
            "Connection",
            "DruidPooledConnection",
            "create_sql_xml",
            ("begin_admission", "create_sql_xml().await", "DruidSqlXml::register"),
        ),
        (
            "druid_pooled_connection.rs",
            "Connection",
            "DruidPooledConnection",
            "create_array_of",
            ("begin_admission", "create_array_of(type_name,elements).await", "DruidArray::register"),
        ),
        (
            "druid_pooled_statement.rs",
            "Statement",
            "DruidPooledStatement",
            "execute_query",
            (
                "begin_standard_admission",
                "execute_query(&effective_sql).await",
                "DruidPooledResultSet::new_standard",
            ),
        ),
        (
            "druid_pooled_statement.rs",
            "Statement",
            "DruidPooledStatement",
            "execute",
            (
                "begin_standard_admission",
                "execute(&effective_sql,generated_keys).await",
                "DruidPooledResultSet::new_standard_results",
            ),
        ),
        (
            "druid_pooled_statement.rs",
            "Statement",
            "DruidPooledStatement",
            "generated_keys",
            (
                "begin_standard_admission",
                ".generated_keys().await",
                "DruidPooledResultSet::new_standard",
            ),
        ),
        (
            "druid_pooled_statement.rs",
            "Statement",
            "DruidPooledStatement",
            "get_result_set",
            (
                "begin_standard_admission",
                ".get_result_set().await",
                "DruidPooledResultSet::new_standard",
            ),
        ),
        (
            "druid_pooled_prepared_statement.rs",
            "PreparedStatement",
            "DruidPooledPreparedStatement",
            "execute_prepared_query",
            (
                "begin_standard_admission",
                "execute_prepared_query().await",
                "DruidPooledResultSet::new_standard",
            ),
        ),
        (
            "druid_pooled_prepared_statement.rs",
            "Statement",
            "DruidPooledPreparedStatement",
            "generated_keys",
            (
                "begin_standard_admission",
                ".generated_keys().await",
                "DruidPooledResultSet::new_standard",
            ),
        ),
        (
            "druid_pooled_callable_statement.rs",
            "PreparedStatement",
            "DruidPooledCallableStatement",
            "execute_prepared_query",
            (
                "begin_standard_admission",
                "execute_prepared_query().await",
                "DruidPooledResultSet::new_standard",
            ),
        ),
        (
            "druid_pooled_callable_statement.rs",
            "Statement",
            "DruidPooledCallableStatement",
            "generated_keys",
            (
                "begin_standard_admission",
                ".generated_keys().await",
                "DruidPooledResultSet::new_standard",
            ),
        ),
        (
            "database_meta_data_proxy_impl.rs",
            "DatabaseMetaData",
            "DatabaseMetaDataProxyImpl",
            "metadata_result_set",
            (
                "begin_admission",
                ".metadata_result_set(request).await",
                "DruidPooledResultSet::new_standard_metadata",
            ),
        ),
        (
            "database_meta_data_proxy_impl.rs",
            "DatabaseMetaData",
            "DatabaseMetaDataProxyImpl",
            "tables",
            (
                "begin_admission",
                ".tables(query).await",
                "DruidPooledResultSet::new_standard_metadata",
            ),
        ),
    )
    for label, trait_leaf, type_name, method, markers in ordered_methods:
        blocks = _impl_blocks(producer_sources[label], trait_leaf, type_name)
        if len(blocks) != 1:
            fail(f"Task 5 pre-raw admission impl is missing or ambiguous: {label}")
        tokens, _, opening, closing = blocks[0]
        body = _method_bodies(tokens, opening, closing).get(method)
        if body is None:
            fail(f"Task 5 pre-raw admission method is missing: {label}:{method}")
        _require_ordered_markers(body, markers, f"{label}:{type_name}::{method}")

    # 普通重载只有这一条转发路径；禁止额外原生调用、错误请求和不可达伪证据。
    connection_blocks = _impl_blocks(
        producer_sources["druid_pooled_connection.rs"], "Connection", "DruidPooledConnection"
    )
    tokens, _, opening, closing = connection_blocks[0]
    delegate = _method_bodies(tokens, opening, closing).get("prepare_statement")
    expected_delegate = (
        "self.prepare_statement_with_generated_keys("
        "sql,options,druid_rdbc::statement::GeneratedKeys::None,).await"
    )
    if delegate != expected_delegate:
        fail("Task 6 pre-raw admission prepared overload delegation is not exact")

    for label in (
        "druid_pooled_prepared_statement.rs",
        "druid_pooled_callable_statement.rs",
    ):
        helper = _named_function_bodies(
            producer_sources[label],
            "execute_prepared_results",
        )
        if len(helper) != 1:
            fail(
                "Task 6 canonical pre-raw admission prepared execute helper "
                f"is missing or ambiguous: {label}"
            )
        _require_ordered_markers(
            helper[0],
            (
                "begin_standard_admission",
                ".execute_prepared(",
                "new_standard_results",
            ),
            f"{label}:execute_prepared_results",
        )


def _named_function_bodies(source: str, name: str) -> list[str]:
    """Return normalized bodies for every real Rust function with an exact name."""

    tokens = _tokens(source)
    bodies: list[str] = []
    for index, token in enumerate(tokens):
        if token.text != "fn" or index + 1 >= len(tokens):
            continue
        if tokens[index + 1].text != name:
            continue
        body_open = index + 2
        while body_open < len(tokens) and tokens[body_open].text not in {"{", ";"}:
            body_open += 1
        if body_open >= len(tokens) or tokens[body_open].text != "{":
            continue
        _, body_close = _balanced_block(tokens, body_open)
        bodies.append(_normalized(tokens, body_open + 1, body_close))
    return bodies


def _require_ordered_markers(body: str, markers: tuple[str, ...], label: str) -> None:
    """Require permit, raw acquisition and registry ownership in exact lexical order."""

    cursor = 0
    for marker in markers:
        position = body.find(marker, cursor)
        if position < 0:
            fail(
                "Task 5 pre-raw admission ordering is incomplete: " f"{label}:{marker}"
            )
        cursor = position + len(marker)


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
    """Require the fully regenerated Task 6 runtime and compiler evidence manifests."""

    coverage_path = (
        root / "docs/verification/rdbc-first/rdbc-adapter-coverage-regions.json"
    )
    runtime_path = root / "docs/verification/rdbc-first/rdbc-adapter-runtime-traces.tsv"
    if (
        hashlib.sha256(coverage_path.read_bytes()).hexdigest()
        != TASK4_COVERAGE_MANIFEST_SHA256
    ):
        fail("Task 6 regenerated compiler-coverage manifest changed")
    if (
        hashlib.sha256(runtime_path.read_bytes()).hexdigest()
        != TASK4_RUNTIME_MANIFEST_SHA256
    ):
        fail("Task 6 regenerated runtime-trace manifest changed")
    try:
        coverage = load_coverage_manifest(root, coverage_path.relative_to(root))
        adapters = task_adapter_rows(
            root / TASK4_ADAPTER_MANIFEST,
            root / TASK4_RUNTIME_EVIDENCE_MANIFEST,
        )
        specs = evidence_specs(adapters)
        verify_coverage_manifest_bindings(specs, coverage)
    except WrapperVerificationError as error:
        fail(f"Task 6 regenerated Task 4 evidence is invalid: {error}")
    admission_source = (
        root / "crates/druid-wrapper/tests/druid_standard_pool_admission_test.rs"
    ).read_text(encoding="utf-8")
    require_admission_composition_source(admission_source)


def task5_task4_coverage_contract(root: Path) -> tuple[CoverageFamilySpec, ...]:
    """Load the final Task 6-regenerated exact compiler-coverage contract."""

    path = root / "docs/verification/rdbc-first/rdbc-adapter-coverage-regions.json"
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != TASK4_COVERAGE_MANIFEST_SHA256:
        fail("Task 6 regenerated compiler-coverage manifest changed")
    try:
        result = load_coverage_manifest(root, path.relative_to(root))
    except WrapperVerificationError as error:
        fail(f"Task 6 regenerated coverage contract is invalid: {error}")
    return result


def execute_task5_rbdc_compiler_coverage(root: Path) -> None:
    """Run fresh RBDC coverage-v2 evidence for all fifteen exact obligations."""

    try:
        adapters = task_adapter_rows(
            root / TASK4_ADAPTER_MANIFEST,
            root / TASK4_RUNTIME_EVIDENCE_MANIFEST,
        )
        specs = evidence_specs(adapters)
        coverage = task5_task4_coverage_contract(root)
        verify_coverage_manifest_bindings(specs, coverage)
        matching = [spec for spec in specs if spec.family == "RBDC"]
        rbdc_families = [family for family in coverage if family.family == "RBDC"]
        if len(matching) != 1 or len(rbdc_families) != 1:
            fail("Task 4 RBDC runtime/coverage binding is not unique")
        if len(rbdc_families[0].requirements) != 15:
            fail("Task 4 RBDC compiler coverage denominator is not fifteen")
        execute_evidence(root, matching[0], coverage)
    except WrapperVerificationError as error:
        fail(f"Task 4 current RBDC compiler coverage failed: {error}")


def verify_task5_runtime_contracts(root: Path) -> None:
    """Bind LIVE claims to the review's focused production behavior suites."""

    for label, test_name in (
        ("Task 5 pool/cache/cleanup runtime", "single_standard_logical_contract_test"),
        ("Task 5 Filter/Stat/error runtime", "standard_filter_stat_semantics_test"),
        ("Task 5 metadata cross-lease runtime", "standard_metadata_ownership_test"),
        ("Task 5 raw/logical owner runtime", "raw_logical_ownership_test"),
    ):
        run_cargo_gate(
            root,
            label,
            [
                "cargo",
                "test",
                "--offline",
                "--locked",
                "-p",
                "druid",
                "--test",
                test_name,
            ],
        )
    for spec in TASK5_REQUIRED_EXACT_TESTS:
        run_exact_cargo_test(
            root,
            f"Task 5 exact runtime {spec.test_id}",
            spec,
        )


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

    checked_rows = tuple(
        parse_consumer_payload((root / DEFAULT_CONSUMERS).read_bytes())
    )
    if any(row.state == TASK6_CLOSED for row in checked_rows):
        return verify_final_repository(root, checked_rows)

    require_fixed_files(root)
    for relative, type_name in CONCRETE_OBJECTS.items():
        require_concrete_object((root / relative).read_text(encoding="utf-8"), type_name, relative)
    holder_path = "crates/druid/src/core/druid_connection_holder.rs"
    require_holder_contract((root / holder_path).read_text(encoding="utf-8"), holder_path)
    for relative, (trait_path, type_name, methods) in STANDARD_IMPLS.items():
        require_direct_standard_impl(
            (root / relative).read_text(encoding="utf-8"), trait_path, type_name, methods, relative
        )
    for relative, trait_path, type_name, methods in STANDARD_INHERITED_IMPLS:
        require_direct_standard_impl(
            (root / relative).read_text(encoding="utf-8"), trait_path, type_name, methods, relative
        )
    require_method_delegation(
        (root / "crates/druid/src/core/druid_pooled_statement.rs").read_text(encoding="utf-8"),
        "druid_rdbc::statement::Statement",
        "DruidPooledStatement",
        {
            "add_batch": "statement_add_batch_sql",
            "execute_batch": "execute_statement_batch",
        },
        "crates/druid/src/core/druid_pooled_statement.rs",
    )
    for relative, type_name in (
        (
            "crates/druid/src/core/druid_pooled_prepared_statement.rs",
            "DruidPooledPreparedStatement",
        ),
        (
            "crates/druid/src/core/druid_pooled_callable_statement.rs",
            "DruidPooledCallableStatement",
        ),
    ):
        require_method_delegation(
            (root / relative).read_text(encoding="utf-8"),
            "druid_rdbc::statement::PreparedStatement",
            type_name,
            {"execute_prepared": "execute_standard_prepared_results"},
            relative,
        )
    require_pending_admission_policy(
        (root / "crates/druid/src/core/standard_resource_registry.rs").read_text(encoding="utf-8"),
        (root / "crates/druid/src/core/standard_resource_admission.rs").read_text(encoding="utf-8"),
        {
            path.name: path.read_text(encoding="utf-8")
            for path in (
                root / "crates/druid/src/core/druid_pooled_connection.rs",
                root / "crates/druid/src/core/druid_pooled_statement.rs",
                root / "crates/druid/src/core/druid_pooled_prepared_statement.rs",
                root / "crates/druid/src/core/druid_pooled_callable_statement.rs",
                root / "crates/druid/src/core/database_meta_data_proxy_impl.rs",
                root / "crates/druid/src/core/druid_pooled_result_set.rs",
            )
        },
    )
    owner_path = "crates/druid/src/core/druid_pooled_connection.rs"
    require_owner_close_contract((root / owner_path).read_text(encoding="utf-8"), owner_path)
    require_shared_execution_policy(
        (root / "crates/druid/src/core/druid_execution_filter_policy.rs").read_text(encoding="utf-8"),
        (root / "crates/druid/src/core/standard_logical_policy.rs").read_text(encoding="utf-8"),
        (root / owner_path).read_text(encoding="utf-8"),
    )
    verify_task4_evidence_handoff(root)
    verify_task5_runtime_contracts(root)

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


def verify_final_repository(
    root: Path, checked_rows: tuple[ConsumerRow, ...]
) -> VerificationSummary:
    """Verify the Task 6 canonical-only Druid source and retained Task 5 behavior."""

    required = (
        "crates/druid/src/core/connection_info.rs",
        "crates/druid/src/core/druid_connection_holder.rs",
        "crates/druid/src/core/druid_pooled_connection.rs",
        "crates/druid/src/core/druid_pooled_statement.rs",
        "crates/druid/src/core/druid_pooled_prepared_statement.rs",
        "crates/druid/src/core/druid_pooled_callable_statement.rs",
        "crates/druid/src/core/druid_pooled_result_set.rs",
        "crates/druid/src/core/database_meta_data_proxy_impl.rs",
        "crates/druid/src/core/standard_resource_registry.rs",
        "crates/druid/tests/single_standard_logical_contract_test.rs",
        "crates/druid/tests/raw_logical_ownership_test.rs",
        "crates/druid/tests/standard_metadata_ownership_test.rs",
        "crates/druid/tests/standard_filter_stat_semantics_test.rs",
    )
    missing = [path for path in required if not (root / path).is_file()]
    if missing:
        fail(f"required final Druid file is missing: {missing}")
    forbidden_paths = (
        root / "crates/druid/src/core/physical_connection_info.rs",
        root / "crates/druid/src/spi/mod.rs",
        root / "crates/druid/src/rdbc.rs",
    )
    if any(path.exists() for path in forbidden_paths):
        fail("retired Druid compatibility facade remains")
    for source_root in (
        root / "crates/druid/src",
        root / "crates/druid-wrapper/src",
        root / "crates/druid-rdbc/src",
    ):
        for path in source_root.rglob("*.rs"):
            source = path.read_text(encoding="utf-8")
            match = re.search(r"\bPhysical[A-Z][A-Za-z0-9_]*\b", source)
            if match:
                fail(f"retired identity remains in {path}: {match.group(0)}")
    for relative, (trait_path, type_name, methods) in STANDARD_IMPLS.items():
        require_direct_standard_impl(
            (root / relative).read_text(encoding="utf-8"), trait_path, type_name, methods, relative
        )
    for relative, trait_path, type_name, methods in STANDARD_INHERITED_IMPLS:
        require_direct_standard_impl(
            (root / relative).read_text(encoding="utf-8"), trait_path, type_name, methods, relative
        )
    require_pending_admission_policy(
        (root / "crates/druid/src/core/standard_resource_registry.rs").read_text(encoding="utf-8"),
        (root / "crates/druid/src/core/standard_resource_admission.rs").read_text(encoding="utf-8"),
        {
            path.name: path.read_text(encoding="utf-8")
            for path in (
                root / "crates/druid/src/core/druid_pooled_connection.rs",
                root / "crates/druid/src/core/druid_pooled_statement.rs",
                root / "crates/druid/src/core/druid_pooled_prepared_statement.rs",
                root / "crates/druid/src/core/druid_pooled_callable_statement.rs",
                root / "crates/druid/src/core/database_meta_data_proxy_impl.rs",
                root / "crates/druid/src/core/druid_pooled_result_set.rs",
            )
        },
    )
    task5 = tuple(row for row in checked_rows if row.target_task == TASK5)
    counts = Counter(row.state for row in task5)
    expected = Counter({TASK5_CLOSED: 180, TASK6_CLOSED: 75, TASK7_PENDING: 108})
    if counts != expected:
        fail(f"final Task 5/6/7 consumer partition drift: {counts} != {expected}")
    verify_task4_evidence_handoff(root)
    verify_task5_runtime_contracts(root)
    return VerificationSummary(
        len(task5),
        counts[TASK5_CLOSED],
        counts[TASK6_PENDING],
        counts[TASK7_PENDING],
        len(STANDARD_IMPLS),
        7,
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
    if summary.task6_pending_rows == 0:
        print("TASK6_COMPATIBILITY_CLOSED=75")
    print(f"TASK7_RESOURCE_OPERATIONS_PENDING={summary.task7_pending_rows}")
    print(f"DRUID_STANDARD_IMPLS={summary.standard_impls}")
    print(f"DRUID_INTERNAL_OBJECTS={summary.concrete_objects}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
