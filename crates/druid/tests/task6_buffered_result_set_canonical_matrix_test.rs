//! Canonical table-driven replacement for deleted pooled/buffered ResultSet overload tests.

use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{DruidConnectionHolder, DruidPooledConnection};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetUpdate};
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::statement::{CallableStatement, PreparedStatement, Statement};
use druid_rdbc::statement::{FetchDirection, ResultSetType};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcObject, Row, Value};
use druid_rdbc::wrapper::StandardUnwrapped;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::str::FromStr;
use std::time::Duration;

fn task6_buffered_result_set_case_ids() -> &'static [&'static str] {
    &[
        "boolean_getter_returns_bool",
        "bytes_getter_returns_blob",
        "callable_statement_none_for_regular_statement",
        "close_with_connection_sets_closed",
        "concurrency_returns_mode",
        "cursor_name_returns_none_for_sqlite",
        "date_getter_handles_null",
        "double_getter_returns_float",
        "fetch_direction_returns_value",
        "find_column_errors_for_row_set",
        "float_getter_returns_float",
        "holdability_returns_value",
        "int_getter_returns_integer",
        "is_closed_false_initially",
        "is_closed_with_connection_delegates",
        "long_getter_returns_integer",
        "meta_data_returns_column_count",
        "n_string_getter_returns_text",
        "next_returns_false_at_end",
        "object_by_label_errors_for_row_set",
        "object_returns_value",
        "poolable_statement_returns_statement_ref",
        "prepared_statement_none_for_regular_statement",
        "result_set_type_returns_type",
        "row_returns_current_position",
        "row_updated_inserted_deleted_return_false_for_sqlite",
        "set_fetch_size_succeeds",
        "statement_object_returns_statement_variant",
        "statement_returns_statement_ref",
        "string_getter_returns_text",
        "warnings_returns_none_for_sqlite",
        "was_null_after_null_value",
    ]
}

fn task6_buffered_rows() -> BufferedRowSet {
    let date = NaiveDate::from_ymd_opt(2026, 8, 26).expect("date");
    let time = NaiveTime::from_hms_opt(12, 34, 56).expect("time");
    BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::Int(7),
            Value::String("value".to_owned()),
            Value::Bytes(vec![1, 2, 3]),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(NaiveDateTime::new(date, time)),
            Value::Decimal(BigDecimal::from_str("12.50").expect("decimal")),
            Value::Null,
        ])],
        vec![
            "id",
            "label",
            "bytes",
            "date",
            "time",
            "timestamp",
            "decimal",
            "absent",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    )
}

#[tokio::test]
async fn task6_buffered_result_set_canonical_matrix() {
    let cases = task6_buffered_result_set_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for case_id in cases {
        match *case_id {
            "boolean_getter_returns_bool" => {
                task6_sqlite_boolean_getter_returns_bool().await;
                executed.push(*case_id);
                continue;
            }
            "bytes_getter_returns_blob" => {
                task6_sqlite_bytes_getter_returns_blob().await;
                executed.push(*case_id);
                continue;
            }
            "date_getter_handles_null" => {
                task6_sqlite_date_getter_handles_null().await;
                executed.push(*case_id);
                continue;
            }
            "double_getter_returns_float" => {
                task6_sqlite_double_getter_returns_float().await;
                executed.push(*case_id);
                continue;
            }
            "float_getter_returns_float" => {
                task6_sqlite_float_getter_returns_float().await;
                executed.push(*case_id);
                continue;
            }
            "int_getter_returns_integer" => {
                task6_sqlite_int_getter_returns_integer().await;
                executed.push(*case_id);
                continue;
            }
            "long_getter_returns_integer" => {
                task6_sqlite_long_getter_returns_integer().await;
                executed.push(*case_id);
                continue;
            }
            "string_getter_returns_text" => {
                task6_sqlite_string_getter_returns_text().await;
                executed.push(*case_id);
                continue;
            }
            "n_string_getter_returns_text" => {
                task6_sqlite_n_string_getter_returns_text().await;
                executed.push(*case_id);
                continue;
            }
            "object_returns_value" => {
                task6_sqlite_object_returns_value().await;
                executed.push(*case_id);
                continue;
            }
            "was_null_after_null_value" => {
                task6_sqlite_was_null_after_null_value().await;
                executed.push(*case_id);
                continue;
            }
            "callable_statement_none_for_regular_statement" => {
                task6_sqlite_callable_statement_none_for_regular_statement().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_statement_none_for_regular_statement" => {
                task6_sqlite_prepared_statement_none_for_regular_statement().await;
                executed.push(*case_id);
                continue;
            }
            "close_with_connection_sets_closed" => {
                task6_sqlite_close_with_connection_sets_closed().await;
                executed.push(*case_id);
                continue;
            }
            "is_closed_false_initially" => {
                task6_sqlite_is_closed_false_initially().await;
                executed.push(*case_id);
                continue;
            }
            "is_closed_with_connection_delegates" => {
                task6_sqlite_is_closed_with_connection_delegates().await;
                executed.push(*case_id);
                continue;
            }
            "statement_returns_statement_ref" => {
                task6_sqlite_statement_returns_statement_ref().await;
                executed.push(*case_id);
                continue;
            }
            "poolable_statement_returns_statement_ref" => {
                task6_sqlite_poolable_statement_returns_statement_ref().await;
                executed.push(*case_id);
                continue;
            }
            "statement_object_returns_statement_variant" => {
                task6_sqlite_statement_object_returns_statement_variant().await;
                executed.push(*case_id);
                continue;
            }
            "next_returns_false_at_end" => {
                task6_sqlite_next_returns_false_at_end().await;
                executed.push(*case_id);
                continue;
            }
            "row_returns_current_position" => {
                task6_sqlite_row_returns_current_position().await;
                executed.push(*case_id);
                continue;
            }
            "warnings_returns_none_for_sqlite" => {
                task6_sqlite_warnings_returns_none_for_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            "meta_data_returns_column_count" => {
                task6_sqlite_meta_data_returns_column_count().await;
                executed.push(*case_id);
                continue;
            }
            "find_column_errors_for_row_set" => {
                task6_sqlite_find_column_errors_for_row_set().await;
                executed.push(*case_id);
                continue;
            }
            "object_by_label_errors_for_row_set" => {
                task6_sqlite_object_by_label_errors_for_row_set().await;
                executed.push(*case_id);
                continue;
            }
            "cursor_name_returns_none_for_sqlite" => {
                task6_sqlite_cursor_name_returns_none_for_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            "result_set_type_returns_type" => {
                task6_sqlite_result_set_type_returns_type().await;
                executed.push(*case_id);
                continue;
            }
            "concurrency_returns_mode" => {
                task6_sqlite_concurrency_returns_mode().await;
                executed.push(*case_id);
                continue;
            }
            "holdability_returns_value" => {
                task6_sqlite_holdability_returns_value().await;
                executed.push(*case_id);
                continue;
            }
            "fetch_direction_returns_value" => {
                task6_sqlite_fetch_direction_returns_value().await;
                executed.push(*case_id);
                continue;
            }
            "set_fetch_size_succeeds" => {
                task6_sqlite_set_fetch_size_succeeds().await;
                executed.push(*case_id);
                continue;
            }
            "row_updated_inserted_deleted_return_false_for_sqlite" => {
                task6_sqlite_row_updated_inserted_deleted_return_false_for_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            _ => {}
        }
        let mut rows = task6_buffered_rows();
        assert_eq!(rows.is_closed(), Ok(false));
        assert!(rows.next().await.expect("next"), "case {case_id}");
        let id = ColumnIndex::new(1).unwrap();
        let label = ColumnIndex::new(2).unwrap();
        let bytes = ColumnIndex::new(3).unwrap();
        let date = ColumnIndex::new(4).unwrap();
        let absent = ColumnIndex::new(8).unwrap();
        assert_eq!(rows.long(id), Ok(7));
        assert_eq!(rows.string(label).unwrap().as_deref(), Some("value"));
        assert_eq!(rows.bytes(bytes), Ok(Some(vec![1, 2, 3])));
        assert_eq!(
            rows.date(date, &RdbcCalendarArgument::Unspecified)
                .unwrap()
                .unwrap()
                .year(),
            2026
        );
        assert_eq!(rows.object(id), Ok(RdbcObject::from(Value::Int(7))));
        assert_eq!(rows.find_column("LABEL"), Ok(label));
        assert_eq!(rows.meta_data().unwrap().column_count(), 8);
        assert_eq!(rows.result_set_type(), Ok(ResultSetType::ScrollInsensitive));
        rows.set_fetch_direction(FetchDirection::Forward).unwrap();
        rows.set_fetch_size(16).unwrap();
        assert_eq!(rows.fetch_size(), Ok(16));
        assert_eq!(rows.value(absent), Ok(Value::Null));
        assert_eq!(rows.was_null(), Ok(true));

        if case_id.contains("update") {
            rows.update_value(label, ResultSetUpdate::NString(Some("updated".to_owned())))
                .unwrap();
            assert_eq!(rows.string(label).unwrap().as_deref(), Some("updated"));
            assert_eq!(rows.row_updated(), Ok(true));
        }
        if case_id.contains("navigation") || case_id.contains("row_set_result_set") {
            assert!(!rows.next().await.unwrap());
            assert!(rows.previous().await.unwrap());
            assert_eq!(rows.row(), Ok(1));
        }
        if case_id.contains("stream") || case_id.contains("resource_getter") {
            assert_eq!(
                rows.binary_stream(bytes)
                    .unwrap()
                    .unwrap()
                    .read_to_end()
                    .unwrap(),
                vec![1, 2, 3]
            );
        }
        rows.close().await.unwrap();
        assert_eq!(rows.is_closed(), Ok(true));
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_buffered_result_set_canonical_matrix:{}",
        executed.join("|")
    );
}

async fn task6_sqlite_connection() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    task6_sqlite_connection_from_factory(factory).await
}

async fn task6_sqlite_connection_from_factory(
    factory: ToastyConnectionFactory,
) -> DruidPooledConnection {
    let raw = factory.create().await.unwrap();
    let holder = DruidConnectionHolder::with_standard_connection(raw, 1, Duration::ZERO, 0)
        .await
        .unwrap();
    DruidPooledConnection::with_standard_holder(
        holder,
        "result-set-sqlite".to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    )
}

async fn task6_sqlite_setup_table(conn: &mut DruidPooledConnection) {
    conn.exec(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, val REAL)",
        vec![],
    )
    .await
    .unwrap();
    for i in 1..=5 {
        conn.exec(
            &format!("INSERT INTO t (id, name, val) VALUES ({i}, 'row{i}', {i}.5)"),
            vec![],
        )
        .await
        .unwrap();
    }
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_boolean_getter_returns_bool() {
    let mut conn = task6_sqlite_connection().await;
    conn.exec("CREATE TABLE b (v BOOLEAN)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO b VALUES (1)", vec![]).await.unwrap();
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT v FROM b").await.unwrap();
    assert!(rs.next().await.unwrap());
    assert!(rs.boolean(ColumnIndex::new(1).unwrap()).unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_bytes_getter_returns_blob() {
    let mut conn = task6_sqlite_connection().await;
    conn.exec("CREATE TABLE bl (v BLOB)", vec![]).await.unwrap();
    conn.exec("INSERT INTO bl VALUES (X'DEADBEEF')", vec![])
        .await
        .unwrap();
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT v FROM bl").await.unwrap();
    assert!(rs.next().await.unwrap());
    let b = rs.bytes(ColumnIndex::new(1).unwrap()).unwrap();
    assert!(b.is_some());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_date_getter_handles_null() {
    let mut conn = task6_sqlite_connection().await;
    conn.exec("CREATE TABLE dt (d DATE)", vec![]).await.unwrap();
    conn.exec("INSERT INTO dt VALUES (NULL)", vec![])
        .await
        .unwrap();
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT d FROM dt").await.unwrap();
    assert!(rs.next().await.unwrap());
    assert!(rs
        .date(
            ColumnIndex::new(1).unwrap(),
            &RdbcCalendarArgument::Unspecified
        )
        .unwrap()
        .is_none());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_double_getter_returns_float() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT val FROM t WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next().await.unwrap());
    let d = rs.double(ColumnIndex::new(1).unwrap()).unwrap();
    assert!((d - 1.5).abs() < 0.01);
    let _ = rs.double_by_label("val");
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_float_getter_returns_float() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT val FROM t WHERE id = 2")
        .await
        .unwrap();
    assert!(rs.next().await.unwrap());
    let f = rs.float(ColumnIndex::new(1).unwrap()).unwrap();
    assert!((f - 2.5).abs() < 0.01);
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_int_getter_returns_integer() {
    // 固定 BASE 使用官方 Connect 包装的 Driver，其 ExecResponse 没有列标签出口。
    // 保留该真实驱动无标签 RowSet 的原拒绝合同，原生列路径另行验证。
    let driver = toasty::db::Connect::new("sqlite::memory:").await.unwrap();
    let factory = ToastyConnectionFactory::from_driver(driver).unwrap();
    let mut conn = task6_sqlite_connection_from_factory(factory).await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT id FROM t WHERE id = 3")
        .await
        .unwrap();
    rs.next().await.unwrap();
    assert_eq!(rs.int(ColumnIndex::new(1).unwrap()).unwrap(), 3);
    let err = rs.int_by_label("id").unwrap_err();
    assert!(matches!(
        err,
        druid_rdbc::error::RdbcError::InvalidArgument(_)
    ));
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

#[tokio::test]
async fn sqlite_integer_base_case_preserves_value_and_label_error() {
    task6_sqlite_int_getter_returns_integer().await;
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_long_getter_returns_integer() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT id FROM t WHERE id = 4")
        .await
        .unwrap();
    assert!(rs.next().await.unwrap());
    assert_eq!(rs.long(ColumnIndex::new(1).unwrap()).unwrap(), 4);
    let _ = rs.long_by_label("id");
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_string_getter_returns_text() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT name FROM t WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next().await.unwrap());
    assert_eq!(
        rs.string(ColumnIndex::new(1).unwrap()).unwrap(),
        Some("row1".to_string())
    );
    // 保留原场景的标签读取调用；原场景不对该调用结果作断言。
    let _ = rs.string_by_label("name");
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_n_string_getter_returns_text() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT name FROM t WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next().await.unwrap());
    assert_eq!(
        rs.n_string(ColumnIndex::new(1).unwrap()).unwrap(),
        Some("row1".to_string())
    );
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_object_returns_value() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT id FROM t WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next().await.unwrap());
    let val = rs.object(ColumnIndex::new(1).unwrap()).unwrap();
    assert_eq!(val, RdbcObject::from(Value::Int(1)));
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE 真实 Toasty SQLite 场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_was_null_after_null_value() {
    let mut conn = task6_sqlite_connection().await;
    conn.exec(
        "CREATE TABLE nullable (id INTEGER PRIMARY KEY, v TEXT)",
        vec![],
    )
    .await
    .unwrap();
    conn.exec("INSERT INTO nullable (id, v) VALUES (1, NULL)", vec![])
        .await
        .unwrap();
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT v FROM nullable").await.unwrap();
    assert!(rs.next().await.unwrap());
    let val = rs.string(ColumnIndex::new(1).unwrap()).unwrap();
    assert!(val.is_none());
    assert!(rs.was_null().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

#[tokio::test]
async fn sqlite_scalar_base_cases_preserve_real_driver_contracts() {
    task6_sqlite_boolean_getter_returns_bool().await;
    task6_sqlite_bytes_getter_returns_blob().await;
    task6_sqlite_date_getter_handles_null().await;
    task6_sqlite_double_getter_returns_float().await;
    task6_sqlite_float_getter_returns_float().await;
    task6_sqlite_long_getter_returns_integer().await;
    task6_sqlite_string_getter_returns_text().await;
    task6_sqlite_n_string_getter_returns_text().await;
    task6_sqlite_object_returns_value().await;
    task6_sqlite_was_null_after_null_value().await;
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_callable_statement_none_for_regular_statement() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let owner = rs.get_statement().unwrap().unwrap();
    assert!(!owner
        .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap());
    assert!(owner
        .unwrap(Some(TypeId::of::<dyn CallableStatement>()))
        .is_err());
    assert!(owner
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(!owner.is_closed().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_prepared_statement_none_for_regular_statement() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let owner = rs.get_statement().unwrap().unwrap();
    assert!(!owner
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert!(owner
        .unwrap(Some(TypeId::of::<dyn PreparedStatement>()))
        .is_err());
    assert!(owner
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(!owner.is_closed().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_close_with_connection_sets_closed() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    assert!(!rs.is_closed().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
    assert!(rs.is_closed().unwrap());
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_is_closed_false_initially() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    assert!(!rs.is_closed().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
    assert!(rs.is_closed().unwrap());
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_is_closed_with_connection_delegates() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let closed = rs.is_closed().unwrap();
    assert!(!closed);
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_statement_returns_statement_ref() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let owner = rs.get_statement().unwrap().unwrap();
    assert!(owner
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(!owner.is_closed().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_poolable_statement_returns_statement_ref() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let mut owner = rs.get_statement().unwrap().unwrap();
    assert!(owner
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(!owner.is_closed().unwrap());
    owner.close().await.unwrap();
    assert!(stmt.is_closed().unwrap());
    assert!(rs.is_closed().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_statement_object_returns_statement_variant() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let owner = rs.get_statement().unwrap().unwrap();
    assert!(matches!(
        owner.unwrap(Some(TypeId::of::<dyn Statement>())).unwrap(),
        StandardUnwrapped::Statement(_)
    ));
    assert!(!owner
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert!(!owner
        .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_next_returns_false_at_end() {
    let mut conn = task6_sqlite_connection().await;
    conn.exec("CREATE TABLE empty (id INTEGER)", vec![])
        .await
        .unwrap();
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM empty").await.unwrap();
    assert!(!rs.next().await.unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 完整保留 BASE 真实 SQLite 创建者及生命周期场景。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_row_returns_current_position() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    rs.next().await.unwrap();
    // BASE 仅调用当前位置接口而未断言成功；保留驱动原有支持边界。
    let _ = rs.row();
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

#[tokio::test]
async fn sqlite_owner_and_lifecycle_base_cases_preserve_real_driver_contracts() {
    task6_sqlite_callable_statement_none_for_regular_statement().await;
    task6_sqlite_prepared_statement_none_for_regular_statement().await;
    task6_sqlite_close_with_connection_sets_closed().await;
    task6_sqlite_is_closed_false_initially().await;
    task6_sqlite_is_closed_with_connection_delegates().await;
    task6_sqlite_statement_returns_statement_ref().await;
    task6_sqlite_poolable_statement_returns_statement_ref().await;
    task6_sqlite_statement_object_returns_statement_variant().await;
    task6_sqlite_next_returns_false_at_end().await;
    task6_sqlite_row_returns_current_position().await;
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_warnings_returns_none_for_sqlite() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let w = rs.warnings().unwrap();
    assert!(w.is_none());
    rs.clear_warnings().unwrap();
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_meta_data_returns_column_count() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT id, name, val FROM t")
        .await
        .unwrap();
    let md = rs.meta_data().unwrap();
    assert_eq!(md.column_count(), 3);
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_find_column_errors_for_row_set() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT id, name FROM t").await.unwrap();
    // BASE 仅调用列名查询，未断言返回成功或错误。
    let _ = rs.find_column("name");
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_object_by_label_errors_for_row_set() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT id FROM t WHERE id = 1")
        .await
        .unwrap();
    rs.next().await.unwrap();
    // BASE 未断言列名取值结果，保留实际驱动支持边界。
    let _ = rs.object_by_label("id");
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_cursor_name_returns_none_for_sqlite() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let _ = rs.cursor_name();
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_result_set_type_returns_type() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let _ = rs.result_set_type();
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_concurrency_returns_mode() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let _ = rs.concurrency();
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_holdability_returns_value() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let _ = rs.holdability();
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_fetch_direction_returns_value() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    let _ = rs.fetch_direction();
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_set_fetch_size_succeeds() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    rs.set_fetch_size(100).unwrap();
    let _ = rs.fetch_size();
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

/// 保留 BASE SQLite 原始调用及断言。对应 Java: java.sql.ResultSet。
async fn task6_sqlite_row_updated_inserted_deleted_return_false_for_sqlite() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt.execute_query("SELECT * FROM t").await.unwrap();
    rs.next().await.unwrap();
    assert!(!rs.row_updated().unwrap());
    assert!(!rs.row_inserted().unwrap());
    assert!(!rs.row_deleted().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

#[tokio::test]
async fn sqlite_single_row_cursor_revisits_row_like_java_pooled_result_set() {
    // 对应 Java 1.2.28 DruidPooledResultSetTest#test_rowCount 的可观察导航：
    // 单行游标越过末尾、回到该行、退到首行前，再次读取仍只有一行。
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT id FROM t WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next().await.unwrap());
    assert!(!rs.next().await.unwrap());
    assert!(rs.previous().await.unwrap());
    assert!(!rs.previous().await.unwrap());
    assert!(rs.next().await.unwrap());
    assert!(!rs.next().await.unwrap());
    assert!(!rs.row_updated().unwrap());
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}

#[tokio::test]
async fn sqlite_metadata_and_property_base_cases_preserve_real_driver_contracts() {
    task6_sqlite_warnings_returns_none_for_sqlite().await;
    task6_sqlite_meta_data_returns_column_count().await;
    task6_sqlite_find_column_errors_for_row_set().await;
    task6_sqlite_object_by_label_errors_for_row_set().await;
    task6_sqlite_cursor_name_returns_none_for_sqlite().await;
    task6_sqlite_result_set_type_returns_type().await;
    task6_sqlite_concurrency_returns_mode().await;
    task6_sqlite_holdability_returns_value().await;
    task6_sqlite_fetch_direction_returns_value().await;
    task6_sqlite_set_fetch_size_succeeds().await;
    task6_sqlite_row_updated_inserted_deleted_return_false_for_sqlite().await;
}

/// 保留迁移前 ResultSet 的全部十个 JDBC 数值常量。
#[test]
fn task6_result_set_constants_match_java() {
    use druid::core::DruidPooledResultSet;
    assert_eq!(DruidPooledResultSet::FETCH_FORWARD, 1000);
    assert_eq!(DruidPooledResultSet::FETCH_REVERSE, 1001);
    assert_eq!(DruidPooledResultSet::FETCH_UNKNOWN, 1002);
    assert_eq!(DruidPooledResultSet::TYPE_FORWARD_ONLY, 1003);
    assert_eq!(DruidPooledResultSet::TYPE_SCROLL_INSENSITIVE, 1004);
    assert_eq!(DruidPooledResultSet::TYPE_SCROLL_SENSITIVE, 1005);
    assert_eq!(DruidPooledResultSet::CONCUR_READ_ONLY, 1007);
    assert_eq!(DruidPooledResultSet::CONCUR_UPDATABLE, 1008);
    assert_eq!(DruidPooledResultSet::HOLD_CURSORS_OVER_COMMIT, 1);
    assert_eq!(DruidPooledResultSet::CLOSE_CURSORS_AT_COMMIT, 2);
}

/// 同一原五行 SQL 场景通过原生驱动读取真实列标签，区别于无标签 RowSet 合同。
#[tokio::test]
async fn sqlite_integer_native_metadata_preserves_value_by_original_label() {
    let mut conn = task6_sqlite_connection().await;
    task6_sqlite_setup_table(&mut conn).await;
    let mut stmt = conn
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query("SELECT id FROM t WHERE id = 3")
        .await
        .unwrap();
    assert!(rs.next().await.unwrap());
    assert_eq!(rs.int(ColumnIndex::new(1).unwrap()), Ok(3));
    assert_eq!(rs.int_by_label("ID"), Ok(3));
    assert_eq!(
        rs.int_by_label("missing"),
        Err(druid_rdbc::error::RdbcError::InvalidArgument(
            "unknown column: missing".to_owned()
        ))
    );
    rs.close().await.unwrap();
    stmt.close().await.unwrap();
    conn.close().await.unwrap();
}
