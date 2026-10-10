#!/usr/bin/env python3
"""Render checked receiver-exact Task 6 fallback method behavior tests."""

from __future__ import annotations

import argparse
import subprocess
from collections import defaultdict
from pathlib import Path
from typing import Iterable, Sequence

import verify_task6_method_parity as verifier


GENERATED_DIRECTORY = Path(
    "crates/druid-rdbc/tests/task6_method_behavior_generated"
)
ROOT_TEST = Path(verifier.FALLBACK_EVIDENCE_PATH)

# 主证据改绑到 C2 的真实租约合同后，仍保留已存在的独立默认执行控制。
# 这是补充测试身份，不改变方法账本的主证据或 C2 来源要求。
SUPPLEMENTAL_DEFAULT_CONTROLS = frozenset({("PhysicalConnection", "execute")})

STRICT_TYPES = {
    "Connection": "StrictConnection",
    "Statement": "StrictStatement",
    "PreparedStatement": "StrictPreparedStatement",
    "CallableStatement": "StrictCallableStatement",
    "ResultSet": "StrictResultSet",
    "ResultSetExt": "StrictResultSet",
    "DatabaseMetaDataInfoExt": "StrictDatabaseMetaData",
    "DatabaseMetaDataLimitsExt": "StrictDatabaseMetaData",
    "DatabaseMetaDataQueryExt": "StrictDatabaseMetaData",
    "DatabaseMetaDataSqlCapabilitiesExt": "StrictDatabaseMetaData",
    "ResultSetMetaData": "StrictResultSetMetaData",
}

ARGUMENTS = {
    "&CallableParameter": "&CallableParameter::by_index(1).unwrap()",
    "&RdbcCalendarArgument": "&RdbcCalendarArgument::Unspecified",
    "&RdbcTargetType": "&RdbcTargetType::Integer",
    "&Savepoint": "&Savepoint { id: 1, name: None }",
    "&mut dyn PreparedStatement": "&mut prepared",
    "&str": '"task6"',
    "CallableInputParameter": "CallableInputParameter::Int(1)",
    "CallableOutParameter": "CallableOutParameter::new(4)",
    "CallableParameter": "CallableParameter::by_index(1).unwrap()",
    "ColumnIndex": "ColumnIndex::new(1).unwrap()",
    "CurrentResultDisposition": "CurrentResultDisposition::Default",
    "Duration": "Duration::from_millis(1)",
    "FetchDirection": "FetchDirection::Forward",
    "GeneratedKeys": "GeneratedKeys::None",
    "Option<&RdbcInputStream>": "None",
    "Option<&RdbcReader>": "None",
    "Option<&RdbcTypeMap>": "None",
    "Option<&[String]>": "None",
    "Option<&[i32]>": "None",
    "Option<&str>": "Some(\"task6\")",
    "Option<RdbcArray>": "None",
    "Option<RdbcBlob>": "None",
    "Option<RdbcClob>": "None",
    "Option<RdbcNClob>": "None",
    "Option<RdbcRef>": "None",
    "Option<RdbcRowId>": "None",
    "Option<RdbcSqlXml>": "None",
    "PreparedParameter": "prepared_parameter()",
    "RdbcCharacterLength": "RdbcCharacterLength::Unspecified",
    "RdbcStreamLength": "RdbcStreamLength::Unspecified",
    "RdbcTypeMap": "RdbcTypeMap::new()",
    "ResultSetHoldability": "ResultSetHoldability::CloseAtCommit",
    "ResultSetUpdate": "ResultSetUpdate::Null",
    "StatementOptions": "StatementOptions::default()",
    "String": '"task6".to_owned()',
    "TransactionIsolation": "TransactionIsolation::READ_COMMITTED",
    "Vec<(String,Vec<Value>)>": 'vec![("SELECT 1".to_owned(), vec![])]',
    "Vec<PreparedParameter>": "vec![prepared_parameter()]",
    "Vec<Value>": "vec![Value::Int(1)]",
    "Vec<Vec<PreparedInputParameter>>": (
        "vec![vec![PreparedInputParameter::RustValue(Value::Int(1))]]"
    ),
    "Vec<Vec<Value>>": "vec![vec![Value::Int(1)]]",
    "bool": "true",
    "i32": "1",
}

GROUP_FILES = {
    "Connection": "connection.inc.rs",
    "Statement": "statement.inc.rs",
    "PreparedStatement": "prepared_statement.inc.rs",
    "CallableStatement": "callable_statement.inc.rs",
    "DatabaseMetaDataInfoExt": "database_meta_data_info.inc.rs",
    "DatabaseMetaDataLimitsExt": "database_meta_data_limits.inc.rs",
    "DatabaseMetaDataQueryExt": "database_meta_data_query.inc.rs",
    "DatabaseMetaDataSqlCapabilitiesExt": (
        "database_meta_data_sql_capabilities.inc.rs"
    ),
    "ResultSet": "result_set.inc.rs",
    "ResultSetMetaData": "result_set_meta_data.inc.rs",
}


def _rustfmt(source: str) -> bytes:
    """Canonicalize generated Rust through the workspace formatter toolchain."""

    result = subprocess.run(
        ["rustfmt", "--edition", "2024", "--emit", "stdout"],
        input=source,
        capture_output=True,
        text=True,
        check=False,
        timeout=120,
    )
    if result.returncode != 0:
        raise verifier.VerificationError(
            f"rustfmt rejected generated method behavior source: {result.stderr}"
        )
    return result.stdout.encode("utf-8")


def _split_top_level(value: str) -> tuple[str, ...]:
    result: list[str] = []
    start = 0
    round_depth = square_depth = angle_depth = 0
    for index, token in enumerate(value):
        if token == "(":
            round_depth += 1
        elif token == ")":
            round_depth -= 1
        elif token == "[":
            square_depth += 1
        elif token == "]":
            square_depth -= 1
        elif token == "<":
            angle_depth += 1
        elif token == ">" and angle_depth:
            angle_depth -= 1
        elif token == "," and not (round_depth or square_depth or angle_depth):
            result.append(value[start:index])
            start = index + 1
    if value[start:]:
        result.append(value[start:])
    return tuple(part for part in result if part)


def _signature_parameters(signature: str) -> tuple[str, tuple[str, ...], bool]:
    opening = signature.find("(")
    if opening < 0:
        raise verifier.VerificationError(f"target signature has no parameters: {signature}")
    depth = 1
    closing = opening + 1
    while closing < len(signature) and depth:
        if signature[closing] == "(":
            depth += 1
        elif signature[closing] == ")":
            depth -= 1
        closing += 1
    if depth:
        raise verifier.VerificationError(f"target signature is unbalanced: {signature}")
    parts = _split_top_level(signature[opening + 1 : closing - 1])
    if not parts or parts[0] not in {"&self", "&mut self", "self"}:
        raise verifier.VerificationError(
            f"target method has no supported receiver: {signature}"
        )
    parameter_types: list[str] = []
    for parameter in parts[1:]:
        if ":" not in parameter:
            raise verifier.VerificationError(
                f"target parameter has no type: {parameter} @ {signature}"
            )
        parameter_types.append(parameter.split(":", 1)[1])
    return parts[0], tuple(parameter_types), signature.startswith("async fn ")


def _call_arguments(parameter_types: Iterable[str]) -> tuple[tuple[str, ...], tuple[str, ...]]:
    setup: list[str] = []
    arguments: list[str] = []
    for parameter_type in parameter_types:
        if parameter_type == "&mut dyn PreparedStatement" and not setup:
            setup.append(
                "    let mut prepared = StrictPreparedStatement::new(expected);"
            )
        try:
            arguments.append(ARGUMENTS[parameter_type])
        except KeyError as error:
            raise verifier.VerificationError(
                f"no strict fallback argument for target type: {parameter_type}"
            ) from error
    return tuple(setup), tuple(arguments)


def render_behavior_case(row: verifier.MethodRow) -> str:
    """Render one independent receiver-exact fallback case."""

    try:
        strict_type = STRICT_TYPES[row.target_trait]
    except KeyError as error:
        raise verifier.VerificationError(
            f"no strict fallback receiver for {row.target_trait}"
        ) from error
    receiver, parameter_types, asynchronous = _signature_parameters(
        row.target_signature
    )
    setup, arguments = _call_arguments(parameter_types)
    mutable = receiver == "&mut self"
    receiver_expression = "&mut target" if mutable else "&target"
    arguments_text = ", ".join((receiver_expression, *arguments))
    target_method = row.target_symbol.rsplit("::", 1)[-1]
    call = f"{row.target_trait}::{target_method}({arguments_text})"
    if asynchronous:
        call += ".await"
    attribute = "#[tokio::test]" if asynchronous else "#[test]"
    qualifier = "async " if asynchronous else ""
    mutability = "mut " if mutable else ""
    case_function = verifier.fallback_selector(row).function
    lines = [
        attribute,
        f"{qualifier}fn {case_function}() {{",
        f'    let expected = "{row.target_ufcs}";',
        f"    let {mutability}target = {strict_type}::new(expected);",
        *setup,
        f"    let actual = {call};",
    ]
    if row.target_trait == "Connection" and target_method == "capabilities":
        lines.append("    assert_eq!(actual, ConnectionCapabilities::default());")
    elif row.target_trait == "ResultSetMetaData" and target_method == "column_count":
        lines.append("    assert_eq!(actual, 7);")
    else:
        lines.append(f'    assert_trace(actual, "{row.target_ufcs}");')
    lines.extend(("}", ""))
    return "\n".join(lines)


def _method_lookup(
    repository_root: Path,
) -> dict[tuple[str, str], verifier.ParsedMethod]:
    lookup: dict[tuple[str, str], verifier.ParsedMethod] = {}
    target_specs = (
        ("crates/druid-rdbc/src/connection/connection.rs", "Connection"),
        ("crates/druid-rdbc/src/statement/statement.rs", "Statement"),
        (
            "crates/druid-rdbc/src/statement/prepared_statement.rs",
            "PreparedStatement",
        ),
        (
            "crates/druid-rdbc/src/statement/callable_statement.rs",
            "CallableStatement",
        ),
        ("crates/druid-rdbc/src/result_set/result_set.rs", "ResultSet"),
        (
            "crates/druid-rdbc/src/metadata/result_set_meta_data.rs",
            "ResultSetMetaData",
        ),
    )
    for path, trait in target_specs:
        source = (repository_root / path).read_text(encoding="utf-8")
        for method in verifier.parse_direct_trait_methods(source, trait):
            lookup[(trait, method.name)] = method
    return lookup


def _render_trace_impl(
    trait: str,
    strict_type: str,
    method_names: Iterable[str],
    method_lookup: dict[tuple[str, str], verifier.ParsedMethod],
) -> str:
    methods: list[str] = []
    for method_name in sorted(set(method_names)):
        method = method_lookup[(trait, method_name)]
        if trait == "Connection" and method_name == "capabilities":
            body = "ConnectionCapabilities::default()"
        elif trait == "ResultSetMetaData" and method_name == "column_count":
            body = "7"
        elif trait == "ResultSet" and method_name == "input_stream":
            # 显式流入口证明真实种类转发，不依赖通用value读取冒充重载。
            body = r'''assert_eq!(_column, ColumnIndex::new(1).unwrap());
                let expected_kind = match self.trace.expected {
                    "druid_rdbc::result_set::ResultSetExt::ascii_stream" => ResultSetStreamKind::Ascii,
                    "druid_rdbc::result_set::ResultSetExt::unicode_stream" => ResultSetStreamKind::Unicode,
                    "druid_rdbc::result_set::ResultSetExt::binary_stream" => ResultSetStreamKind::Binary,
                    unexpected => panic!("unexpected stream probe: {unexpected}"),
                };
                assert_eq!(kind, expected_kind);
                self.trace.fail()'''
        elif trait == "ResultSet" and method_name == "reader":
            body = r'''assert_eq!(_column, ColumnIndex::new(1).unwrap());
                let expected_national = match self.trace.expected {
                    "druid_rdbc::result_set::ResultSetExt::character_stream" => false,
                    "druid_rdbc::result_set::ResultSetExt::n_character_stream" => true,
                    unexpected => panic!("unexpected reader probe: {unexpected}"),
                };
                assert_eq!(national, expected_national);
                self.trace.fail()'''
        else:
            body = "self.trace.fail()"
        methods.append(f"    {method.signature} {{ {body} }}")
    async_attribute = (
        "#[async_trait::async_trait]\n" if trait != "ResultSetMetaData" else ""
    )
    return (
        f"{async_attribute}#[allow(unused_variables)]\n"
        f"impl {trait} for {strict_type} {{\n"
        + "\n".join(methods)
        + "\n}\n\n"
    )


def _common_support() -> str:
    return r'''// @generated by scripts/render_task6_method_default_evidence.py
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::extensions::ConnectionCapabilities;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, DatabaseMetaData, DatabaseMetaDataInfoExt,
    DatabaseMetaDataLimitsExt, DatabaseMetaDataQueryExt, DatabaseMetaDataRequest,
    DatabaseMetaDataSqlCapabilitiesExt, ResultSetColumnType, ResultSetMetaData, RowIdLifetime,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetStreamKind, ResultSetUpdate};
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    CurrentResultDisposition, ExecResult, ExecutionResult, FetchDirection, GeneratedKeys,
    ParameterIndex, PreparedInputParameter, PreparedParameter, PreparedStatement,
    ResultSetConcurrency, ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_rdbc::resource::{Blob, Clob, NClob};
use druid_rdbc::types::{
    RdbcArray, RdbcBigDecimal as BigDecimal, RdbcBlob, RdbcCalendarArgument,
    RdbcCharacterLength, RdbcClob, RdbcNClob,
    RdbcNaiveDate as NaiveDate, RdbcNaiveDateTime as NaiveDateTime,
    RdbcNaiveTime as NaiveTime, RdbcInputStream, RdbcObject, RdbcReader, RdbcRef, RdbcRowId,
    RdbcSqlXml, RdbcStreamLength, RdbcTargetType, RdbcTypeMap, RdbcUrl, Row, Savepoint,
    Value,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
struct TraceExpectation {
    expected: &'static str,
}

impl TraceExpectation {
    const fn new(expected: &'static str) -> Self {
        Self { expected }
    }

    fn fail<T>(&self) -> Result<T, RdbcError> {
        Err(RdbcError::Other(self.expected.to_owned()))
    }
}

fn assert_trace<T>(actual: Result<T, RdbcError>, expected: &'static str) {
    match actual {
        Err(RdbcError::Other(actual)) => assert_eq!(actual, expected),
        Err(other) => panic!("expected exact trace {expected}, got {other:?}"),
        Ok(_) => panic!("expected exact trace {expected}, got success"),
    }
}

fn prepared_parameter() -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(1).unwrap(),
        value: Value::Int(1),
    }
}

macro_rules! strict_type {
    ($name:ident) => {
        #[derive(Debug)]
        struct $name {
            trace: TraceExpectation,
        }

        impl $name {
            const fn new(expected: &'static str) -> Self {
                Self {
                    trace: TraceExpectation::new(expected),
                }
            }
        }

        impl Wrapper for $name {
            fn wrapper_view(&self) -> &dyn Wrapper {
                self
            }

            fn canonical_unwrap(&self, _target: TypeId) -> Option<StandardUnwrapped<'_>> {
                None
            }

            fn unwrap(&self, _target: Option<TypeId>) -> Result<StandardUnwrapped<'_>, RdbcError> {
                self.trace.fail()
            }
        }
    };
}

strict_type!(StrictConnection);
strict_type!(StrictStatement);
strict_type!(StrictPreparedStatement);
strict_type!(StrictCallableStatement);
strict_type!(StrictResultSet);
strict_type!(StrictDatabaseMetaData);
strict_type!(StrictResultSetMetaData);

#[async_trait::async_trait]
impl Statement for StrictPreparedStatement {}
#[async_trait::async_trait]
impl Statement for StrictCallableStatement {}
#[async_trait::async_trait]
impl PreparedStatement for StrictCallableStatement {}

#[async_trait::async_trait]
impl DatabaseMetaData for StrictDatabaseMetaData {
    async fn metadata_boolean(
        &mut self,
        _request: DatabaseMetaDataRequest,
    ) -> Result<bool, RdbcError> {
        self.trace.fail()
    }

    async fn metadata_string(
        &mut self,
        _request: DatabaseMetaDataRequest,
    ) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }

    async fn metadata_integer(
        &mut self,
        _request: DatabaseMetaDataRequest,
    ) -> Result<i32, RdbcError> {
        self.trace.fail()
    }

    async fn metadata_result_set(
        &mut self,
        _request: DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.trace.fail()
    }

    async fn metadata_row_id_lifetime(
        &mut self,
        _request: DatabaseMetaDataRequest,
    ) -> Result<RowIdLifetime, RdbcError> {
        self.trace.fail()
    }
}
'''


def _root_test(include_files: Sequence[str]) -> str:
    includes = "\n".join(
        f'include!("task6_method_behavior_generated/{name}");'
        for name in include_files
    )
    return f'''//! Checked generated receiver-exact evidence for Task 6 canonical methods.
#![allow(clippy::too_many_lines)]

{includes}
'''


def _render_group(
    trait: str,
    rows: Sequence[verifier.MethodRow],
    method_lookup: dict[tuple[str, str], verifier.ParsedMethod],
) -> str:
    support = ""
    if trait in {
        "Connection",
        "Statement",
        "PreparedStatement",
        "CallableStatement",
        "ResultSet",
    }:
        required = {row.target_symbol.rsplit("::", 1)[-1] for row in rows}
        if trait == "ResultSet":
            # 列名扩展现在委托 canonical hook；严格探针须覆盖实际叶入口，
            # 不再依赖 find_column 失败来冒充对象重载调用证据。
            required.update({
                "object_with_type_map_by_label", "object_as_by_label",
                "input_stream", "reader",
            })
        support = _render_trace_impl(
            trait,
            STRICT_TYPES[trait],
            required,
            method_lookup,
        )
    elif trait == "ResultSetMetaData":
        required = {row.target_symbol.rsplit("::", 1)[-1] for row in rows}
        required.update({"column", "column_count"})
        support = _render_trace_impl(
            trait,
            STRICT_TYPES[trait],
            required,
            method_lookup,
        )
    cases = "\n".join(render_behavior_case(row) for row in rows)
    return f"// @generated receiver-exact group: {trait}\n{support}{cases}"


def render_files(
    repository_root: Path, rows: Sequence[verifier.MethodRow]
) -> dict[Path, bytes]:
    fallback = [
        row
        for row in rows
        if verifier.parse_behavior_selector(row.behavior_evidence).source_path
        == verifier.FALLBACK_EVIDENCE_PATH
        or (
            (row.source_object, row.method) in SUPPLEMENTAL_DEFAULT_CONTROLS
            and row.base_default_kind == "DEFAULT_UNSUPPORTED"
        )
    ]
    grouped: dict[str, list[verifier.MethodRow]] = defaultdict(list)
    for row in fallback:
        grouped[row.target_trait].append(row)
    method_lookup = _method_lookup(repository_root)
    files: dict[Path, bytes] = {
        GENERATED_DIRECTORY / "common.inc.rs": _rustfmt(_common_support())
    }
    include_files = ["common.inc.rs"]
    for trait, filename in GROUP_FILES.items():
        trait_rows = sorted(grouped.get(trait, ()))
        files[GENERATED_DIRECTORY / filename] = _rustfmt(
            _render_group(trait, trait_rows, method_lookup)
        )
        include_files.append(filename)

    result_set_ext_rows = sorted(grouped.get("ResultSetExt", ()))
    midpoint = (len(result_set_ext_rows) + 1) // 2
    for part, part_rows in enumerate(
        (result_set_ext_rows[:midpoint], result_set_ext_rows[midpoint:]), start=1
    ):
        filename = f"result_set_ext_part_{part:02}.inc.rs"
        files[GENERATED_DIRECTORY / filename] = _rustfmt(
            _render_group("ResultSetExt", part_rows, method_lookup)
        )
        include_files.append(filename)
    files[ROOT_TEST] = _rustfmt(_root_test(include_files))
    return files


def _load_rows(repository_root: Path, matrix: Path) -> tuple[verifier.MethodRow, ...]:
    path = matrix if matrix.is_absolute() else repository_root / matrix
    return verifier.parse_rows(path.read_bytes())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository-root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
    )
    parser.add_argument("--matrix", type=Path, default=verifier.DEFAULT_MATRIX)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    root = arguments.repository_root.resolve()
    files = render_files(root, _load_rows(root, arguments.matrix))
    if arguments.write:
        for relative, payload in files.items():
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(payload)
        return 0
    if arguments.check:
        drift = [
            str(relative)
            for relative, payload in files.items()
            if not (root / relative).is_file()
            or (root / relative).read_bytes() != payload
        ]
        if drift:
            raise verifier.VerificationError(
                f"generated method behavior matrix drift: {drift}"
            )
        print(f"PASS generated method behavior matrix: files={len(files)}")
        return 0
    for relative, payload in sorted(files.items()):
        print(f"// FILE: {relative}")
        print(payload.decode("utf-8"), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
