//! Canonical Toasty adapter matrix replacing connection-owned/private prepared test coupling.

use druid_rdbc::metadata::DatabaseMetaDataInfoExt;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::toasty::ToastyConnectionFactory;

fn task6_toasty_case_ids() -> &'static [&'static str] {
    &[
        "auto_commit_default_is_true",
        "begin_commit_transaction",
        "begin_rollback_transaction",
        "begin_when_already_active_errors",
        "capabilities_for_sqlite",
        "clear_warnings_after_close_errors",
        "clear_warnings_succeeds",
        "close_and_is_closed",
        "close_is_idempotent",
        "close_prepared_statement_succeeds",
        "commit_without_transaction_errors",
        "database_meta_data_driver_name",
        "database_meta_data_returns_url",
        "default_transaction_isolation_for_sqlite",
        "driver_name_is_consistent_across_calls",
        "driver_name_is_sqlite",
        "exec_after_close_errors",
        "exec_batch_multiple_statements",
        "exec_create_table_and_insert",
        "exec_on_discarded_connection_errors",
        "exec_prepared_parameter_batch",
        "exec_prepared_update",
        "exec_prepared_with_update_counts",
        "exec_update_and_delete",
        "exec_with_date_param",
        "exec_with_null_param",
        "exec_with_timestamp_param_roundtrip",
        "execute_prepared_on_closed_statement_errors",
        "execute_prepared_query",
        "execute_prepared_query_with_where_clause",
        "execute_query_returns_result_set",
        "execute_update_returns_update_count",
        "execute_with_generated_key_columns_errors",
        "fetch_empty_result",
        "fetch_prepared_result_set_returns_arc",
        "fetch_prepared_result_set_returns_result_set",
        "fetch_prepared_returns_matching_rows",
        "fetch_prepared_with_value_params",
        "fetch_returns_rows",
        "fetch_with_bool_param",
        "fetch_with_bytes_param",
        "fetch_with_decimal_param",
        "fetch_with_float_param",
        "fetch_with_params",
        "fetch_with_string_param",
        "mark_discarded_and_is_discarded",
        "named_savepoint",
        "ping_after_close_errors",
        "ping_on_open_connection",
        "prepare_and_exec_prepared",
        "prepare_and_fetch_prepared",
        "prepare_physical_call_returns_non_callable_statement",
        "read_only_default_is_false",
        "rollback_without_transaction_errors",
        "savepoint_empty_name_errors",
        "savepoint_name_with_special_chars_errors",
        "savepoint_name_with_underscore_ok",
        "savepoint_rollback_to_and_release",
        "sequential_operations_reuse_connection",
        "set_auto_commit_false_begins_transaction",
        "set_auto_commit_noop_when_same",
        "set_read_only_during_transaction_errors",
        "set_read_only_true_errors_for_sqlite",
        "set_transaction_isolation_during_active_transaction_errors",
        "set_transaction_isolation_invalid_level_errors",
        "set_transaction_isolation_non_serializable_errors_for_sqlite",
        "set_transaction_isolation_serializable",
        "warnings_after_close_errors",
        "warnings_returns_none",
    ]
}

fn parameter(index: usize, value: Value) -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(index).unwrap(),
        value,
    }
}

async fn task6_toasty_connection() -> Box<dyn druid_rdbc::connection::Connection> {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite factory");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("Toasty connection");
    connection
        .exec(
            "CREATE TABLE toasty_case(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            Vec::new(),
        )
        .await
        .unwrap();
    connection
}

#[tokio::test]
async fn task6_toasty_canonical_matrix() {
    let cases = task6_toasty_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for case_id in cases {
        let mut connection = task6_toasty_connection().await;
        match *case_id {
            "ping_after_close_errors" => {
                connection.close().await.unwrap();
                let err = connection.ping().await.unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::ConnectionDiscarded
                ));
            }
            "ping_on_open_connection" => {
                connection.ping().await.unwrap();
            }
            "read_only_default_is_false" => {
                assert!(!connection.read_only().unwrap());
            }
            "sequential_operations_reuse_connection" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER)", vec![])
                    .await
                    .unwrap();

                // Insert
                for i in 1..=10 {
                    connection
                        .exec(
                            &format!("INSERT INTO t (id, v) VALUES ({i}, {i} * 10)"),
                            vec![],
                        )
                        .await
                        .unwrap();
                }

                // Update
                let r = connection
                    .exec("UPDATE t SET v = 0 WHERE id <= 5", vec![])
                    .await
                    .unwrap();
                assert_eq!(r.rows_affected, 5);

                // Query
                let rows = connection
                    .fetch("SELECT * FROM t WHERE v = 0", vec![])
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 5);

                // Delete
                let r = connection
                    .exec("DELETE FROM t WHERE id > 8", vec![])
                    .await
                    .unwrap();
                assert_eq!(r.rows_affected, 2);

                // Final count
                let rows = connection.fetch("SELECT * FROM t", vec![]).await.unwrap();
                assert_eq!(rows.len(), 8);
            }
            "set_auto_commit_false_begins_transaction" => {
                connection.set_auto_commit(false).await.unwrap();
                assert!(!connection.auto_commit().unwrap());
                connection.set_auto_commit(true).await.unwrap(); // commits
                assert!(connection.auto_commit().unwrap());
            }
            "set_auto_commit_noop_when_same" => {
                // Already true, setting true is a no-op
                connection.set_auto_commit(true).await.unwrap();
                assert!(connection.auto_commit().unwrap());
            }
            "set_read_only_during_transaction_errors" => {
                connection.begin().await.unwrap();
                let err = connection.set_read_only(false).await.unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::InvalidArgument(_)
                ));
                connection.rollback().await.unwrap();
            }
            "set_read_only_true_errors_for_sqlite" => {
                let err = connection.set_read_only(true).await.unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::UnsupportedOperation { .. }
                ));
            }
            "set_transaction_isolation_during_active_transaction_errors" => {
                connection.begin().await.unwrap();
                let err = connection
                    .set_transaction_isolation(
                        druid_rdbc::connection::TransactionIsolation::from_jdbc(8),
                    )
                    .await
                    .unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::InvalidArgument(_)
                ));
                connection.rollback().await.unwrap();
            }
            "set_transaction_isolation_invalid_level_errors" => {
                let err = connection
                    .set_transaction_isolation(
                        druid_rdbc::connection::TransactionIsolation::from_jdbc(99),
                    )
                    .await
                    .unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::InvalidArgument(_)
                ));
            }
            "set_transaction_isolation_non_serializable_errors_for_sqlite" => {
                let err = connection
                    .set_transaction_isolation(
                        druid_rdbc::connection::TransactionIsolation::from_jdbc(2),
                    )
                    .await
                    .unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::InvalidArgument(_)
                ));
            }
            "set_transaction_isolation_serializable" => {
                connection
                    .set_transaction_isolation(
                        druid_rdbc::connection::TransactionIsolation::from_jdbc(8),
                    )
                    .await
                    .unwrap();
                assert_eq!(connection.transaction_isolation().unwrap().jdbc_code(), 8);
            }
            "warnings_after_close_errors" => {
                connection.close().await.unwrap();
                let err = connection.warnings().await.unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::ConnectionDiscarded
                ));
            }
            "warnings_returns_none" => {
                let w = connection.warnings().await.unwrap();
                assert!(w.is_none());
            }
            "mark_discarded_and_is_discarded" => {
                assert!(!connection.is_closed().unwrap());
                connection.abort().await.unwrap();
                assert!(connection.is_closed().unwrap());
                assert!(matches!(
                    connection.exec("SELECT 1", vec![]).await,
                    Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
                ));
            }
            "prepare_and_exec_prepared" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement(
                        "INSERT INTO t (id, val) VALUES (?, ?)",
                        StatementOptions::default(),
                    )
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(1)))
                    .unwrap();
                statement
                    .set_parameter(parameter(2, Value::String("world".into())))
                    .unwrap();
                assert_eq!(statement.execute_prepared_update().await.unwrap(), 1);
                assert_eq!(
                    connection
                        .fetch("SELECT id, val FROM t", vec![])
                        .await
                        .unwrap()[0]
                        .values,
                    vec![Value::Int(1), Value::String("world".into())]
                );
                statement.close().await.unwrap();
            }
            "prepare_and_fetch_prepared" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO t (id, val) VALUES (1, 'foo')", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement(
                        "SELECT val FROM t WHERE id = ?",
                        StatementOptions::default(),
                    )
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(1)))
                    .unwrap();
                let mut rows = statement.execute_prepared_query().await.unwrap();
                let mut values = Vec::new();
                while rows.next().await.unwrap() {
                    values.push(rows.value(ColumnIndex::new(1).unwrap()).unwrap());
                }
                assert_eq!(values.len(), 1);
                assert_eq!(values, vec![Value::String("foo".into())]);
                rows.close().await.unwrap();
                statement.close().await.unwrap();
            }
            "prepare_physical_call_returns_non_callable_statement" => {
                // BASE 只要求调用不 panic，成功或拒绝都允许；不据此认证 callable 能力。
                let result = connection
                    .prepare_call("SELECT 1", StatementOptions::default())
                    .await;
                if let Ok(mut statement) = result {
                    statement.close().await.unwrap();
                }
            }
            "fetch_empty_result" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                let rows = connection.fetch("SELECT * FROM t", vec![]).await.unwrap();
                assert!(rows.is_empty());
            }
            "fetch_returns_rows" => {
                let rows = connection
                    .fetch("SELECT 42 AS answer", vec![])
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
            }
            "fetch_with_bool_param" => {
                let rows = connection
                    .fetch(
                        "SELECT 1 WHERE ? = ?",
                        vec![Value::Bool(true), Value::Bool(true)],
                    )
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
            }
            "fetch_with_bytes_param" => {
                let rows = connection
                    .fetch("SELECT ? AS val", vec![Value::Bytes(vec![1, 2, 3])])
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
            }
            "fetch_with_decimal_param" => {
                use bigdecimal::BigDecimal;
                use std::str::FromStr;
                let rows = connection
                    .fetch(
                        "SELECT 1 WHERE ? > 0",
                        vec![Value::Decimal(BigDecimal::from_str("1.5").unwrap())],
                    )
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
            }
            "fetch_with_float_param" => {
                // 原 BASE 的十进制输入为 3.14，并非圆周率；保留相同浮点值。
                let rows = connection
                    .fetch("SELECT 1 WHERE ? > 0.0", vec![Value::Float(314.0 / 100.0)])
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
            }
            "fetch_with_params" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO t (id, val) VALUES (1, 'hello')", vec![])
                    .await
                    .unwrap();
                let rows = connection
                    .fetch("SELECT val FROM t WHERE id = ?", vec![Value::Int(1)])
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
            }
            "fetch_with_string_param" => {
                let rows = connection
                    .fetch("SELECT ? AS val", vec![Value::String("hello".to_string())])
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
            }
            "execute_prepared_on_closed_statement_errors" => {
                let mut statement = connection
                    .prepare_statement("SELECT 1", StatementOptions::default())
                    .await
                    .unwrap();
                statement.close().await.unwrap();
                let err = statement
                    .execute_prepared()
                    .await
                    .expect_err("closed Prepared must reject execution");
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::ConnectionDiscarded
                ));
            }
            "execute_prepared_query" => {
                let mut statement = connection
                    .prepare_statement("SELECT 1 + ? AS result", StatementOptions::default())
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(2)))
                    .unwrap();
                let mut results = statement.execute_prepared().await.unwrap();
                assert!(!results.is_empty());
                let druid_rdbc::statement::ExecutionResult::ResultSet(rows) = &mut results[0]
                else {
                    panic!("query must return a result set");
                };
                assert!(rows.next().await.unwrap());
                assert_eq!(
                    rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                    Value::Int(3)
                );
                rows.close().await.unwrap();
                statement.close().await.unwrap();
            }
            "execute_prepared_query_with_where_clause" => {
                connection
                    .exec("CREATE TABLE epp (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO epp VALUES (1)", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement(
                        "SELECT * FROM epp WHERE id = ?",
                        StatementOptions::default(),
                    )
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(1)))
                    .unwrap();
                let mut results = statement.execute_prepared().await.unwrap();
                assert!(!results.is_empty());
                let druid_rdbc::statement::ExecutionResult::ResultSet(rows) = &mut results[0]
                else {
                    panic!("query must return a result set");
                };
                assert!(rows.next().await.unwrap());
                assert_eq!(
                    rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                    Value::Int(1)
                );
                rows.close().await.unwrap();
                statement.close().await.unwrap();
            }
            "execute_query_returns_result_set" => {
                let mut statement = connection
                    .create_statement(StatementOptions::default())
                    .await
                    .unwrap();
                let mut results = statement
                    .execute(
                        "SELECT 1 + 1 AS result",
                        druid_rdbc::statement::GeneratedKeys::None,
                    )
                    .await
                    .unwrap();
                assert!(!results.is_empty());
                let druid_rdbc::statement::ExecutionResult::ResultSet(rows) = &mut results[0]
                else {
                    panic!("query must return a result set");
                };
                assert!(rows.next().await.unwrap());
                assert_eq!(
                    rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                    Value::Int(2)
                );
                rows.close().await.unwrap();
                statement.close().await.unwrap();
            }
            "execute_update_returns_update_count" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .create_statement(StatementOptions::default())
                    .await
                    .unwrap();
                let results = statement
                    .execute(
                        "INSERT INTO t (id) VALUES (1)",
                        druid_rdbc::statement::GeneratedKeys::None,
                    )
                    .await
                    .unwrap();
                assert!(!results.is_empty());
                assert!(matches!(
                    results.as_slice(),
                    [druid_rdbc::statement::ExecutionResult::UpdateCount(1)]
                ));
                statement.close().await.unwrap();
            }
            "execute_with_generated_key_columns_errors" => {
                connection
                    .exec(
                        "CREATE TABLE gk (id INTEGER PRIMARY KEY AUTOINCREMENT)",
                        vec![],
                    )
                    .await
                    .unwrap();
                let mut statement = connection
                    .create_statement(StatementOptions::default())
                    .await
                    .unwrap();
                let err = statement
                    .execute(
                        "INSERT INTO gk DEFAULT VALUES",
                        druid_rdbc::statement::GeneratedKeys::ColumnIndexes(vec![
                            ColumnIndex::new(1).unwrap(),
                        ]),
                    )
                    .await
                    .expect_err("ordinary Statement column keys unsupported");
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::UnsupportedOperation { .. }
                ));
                statement.close().await.unwrap();
            }
            "fetch_prepared_result_set_returns_arc" => {
                connection
                    .exec("CREATE TABLE fprs (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO fprs VALUES (1)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO fprs VALUES (2)", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement(
                        "SELECT * FROM fprs WHERE id <= ?",
                        StatementOptions::default(),
                    )
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(2)))
                    .unwrap();
                let mut rows = statement.execute_prepared_query().await.unwrap();
                assert!(!rows.is_closed().unwrap());
                assert!(rows.next().await.unwrap());
                assert_eq!(
                    rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                    Value::Int(1)
                );
                assert!(rows.next().await.unwrap());
                assert_eq!(
                    rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                    Value::Int(2)
                );
                assert!(!rows.next().await.unwrap());
                rows.close().await.unwrap();
                statement.close().await.unwrap();
            }
            "fetch_prepared_result_set_returns_result_set" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO t (id) VALUES (1)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO t (id) VALUES (2)", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement("SELECT * FROM t ORDER BY id", StatementOptions::default())
                    .await
                    .unwrap();
                let mut rows = statement.execute_prepared_query().await.unwrap();
                assert!(rows.next().await.unwrap());
                assert_eq!(
                    rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                    Value::Int(1)
                );
                assert!(rows.next().await.unwrap());
                assert_eq!(
                    rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                    Value::Int(2)
                );
                assert!(!rows.next().await.unwrap());
                rows.close().await.unwrap();
                statement.close().await.unwrap();
            }
            "fetch_prepared_returns_matching_rows" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY, x INTEGER)", vec![])
                    .await
                    .unwrap();
                for i in 1..=5 {
                    connection
                        .exec(&format!("INSERT INTO t (id, x) VALUES ({i}, {i})"), vec![])
                        .await
                        .unwrap();
                }
                let mut statement = connection
                    .prepare_statement("SELECT * FROM t WHERE x <= ?", StatementOptions::default())
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(3)))
                    .unwrap();
                let mut rows = statement.execute_prepared_query().await.unwrap();
                let mut values = Vec::new();
                while rows.next().await.unwrap() {
                    values.push(vec![
                        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                        rows.value(ColumnIndex::new(2).unwrap()).unwrap(),
                    ]);
                }
                assert_eq!(values.len(), 3);
                assert_eq!(
                    values,
                    vec![
                        vec![Value::Int(1), Value::Int(1)],
                        vec![Value::Int(2), Value::Int(2)],
                        vec![Value::Int(3), Value::Int(3)]
                    ]
                );
                rows.close().await.unwrap();
                statement.close().await.unwrap();
            }
            "fetch_prepared_with_value_params" => {
                connection
                    .exec("CREATE TABLE fp (id INTEGER PRIMARY KEY, v TEXT)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO fp VALUES (1, 'hello')", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement("SELECT v FROM fp WHERE id = ?", StatementOptions::default())
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(1)))
                    .unwrap();
                let mut rows = statement.execute_prepared_query().await.unwrap();
                let mut values = Vec::new();
                while rows.next().await.unwrap() {
                    values.push(rows.value(ColumnIndex::new(1).unwrap()).unwrap());
                }
                assert_eq!(values.len(), 1);
                assert_eq!(values, vec![Value::String("hello".into())]);
                rows.close().await.unwrap();
                statement.close().await.unwrap();
            }
            "database_meta_data_driver_name" => {
                let mut meta = connection.database_meta_data().await.unwrap();
                let name = meta.get_driver_name().await.unwrap();
                assert!(name.is_some());
            }
            "database_meta_data_returns_url" => {
                let mut meta = connection.database_meta_data().await.unwrap();
                let url = meta.get_url().await.unwrap();
                assert!(url.is_some(), "url should be present");
                assert!(url.unwrap().contains("sqlite"), "url should contain sqlite");
            }
            "default_transaction_isolation_for_sqlite" => {
                // SQLite defaults to SERIALIZABLE (8)
                assert_eq!(connection.transaction_isolation().unwrap().jdbc_code(), 8);
            }
            "driver_name_is_consistent_across_calls" => {
                assert_eq!(connection.driver_name().unwrap(), "SQLite");
                assert_eq!(connection.driver_name().unwrap(), "SQLite");
            }
            "driver_name_is_sqlite" => {
                assert_eq!(connection.driver_name().unwrap(), "SQLite");
            }
            "exec_after_close_errors" => {
                connection.close().await.unwrap();
                let err = connection.exec("SELECT 1", vec![]).await.unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::ConnectionDiscarded
                ));
            }
            "exec_batch_multiple_statements" => {
                connection
                    .exec("CREATE TABLE bt (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                let counts = connection
                    .exec_batch(vec![
                        ("INSERT INTO bt (id) VALUES (1)".to_string(), vec![]),
                        ("INSERT INTO bt (id) VALUES (2)".to_string(), vec![]),
                        ("INSERT INTO bt (id) VALUES (3)".to_string(), vec![]),
                    ])
                    .await
                    .unwrap();
                assert_eq!(counts.len(), 3);
                assert!(counts.iter().all(|&c| c == 1));
            }
            "exec_create_table_and_insert" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)", vec![])
                    .await
                    .unwrap();

                let result = connection
                    .exec("INSERT INTO t (id, name) VALUES (1, 'Alice')", vec![])
                    .await
                    .unwrap();
                assert_eq!(result.rows_affected, 1);
                assert!(
                    result.last_insert_id.is_some(),
                    "SQLite should return rowid"
                );
            }
            "exec_update_and_delete" => {
                connection
                    .exec(
                        "CREATE TABLE t (id INTEGER PRIMARY KEY, val INTEGER)",
                        vec![],
                    )
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO t (id, val) VALUES (1, 10)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO t (id, val) VALUES (2, 20)", vec![])
                    .await
                    .unwrap();

                let update_result = connection
                    .exec("UPDATE t SET val = 99 WHERE id = 1", vec![])
                    .await
                    .unwrap();
                assert_eq!(update_result.rows_affected, 1);

                let delete_result = connection
                    .exec("DELETE FROM t WHERE id = 2", vec![])
                    .await
                    .unwrap();
                assert_eq!(delete_result.rows_affected, 1);
            }
            "exec_with_date_param" => {
                connection
                    .exec("CREATE TABLE dt (d DATE, t TIME, ts TIMESTAMP)", vec![])
                    .await
                    .unwrap();
                let d = chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
                let t = chrono::NaiveTime::from_hms_milli_opt(14, 30, 0, 500).unwrap();
                let ts = chrono::NaiveDateTime::parse_from_str(
                    "2025-06-15 14:30:00.000",
                    "%Y-%m-%d %H:%M:%S%.f",
                )
                .unwrap();
                let result = connection
                    .exec(
                        "INSERT INTO dt VALUES (?, ?, ?)",
                        vec![Value::Date(d), Value::Time(t), Value::Timestamp(ts)],
                    )
                    .await
                    .unwrap();
                assert_eq!(result.rows_affected, 1);
            }
            "exec_with_null_param" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)", vec![])
                    .await
                    .unwrap();
                let result = connection
                    .exec(
                        "INSERT INTO t (id, val) VALUES (?, ?)",
                        vec![Value::Int(1), Value::Null],
                    )
                    .await
                    .unwrap();
                assert_eq!(result.rows_affected, 1);
            }
            "exec_with_timestamp_param_roundtrip" => {
                connection
                    .exec("CREATE TABLE ts_rt (ts TIMESTAMP)", vec![])
                    .await
                    .unwrap();
                let ts = chrono::NaiveDateTime::parse_from_str(
                    "2025-01-15 10:30:00.123",
                    "%Y-%m-%d %H:%M:%S%.f",
                )
                .unwrap();
                connection
                    .exec("INSERT INTO ts_rt VALUES (?)", vec![Value::Timestamp(ts)])
                    .await
                    .unwrap();
                let rows = connection
                    .fetch("SELECT ts FROM ts_rt", vec![])
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
            }
            "exec_on_discarded_connection_errors" => {
                // 原始 mark_discarded 的执行拒绝映射为标准强制中止。
                connection.abort().await.unwrap();
                let err = connection.exec("SELECT 1", vec![]).await.unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::ConnectionDiscarded
                ));
            }
            "exec_prepared_parameter_batch" => {
                connection
                    .exec(
                        "CREATE TABLE batch (id INTEGER PRIMARY KEY, v TEXT)",
                        vec![],
                    )
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement(
                        "INSERT INTO batch (id, v) VALUES (?, ?)",
                        StatementOptions::default(),
                    )
                    .await
                    .unwrap();
                for (id, value) in [(1, "a"), (2, "b"), (3, "c")] {
                    statement
                        .add_input_parameter_batch(vec![
                            (
                                ParameterIndex::new(1).unwrap(),
                                druid_rdbc::statement::PreparedInputParameter::Int(id),
                            ),
                            (
                                ParameterIndex::new(2).unwrap(),
                                druid_rdbc::statement::PreparedInputParameter::String(Some(
                                    value.to_owned(),
                                )),
                            ),
                        ])
                        .unwrap();
                }
                let counts = statement.execute_prepared_batch().await.unwrap();
                assert_eq!(counts.len(), 3);
                assert!(counts.iter().all(|&count| count == 1));
                let rows = connection
                    .fetch("SELECT id, v FROM batch ORDER BY id", vec![])
                    .await
                    .unwrap();
                assert_eq!(
                    rows.into_iter().map(|row| row.values).collect::<Vec<_>>(),
                    vec![
                        vec![Value::Int(1), Value::String("a".into())],
                        vec![Value::Int(2), Value::String("b".into())],
                        vec![Value::Int(3), Value::String("c".into())]
                    ]
                );
                statement.close().await.unwrap();
            }
            "exec_prepared_update" => {
                connection
                    .exec(
                        "CREATE TABLE epu (id INTEGER PRIMARY KEY, v INTEGER)",
                        vec![],
                    )
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO epu VALUES (1, 10)", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement(
                        "UPDATE epu SET v = ? WHERE id = 1",
                        StatementOptions::default(),
                    )
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(99)))
                    .unwrap();
                assert_eq!(statement.execute_prepared_update().await.unwrap(), 1);
                assert_eq!(
                    connection
                        .fetch("SELECT v FROM epu WHERE id = 1", vec![])
                        .await
                        .unwrap()[0]
                        .values,
                    vec![Value::Int(99)]
                );
                statement.close().await.unwrap();
            }
            "exec_prepared_with_update_counts" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                let mut statement = connection
                    .prepare_statement("INSERT INTO t (id) VALUES (?)", StatementOptions::default())
                    .await
                    .unwrap();
                statement
                    .set_parameter(parameter(1, Value::Int(10)))
                    .unwrap();
                assert_eq!(statement.execute_prepared_update().await.unwrap(), 1);
                assert_eq!(
                    connection.fetch("SELECT id FROM t", vec![]).await.unwrap()[0].values,
                    vec![Value::Int(10)]
                );
                statement.close().await.unwrap();
            }
            "auto_commit_default_is_true" => {
                assert!(connection.auto_commit().unwrap());
            }
            "begin_commit_transaction" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                connection.begin().await.unwrap();
                connection
                    .exec("INSERT INTO t (id) VALUES (1)", vec![])
                    .await
                    .unwrap();
                connection.commit().await.unwrap();
                let rows = connection.fetch("SELECT * FROM t", vec![]).await.unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].values, vec![Value::Int(1)]);
            }
            "begin_rollback_transaction" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();
                connection
                    .exec("INSERT INTO t (id) VALUES (1)", vec![])
                    .await
                    .unwrap();
                connection.begin().await.unwrap();
                connection
                    .exec("INSERT INTO t (id) VALUES (2)", vec![])
                    .await
                    .unwrap();
                connection.rollback().await.unwrap();
                let rows = connection.fetch("SELECT * FROM t", vec![]).await.unwrap();
                assert_eq!(rows.len(), 1, "rolled-back row must not persist");
                assert_eq!(rows[0].values, vec![Value::Int(1)]);
            }
            "capabilities_for_sqlite" => {
                let caps = connection.capabilities();
                assert!(caps.transactions);
                assert!(caps.savepoints);
                // 已移除的旧 capability 位通过标准操作证明，不重建 Physical descriptor。
                assert!(connection.auto_commit().unwrap());
                connection.set_auto_commit(false).await.unwrap();
                assert!(!connection.auto_commit().unwrap());
                connection.rollback().await.unwrap();
                assert!(matches!(
                    connection.set_read_only(true).await,
                    Err(druid_rdbc::error::RdbcError::UnsupportedOperation { .. })
                ));
                connection
                    .set_transaction_isolation(
                        druid_rdbc::connection::TransactionIsolation::SERIALIZABLE,
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    connection.transaction_isolation().unwrap(),
                    druid_rdbc::connection::TransactionIsolation::SERIALIZABLE
                );
                assert!(matches!(
                    connection.holdability(),
                    Err(druid_rdbc::error::RdbcError::UnsupportedOperation { .. })
                ));
            }
            "clear_warnings_after_close_errors" => {
                connection.close().await.unwrap();
                let err = connection.clear_warnings().await.unwrap_err();
                assert!(matches!(
                    err,
                    druid_rdbc::error::RdbcError::ConnectionDiscarded
                ));
            }
            "clear_warnings_succeeds" => {
                connection.clear_warnings().await.unwrap();
            }
            "close_and_is_closed" => {
                assert!(!connection.is_closed().unwrap());
                connection.close().await.unwrap();
                assert!(connection.is_closed().unwrap());
            }
            "close_is_idempotent" => {
                connection.close().await.unwrap();
                connection.close().await.unwrap();
                assert!(connection.is_closed().unwrap());
            }
            "close_prepared_statement_succeeds" => {
                let mut statement = connection
                    .prepare_statement("SELECT 1", StatementOptions::default())
                    .await
                    .unwrap();
                statement.close().await.unwrap();
                assert!(statement.is_closed().unwrap());
            }
            "begin_when_already_active_errors" => {
                connection.begin().await.unwrap();
                assert!(matches!(
                    connection.begin().await,
                    Err(druid_rdbc::error::RdbcError::DriverError(_))
                ));
                connection.rollback().await.unwrap();
            }
            "commit_without_transaction_errors" => {
                assert!(matches!(
                    connection.commit().await,
                    Err(druid_rdbc::error::RdbcError::DriverError(_))
                ));
            }
            "rollback_without_transaction_errors" => {
                assert!(matches!(
                    connection.rollback().await,
                    Err(druid_rdbc::error::RdbcError::DriverError(_))
                ));
            }
            "named_savepoint" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();

                connection.begin().await.unwrap();
                let sp = connection.set_savepoint_named("my_sp").await.unwrap();
                assert!(sp.name.is_some());
                connection.release_savepoint(&sp).await.unwrap();
                connection.commit().await.unwrap();
            }
            "savepoint_empty_name_errors" => {
                connection.begin().await.unwrap();
                let result = connection.set_savepoint_named("").await;
                assert!(result.is_err());
                connection.rollback().await.unwrap();
            }
            "savepoint_name_with_special_chars_errors" => {
                connection.begin().await.unwrap();
                let result = connection.set_savepoint_named("my sp!").await;
                assert!(result.is_err());
                connection.rollback().await.unwrap();
            }
            "savepoint_name_with_underscore_ok" => {
                connection.begin().await.unwrap();
                let sp = connection
                    .set_savepoint_named("my_savepoint_1")
                    .await
                    .unwrap();
                assert_eq!(sp.name.as_deref(), Some("my_savepoint_1"));
                connection.release_savepoint(&sp).await.unwrap();
                connection.commit().await.unwrap();
            }
            "savepoint_rollback_to_and_release" => {
                connection
                    .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
                    .await
                    .unwrap();

                connection.begin().await.unwrap();
                connection
                    .exec("INSERT INTO t (id) VALUES (1)", vec![])
                    .await
                    .unwrap();

                let sp = connection.set_savepoint().await.unwrap();
                connection
                    .exec("INSERT INTO t (id) VALUES (2)", vec![])
                    .await
                    .unwrap();

                connection.rollback_to(&sp).await.unwrap();
                // After rollback_to, row 2 should be gone but row 1 remains
                connection.release_savepoint(&sp).await.unwrap();
                connection.commit().await.unwrap();

                let rows = connection.fetch("SELECT * FROM t", vec![]).await.unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].values, vec![Value::Int(1)]);
            }
            _ => panic!("unmapped Toasty BASE case: {case_id}"),
        }
        executed.push(*case_id);
        connection.close().await.unwrap();
        assert_eq!(connection.is_closed(), Ok(true));
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_toasty_canonical_matrix:{}",
        executed.join("|")
    );
}
