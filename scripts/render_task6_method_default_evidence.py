#!/usr/bin/env python3
"""Render the executable fallback matrix for Task 6 methods with no BASE direct test."""

from __future__ import annotations

from collections import defaultdict
from pathlib import Path

import render_task6_method_matrix as matrix
import verify_task6_method_parity as verifier


METADATA_ARGS = {
    "deletes_are_detected": "1003",
    "get_attributes": "None, None, None, None",
    "get_best_row_identifier": "None, None, None, 0, false",
    "get_column_privileges": "None, None, None, None",
    "get_function_columns": "None, None, None, None",
    "get_procedure_columns": "None, None, None, None",
    "get_pseudo_columns": "None, None, None, None",
    "get_schemas_with_pattern": "None, None",
    "get_super_tables": "None, None, None",
    "get_super_types": "None, None, None",
    "get_table_privileges": "None, None, None",
    "get_ud_ts": "None, None, None, None",
    "get_version_columns": "None, None, None",
    "inserts_are_detected": "1003",
    "others_deletes_are_visible": "1003",
    "others_inserts_are_visible": "1003",
    "others_updates_are_visible": "1003",
    "own_deletes_are_visible": "1003",
    "own_inserts_are_visible": "1003",
    "own_updates_are_visible": "1003",
    "supports_result_set_holdability": "2",
    "updates_are_detected": "1003",
}


def fallback_rows(root: Path) -> tuple[verifier.MethodRow, ...]:
    return tuple(
        row
        for row in matrix.derive(root)
        if verifier.parse_behavior_selector(row.behavior_evidence).source_path
        == verifier.FALLBACK_EVIDENCE_PATH
    )


def call_expression(row: verifier.MethodRow) -> tuple[str, bool]:
    owner, method = row.target_symbol.rsplit("::", 1)
    if owner.startswith("DatabaseMetaData"):
        arguments = METADATA_ARGS.get(method, "")
        return f"metadata.{method}({arguments})", True
    if owner == "CallableStatement":
        if method == "set_named_parameter":
            return (
                'callable.set_named_parameter("value", CallableInputParameter::Int(1))',
                False,
            )
        if method == "out_parameter_as":
            return "callable.out_parameter_as(&parameter, &RdbcTargetType::Integer)", False
        if method == "out_parameter_with_type_map":
            return "callable.out_parameter_with_type_map(&parameter, None)", False
        return f"callable.{method}(&parameter)", False
    if owner == "PreparedStatement":
        if method == "set_parameter":
            return "prepared.set_parameter(prepared_parameter())", False
        if method == "add_parameter_batch":
            return "prepared.add_parameter_batch(vec![prepared_parameter()])", False
        return f"prepared.{method}()", method.startswith("execute_")
    if owner == "Statement":
        if method == "get_more_results":
            return "statement.get_more_results(CurrentResultDisposition::Default)", True
        return f"statement.{method}()", method in {"get_generated_keys", "get_result_set"}
    if owner == "ResultSet":
        if method == "update_value":
            return "result_set.update_value(column, ResultSetUpdate::Null)", False
        if method == "update_value_by_label":
            return 'result_set.update_value_by_label("value", ResultSetUpdate::Null)', False
        if method == "value_by_label":
            return 'result_set.value_by_label("value")', False
    raise RuntimeError(
        f"no fallback call renderer for {row.source_object}::{row.method} -> {row.target_symbol}"
    )


def render(root: Path) -> str:
    rows = fallback_rows(root)
    cases = "\n".join(
        f'        "{verifier.fallback_case_id(row)}",' for row in rows
    )
    grouped_arms: dict[tuple[str, str, str], list[str]] = defaultdict(list)
    for row in rows:
        expression, asynchronous = call_expression(row)
        if asynchronous:
            expression += ".await"
        helper = "assert_metadata_error" if row.source_object == "PhysicalDatabaseMetaData" else "assert_fail_closed"
        expected = (
            f', "{row.target_symbol.rsplit("::", 1)[-1]}"'
            if helper == "assert_metadata_error"
            else ""
        )
        grouped_arms[(helper, expression, expected)].append(
            verifier.fallback_case_id(row)
        )
    arms = []
    for (helper, expression, expected), case_ids in grouped_arms.items():
        patterns = " | ".join(f'"{case_id}"' for case_id in case_ids)
        arms.append(f"        {patterns} => {helper}({expression}{expected}),")
    match_arms = "\n".join(arms)
    return f'''//! Executable method evidence for BASE methods without a direct BASE behavior test.

use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{{
    DatabaseMetaData, DatabaseMetaDataInfoExt, DatabaseMetaDataLimitsExt,
    DatabaseMetaDataQueryExt, DatabaseMetaDataSqlCapabilitiesExt,
}};
use druid_rdbc::result_set::{{ColumnIndex, ResultSet, ResultSetUpdate}};
use druid_rdbc::statement::{{
    CallableInputParameter, CallableParameter, CallableStatement, CurrentResultDisposition,
    ParameterIndex, PreparedParameter, PreparedStatement, Statement,
}};
use druid_rdbc::types::{{RdbcTargetType, Value}};

struct DefaultMetadata;
druid_rdbc::impl_standard_wrapper!(DefaultMetadata => DatabaseMetaData);
#[async_trait::async_trait]
impl DatabaseMetaData for DefaultMetadata {{}}

struct DefaultStatement;
druid_rdbc::impl_standard_wrapper!(DefaultStatement => Statement);
#[async_trait::async_trait]
impl Statement for DefaultStatement {{}}

struct DefaultPrepared;
druid_rdbc::impl_standard_wrapper!(DefaultPrepared => PreparedStatement);
#[async_trait::async_trait]
impl Statement for DefaultPrepared {{}}
#[async_trait::async_trait]
impl PreparedStatement for DefaultPrepared {{}}

struct DefaultCallable;
druid_rdbc::impl_standard_wrapper!(DefaultCallable => CallableStatement);
#[async_trait::async_trait]
impl Statement for DefaultCallable {{}}
#[async_trait::async_trait]
impl PreparedStatement for DefaultCallable {{}}
#[async_trait::async_trait]
impl CallableStatement for DefaultCallable {{}}

struct DefaultResultSet;
druid_rdbc::impl_standard_wrapper!(DefaultResultSet => ResultSet);
#[async_trait::async_trait]
impl ResultSet for DefaultResultSet {{
    async fn next(&mut self) -> Result<bool, RdbcError> {{
        Ok(false)
    }}
}}

fn prepared_parameter() -> PreparedParameter {{
    PreparedParameter {{
        index: ParameterIndex::new(1).unwrap(),
        value: Value::Int(1),
    }}
}}

fn assert_fail_closed<T>(result: Result<T, RdbcError>) {{
    assert!(result.is_err(), "canonical default must fail closed");
}}

fn assert_metadata_error<T>(result: Result<T, RdbcError>, operation: &'static str) {{
    assert!(matches!(
        result,
        Err(RdbcError::UnsupportedOperation {{ operation: actual }}) if actual == operation
    ));
}}

fn task6_method_default_evidence_case_ids() -> &'static [&'static str] {{
    &[
{cases}
    ]
}}

async fn exercise(case_id: &str) {{
    let mut metadata = DefaultMetadata;
    let mut statement = DefaultStatement;
    let mut prepared = DefaultPrepared;
    let mut callable = DefaultCallable;
    let mut result_set = DefaultResultSet;
    let parameter = CallableParameter::by_index(1).unwrap();
    let column = ColumnIndex::new(1).unwrap();
    match case_id {{
{match_arms}
        unknown => panic!("unbound Task 6 method evidence case: {{unknown}}"),
    }}
}}

#[tokio::test]
async fn task6_method_default_evidence_matrix() {{
    let cases = task6_method_default_evidence_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for case_id in cases {{
        exercise(case_id).await;
        executed.push(*case_id);
    }}
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_method_default_evidence_matrix:{{}}",
        executed.join("|")
    );
}}
'''


if __name__ == "__main__":
    print(render(Path(__file__).resolve().parent.parent), end="")
