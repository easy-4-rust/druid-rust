#!/usr/bin/env python3
"""Render deterministic Task 6 method rows from BASE and the canonical target tree."""

from __future__ import annotations

import argparse
from dataclasses import replace
from pathlib import Path

import verify_task6_method_parity as verifier


TARGET_SPECS = (
    ("crates/druid-rdbc/src/connection/connection.rs", "Connection", "druid_rdbc::connection"),
    ("crates/druid-rdbc/src/statement/statement.rs", "Statement", "druid_rdbc::statement"),
    (
        "crates/druid-rdbc/src/statement/prepared_statement.rs",
        "PreparedStatement",
        "druid_rdbc::statement",
    ),
    (
        "crates/druid-rdbc/src/statement/callable_statement.rs",
        "CallableStatement",
        "druid_rdbc::statement",
    ),
    ("crates/druid-rdbc/src/result_set/result_set.rs", "ResultSet", "druid_rdbc::result_set"),
    (
        "crates/druid-rdbc/src/result_set/result_set_ext.rs",
        "ResultSetExt",
        "druid_rdbc::result_set",
    ),
    (
        "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        "DatabaseMetaDataInfoExt",
        "druid_rdbc::metadata",
    ),
    (
        "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        "DatabaseMetaDataSqlCapabilitiesExt",
        "druid_rdbc::metadata",
    ),
    (
        "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        "DatabaseMetaDataLimitsExt",
        "druid_rdbc::metadata",
    ),
    (
        "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        "DatabaseMetaDataQueryExt",
        "druid_rdbc::metadata",
    ),
    (
        "crates/druid-rdbc/src/metadata/result_set_meta_data.rs",
        "ResultSetMetaData",
        "druid_rdbc::metadata",
    ),
    ("crates/druid-rdbc/src/wrapper/wrapper.rs", "Wrapper", "druid_rdbc::wrapper"),
)

MANUAL = {
    ("PhysicalConnection", "create_physical_statement"): ("Connection", "create_statement"),
    ("PhysicalConnection", "execute"): ("Statement", "execute"),
    ("PhysicalConnection", "fetch_result_set"): ("Statement", "execute_query"),
    ("PhysicalConnection", "prepare_physical_statement"): ("Connection", "prepare_statement"),
    ("PhysicalConnection", "prepare_physical_call"): ("Connection", "prepare_call"),
    ("PhysicalConnection", "exec_prepared"): ("PreparedStatement", "execute_prepared_update"),
    ("PhysicalConnection", "exec_prepared_parameters"): (
        "PreparedStatement",
        "execute_prepared_update",
    ),
    ("PhysicalConnection", "execute_prepared"): ("PreparedStatement", "execute_prepared"),
    ("PhysicalConnection", "execute_prepared_parameters"): (
        "PreparedStatement",
        "execute_prepared",
    ),
    ("PhysicalConnection", "exec_prepared_parameter_batch"): (
        "Connection",
        "exec_prepared_parameter_batch",
    ),
    ("PhysicalConnection", "fetch_prepared"): ("PreparedStatement", "execute_prepared_query"),
    ("PhysicalConnection", "fetch_prepared_parameters"): (
        "PreparedStatement",
        "execute_prepared_query",
    ),
    ("PhysicalConnection", "fetch_prepared_result_set"): (
        "PreparedStatement",
        "execute_prepared_query",
    ),
    ("PhysicalConnection", "fetch_prepared_parameters_result_set"): (
        "PreparedStatement",
        "execute_prepared_query",
    ),
    ("PhysicalConnection", "close_prepared_statement"): ("Statement", "close"),
    ("PhysicalStatement", "as_any"): ("Wrapper", "unwrap"),
    ("PhysicalPreparedStatement", "as_any"): ("Wrapper", "unwrap"),
    ("PhysicalPreparedStatement", "as_callable"): ("Wrapper", "unwrap"),
    ("PhysicalPreparedStatement", "statement_options"): ("Statement", "options"),
    ("ConnectionExt", "create_statement"): ("Connection", "create_statement"),
    ("ConnectionExt", "prepare_statement"): ("Connection", "prepare_statement"),
    ("ConnectionExt", "prepare_call"): ("Connection", "prepare_call"),
    ("ConnectionExt", "get_meta_data"): ("Connection", "database_meta_data"),
    ("ConnectionExt", "get_database_product_name"): ("Connection", "database_product_name"),
    ("ConnectionExt", "get_database_product_version"): ("Connection", "database_product_version"),
    ("ConnectionExt", "get_driver_major_version"): ("Connection", "driver_major_version"),
    ("ConnectionExt", "get_driver_minor_version"): ("Connection", "driver_minor_version"),
    ("ConnectionExt", "get_holdability"): ("Connection", "holdability"),
    ("ConnectionExt", "get_client_info"): ("Connection", "client_info"),
    ("ConnectionExt", "get_warnings"): ("Connection", "warnings"),
    ("ConnectionExt", "get_network_timeout"): ("Connection", "network_timeout"),
    ("ConnectionExt", "get_type_map"): ("Connection", "type_map"),
}

PREFERRED = {
    "ConnectionExt": ("Connection",),
    "PhysicalConnection": ("Connection", "PreparedStatement", "Statement"),
    "PhysicalStatement": ("Statement",),
    "PhysicalPreparedStatement": ("PreparedStatement", "Statement"),
    "PhysicalCallableStatement": ("CallableStatement",),
    "PhysicalResultSet": ("ResultSet", "ResultSetExt"),
    "PhysicalDatabaseMetaData": (
        "DatabaseMetaDataInfoExt",
        "DatabaseMetaDataSqlCapabilitiesExt",
        "DatabaseMetaDataLimitsExt",
        "DatabaseMetaDataQueryExt",
    ),
    "PhysicalResultSetMetaData": ("ResultSetMetaData",),
}


def conversion(source_object: str) -> str:
    return {
        "ConnectionExt": "TYPED_OPTIONS+RESULT_FAIL_CLOSED",
        "PhysicalConnection": "OWNED_BOX+TYPED_OPTIONS+RESULT_FAIL_CLOSED",
        "PhysicalStatement": "OWNED_MUTABLE_STATEMENT+TYPED_STATE",
        "PhysicalPreparedStatement": "RICH_INPUT_DESCRIPTOR+OWNED_STATEMENT",
        "PhysicalCallableStatement": "SELECTOR_DESCRIPTOR+MUTABLE_REGISTRATION",
        "PhysicalResultSet": "USIZE_TO_COLUMN_INDEX+OWNED_ASYNC_CURSOR",
        "PhysicalDatabaseMetaData": "ARC_TO_BOX+TYPED_METADATA_REQUEST",
        "PhysicalResultSetMetaData": "USIZE_TO_COLUMN_INDEX+IMMUTABLE_SNAPSHOT",
    }[source_object]


def derive(repository_root: Path) -> tuple[verifier.MethodRow, ...]:
    base = verifier.load_base_sources(repository_root, verifier.TASK6_SOURCE_BASE)
    method_call_index = verifier.parse_method_call_index(
        (repository_root / verifier.DEFAULT_METHOD_CALL_INDEX).read_bytes()
    )
    lookup: dict[str, list[tuple[str, str, str, verifier.ParsedMethod]]] = {}
    by_trait: dict[str, dict[str, verifier.ParsedMethod]] = {}
    spec_index: dict[str, tuple[str, str]] = {}
    for path, trait, namespace in TARGET_SPECS:
        methods = verifier.parse_direct_trait_methods((repository_root / path).read_text(), trait)
        by_trait[trait] = {method.name: method for method in methods}
        spec_index[trait] = (path, namespace)
        for method in methods:
            lookup.setdefault(method.name, []).append((path, trait, namespace, method))

    rows: list[verifier.MethodRow] = []
    for source_object, source_path in verifier.TRAIT_SPECS.items():
        methods = verifier.parse_direct_trait_methods(base[source_path], source_object)
        names = {method.name for method in methods}
        overrides = verifier._impl_method_paths(base, source_object, names)
        calls = verifier._call_paths(base, source_object, names)
        for method in methods:
            internalized = (source_object, method.name) in {
                ("PhysicalConnection", "mark_discarded"),
                ("PhysicalConnection", "is_discarded"),
            }
            if internalized:
                if method.name == "mark_discarded":
                    target_path = "crates/druid-wrapper/src/sqlx/external_pool_connection_control.rs"
                    target_trait, target_method = "ExternalPoolLeaseControl", "poison"
                    namespace = "druid_wrapper::sqlx::external_pool_connection_control"
                else:
                    target_path = "crates/druid-wrapper/src/sqlx/external_pool_connection_control.rs"
                    target_trait, target_method = "external_pool_connection_control", "take_poisoned"
                    namespace = "druid_wrapper::sqlx"
                disposition, mapping = "INTERNALIZED", "DRUID_PRIVATE_POOL_POISON"
            else:
                chosen = MANUAL.get((source_object, method.name))
                if chosen is None:
                    priorities = PREFERRED[source_object]
                    candidates = sorted(
                        lookup.get(method.name, ()),
                        key=lambda item: priorities.index(item[1]) if item[1] in priorities else 999,
                    )
                    if not candidates:
                        raise RuntimeError(f"no canonical target: {source_object}::{method.name}")
                    target_path, target_trait, namespace, target = candidates[0]
                    target_method = method.name
                else:
                    target_trait, target_method = chosen
                    target_path, namespace = spec_index[target_trait]
                    target = by_trait[target_trait][target_method]
                disposition = "MERGED_EXACT" if method.signature == target.signature else "MERGED_TYPED"
                mapping = "IDENTITY" if disposition == "MERGED_EXACT" else conversion(source_object)
            override_text = verifier._paths_text(overrides.get(method.name, set()))
            call_text = verifier._paths_text(calls.get(method.name, set()))
            target_entry = f"{namespace}::{target_trait}::{target_method}"
            target_definition = verifier.parse_target_method(
                (repository_root / target_path).read_text(encoding="utf-8"),
                target_trait,
                target_method,
            )
            row = verifier.MethodRow(
                source_base=verifier.TASK6_SOURCE_BASE,
                source_object=source_object,
                source_trait_path=source_path,
                method=method.name,
                source_signature=method.signature,
                source_signature_sha256=method.signature_sha256,
                base_default_kind=method.default_kind,
                base_override_paths=override_text,
                base_call_paths=call_text,
                disposition=disposition,
                target_entry=target_entry,
                target_path=target_path,
                target_symbol=f"{target_trait}::{target_method}",
                conversion=mapping,
                approval_ref="-",
                behavior_evidence="UNBOUND",
                notes="BASE direct method mapped to the single canonical standard tree",
                target_trait=target_trait,
                target_signature=target_definition.signature,
                target_signature_sha256=target_definition.signature_sha256,
                target_ufcs=target_entry,
            )
            rows.append(
                replace(
                    row,
                    behavior_evidence=verifier.select_behavior_evidence(
                        row, method_call_index, repository_root
                    ),
                )
            )
    return tuple(sorted(rows))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository-root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--object")
    parser.add_argument("--start", type=int, default=0)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--no-header", action="store_true")
    parser.add_argument("--write", type=Path)
    args = parser.parse_args()
    rows = derive(args.repository_root.resolve())
    if args.object:
        rows = tuple(row for row in rows if row.source_object == args.object)
    rows = rows[args.start : None if args.limit is None else args.start + args.limit]
    rendered = verifier.render_rows(rows)
    if args.write is not None:
        target = args.write if args.write.is_absolute() else args.repository_root / args.write
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(rendered)
        return
    payload = rendered.decode()
    if args.no_header:
        payload = "\n".join(payload.splitlines()[1:]) + ("\n" if rows else "")
    print(payload, end="")


if __name__ == "__main__":
    main()
