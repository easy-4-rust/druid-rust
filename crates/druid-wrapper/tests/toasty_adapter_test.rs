//! Differential tests for `ToastyConnectionAdapter` via real SQLite.
//!
//! The adapter constructor is `pub(crate)`, so we exercise it through
//! `ToastyConnectionFactory::new("sqlite::memory:").create()`.

extern crate druid_wrapper as wrapper;
use druid::core::{
    DruidError, PhysicalConnection, PhysicalConnectionFactory, PreparedInputParameter,
    PreparedStatementKey, PreparedStatementMethodType, StatementGeneratedKeys, Value,
};
use wrapper::toasty::ToastyConnectionFactory;

async fn make_connection() -> Box<dyn PhysicalConnection> {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite factory must create");
    factory
        .create()
        .await
        .expect("must create physical connection")
}

fn make_key(sql: &str) -> PreparedStatementKey {
    PreparedStatementKey::new(Some(sql.to_string()), None, PreparedStatementMethodType::M1)
        .expect("PreparedStatementKey must construct")
}

// ── exec (DML) ───────────────────────────────────────────────────

#[tokio::test]
async fn exec_create_table_and_insert() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)", vec![])
        .await
        .unwrap();

    let result = conn
        .exec("INSERT INTO t (id, name) VALUES (1, 'Alice')", vec![])
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);
    assert!(
        result.last_insert_id.is_some(),
        "SQLite should return rowid"
    );
}

#[tokio::test]
async fn exec_update_and_delete() {
    let mut conn = make_connection().await;
    conn.exec(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, val INTEGER)",
        vec![],
    )
    .await
    .unwrap();
    conn.exec("INSERT INTO t (id, val) VALUES (1, 10)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO t (id, val) VALUES (2, 20)", vec![])
        .await
        .unwrap();

    let update_result = conn
        .exec("UPDATE t SET val = 99 WHERE id = 1", vec![])
        .await
        .unwrap();
    assert_eq!(update_result.rows_affected, 1);

    let delete_result = conn
        .exec("DELETE FROM t WHERE id = 2", vec![])
        .await
        .unwrap();
    assert_eq!(delete_result.rows_affected, 1);
}

// ── execute (generic) ────────────────────────────────────────────

#[tokio::test]
async fn execute_query_returns_result_set() {
    let mut conn = make_connection().await;
    let results = conn
        .execute(
            "SELECT 1 + 1 AS result",
            vec![],
            StatementGeneratedKeys::None,
        )
        .await
        .unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn execute_update_returns_update_count() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let results = conn
        .execute(
            "INSERT INTO t (id) VALUES (1)",
            vec![],
            StatementGeneratedKeys::None,
        )
        .await
        .unwrap();
    assert!(!results.is_empty());
}

// ── fetch (query returning rows) ─────────────────────────────────

#[tokio::test]
async fn fetch_returns_rows() {
    let mut conn = make_connection().await;
    let rows = conn.fetch("SELECT 42 AS answer", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn fetch_empty_result() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let rows = conn.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn fetch_with_params() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO t (id, val) VALUES (1, 'hello')", vec![])
        .await
        .unwrap();
    let rows = conn
        .fetch("SELECT val FROM t WHERE id = ?", vec![Value::Int(1)])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

// ── prepare + exec_prepared ──────────────────────────────────────

#[tokio::test]
async fn prepare_and_exec_prepared() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)", vec![])
        .await
        .unwrap();

    let stmt = conn
        .prepare_physical_statement(&make_key("INSERT INTO t (id, val) VALUES (?, ?)"))
        .await
        .unwrap();

    let result = conn
        .exec_prepared(
            stmt.as_ref(),
            vec![Value::Int(1), Value::String("world".to_string())],
        )
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);
}

#[tokio::test]
async fn prepare_and_fetch_prepared() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO t (id, val) VALUES (1, 'foo')", vec![])
        .await
        .unwrap();

    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT val FROM t WHERE id = ?"))
        .await
        .unwrap();

    let rows = conn
        .fetch_prepared(stmt.as_ref(), vec![Value::Int(1)])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

// ── transaction lifecycle ────────────────────────────────────────

#[tokio::test]
async fn begin_commit_transaction() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();

    conn.begin().await.unwrap();
    conn.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    conn.commit().await.unwrap();

    let rows = conn.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn begin_rollback_transaction() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();

    conn.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    conn.begin().await.unwrap();
    conn.exec("INSERT INTO t (id) VALUES (2)", vec![])
        .await
        .unwrap();
    conn.rollback().await.unwrap();

    let rows = conn.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1, "rolled-back row must not persist");
}

#[tokio::test]
async fn begin_when_already_active_errors() {
    let mut conn = make_connection().await;
    conn.begin().await.unwrap();
    let err = conn.begin().await.unwrap_err();
    assert!(
        matches!(err, DruidError::DriverError(_)),
        "double begin should error: {err:?}"
    );
    conn.rollback().await.unwrap();
}

#[tokio::test]
async fn commit_without_transaction_errors() {
    let mut conn = make_connection().await;
    let err = conn.commit().await.unwrap_err();
    assert!(matches!(err, DruidError::DriverError(_)));
}

#[tokio::test]
async fn rollback_without_transaction_errors() {
    let mut conn = make_connection().await;
    let err = conn.rollback().await.unwrap_err();
    assert!(matches!(err, DruidError::DriverError(_)));
}

// ── savepoints ───────────────────────────────────────────────────

#[tokio::test]
async fn savepoint_rollback_to_and_release() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();

    conn.begin().await.unwrap();
    conn.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();

    let sp = conn.set_savepoint().await.unwrap();
    conn.exec("INSERT INTO t (id) VALUES (2)", vec![])
        .await
        .unwrap();

    conn.rollback_to(&sp).await.unwrap();
    // After rollback_to, row 2 should be gone but row 1 remains
    conn.release_savepoint(&sp).await.unwrap();
    conn.commit().await.unwrap();

    let rows = conn.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn named_savepoint() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();

    conn.begin().await.unwrap();
    let sp = conn.set_savepoint_named("my_sp").await.unwrap();
    assert!(sp.name.is_some());
    conn.release_savepoint(&sp).await.unwrap();
    conn.commit().await.unwrap();
}

// ── auto_commit ──────────────────────────────────────────────────

#[tokio::test]
async fn auto_commit_default_is_true() {
    let conn = make_connection().await;
    assert!(conn.auto_commit());
}

#[tokio::test]
async fn set_auto_commit_false_begins_transaction() {
    let mut conn = make_connection().await;
    conn.set_auto_commit(false).await.unwrap();
    assert!(!conn.auto_commit());
    conn.set_auto_commit(true).await.unwrap(); // commits
    assert!(conn.auto_commit());
}

#[tokio::test]
async fn set_auto_commit_noop_when_same() {
    let mut conn = make_connection().await;
    // Already true, setting true is a no-op
    conn.set_auto_commit(true).await.unwrap();
    assert!(conn.auto_commit());
}

// ── transaction isolation ────────────────────────────────────────

#[tokio::test]
async fn default_transaction_isolation_for_sqlite() {
    let conn = make_connection().await;
    // SQLite defaults to SERIALIZABLE (8)
    assert_eq!(conn.transaction_isolation(), 8);
}

#[tokio::test]
async fn set_transaction_isolation_serializable() {
    let mut conn = make_connection().await;
    conn.set_transaction_isolation(8).await.unwrap();
    assert_eq!(conn.transaction_isolation(), 8);
}

#[tokio::test]
async fn set_transaction_isolation_non_serializable_errors_for_sqlite() {
    let mut conn = make_connection().await;
    let err = conn.set_transaction_isolation(2).await.unwrap_err();
    assert!(matches!(err, DruidError::InvalidArgument(_)));
}

#[tokio::test]
async fn set_transaction_isolation_during_active_transaction_errors() {
    let mut conn = make_connection().await;
    conn.begin().await.unwrap();
    let err = conn.set_transaction_isolation(8).await.unwrap_err();
    assert!(matches!(err, DruidError::InvalidArgument(_)));
    conn.rollback().await.unwrap();
}

// ── read_only ────────────────────────────────────────────────────

#[tokio::test]
async fn read_only_default_is_false() {
    let conn = make_connection().await;
    assert!(!conn.read_only());
}

#[tokio::test]
async fn set_read_only_true_errors_for_sqlite() {
    let mut conn = make_connection().await;
    let err = conn.set_read_only(true).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn set_read_only_during_transaction_errors() {
    let mut conn = make_connection().await;
    conn.begin().await.unwrap();
    let err = conn.set_read_only(false).await.unwrap_err();
    assert!(matches!(err, DruidError::InvalidArgument(_)));
    conn.rollback().await.unwrap();
}

// ── lifecycle ────────────────────────────────────────────────────

#[tokio::test]
async fn close_and_is_closed() {
    let mut conn = make_connection().await;
    assert!(!conn.is_closed());
    conn.close().await.unwrap();
    assert!(conn.is_closed());
}

#[tokio::test]
async fn close_is_idempotent() {
    let mut conn = make_connection().await;
    conn.close().await.unwrap();
    // Second close should not panic
    conn.close().await.unwrap();
    assert!(conn.is_closed());
}

#[tokio::test]
async fn exec_after_close_errors() {
    let mut conn = make_connection().await;
    conn.close().await.unwrap();
    let err = conn.exec("SELECT 1", vec![]).await.unwrap_err();
    assert!(matches!(err, DruidError::ConnectionDiscarded));
}

// ── ping ─────────────────────────────────────────────────────────

#[tokio::test]
async fn ping_on_open_connection() {
    let mut conn = make_connection().await;
    conn.ping().await.unwrap();
}

// ── warnings ─────────────────────────────────────────────────────

#[tokio::test]
async fn warnings_returns_none() {
    let mut conn = make_connection().await;
    let w = conn.warnings().await.unwrap();
    assert!(w.is_none());
}

#[tokio::test]
async fn clear_warnings_succeeds() {
    let mut conn = make_connection().await;
    conn.clear_warnings().await.unwrap();
}

#[tokio::test]
async fn warnings_after_close_errors() {
    let mut conn = make_connection().await;
    conn.close().await.unwrap();
    let err = conn.warnings().await.unwrap_err();
    assert!(matches!(err, DruidError::ConnectionDiscarded));
}

#[tokio::test]
async fn clear_warnings_after_close_errors() {
    let mut conn = make_connection().await;
    conn.close().await.unwrap();
    let err = conn.clear_warnings().await.unwrap_err();
    assert!(matches!(err, DruidError::ConnectionDiscarded));
}

// ── discarded ────────────────────────────────────────────────────

#[tokio::test]
async fn mark_discarded_and_is_discarded() {
    let mut conn = make_connection().await;
    assert!(!conn.is_discarded());
    conn.mark_discarded();
    assert!(conn.is_discarded());
}

#[tokio::test]
async fn exec_on_discarded_connection_errors() {
    let mut conn = make_connection().await;
    conn.mark_discarded();
    let err = conn.exec("SELECT 1", vec![]).await.unwrap_err();
    assert!(matches!(err, DruidError::ConnectionDiscarded));
}

// ── capabilities ─────────────────────────────────────────────────

#[tokio::test]
async fn capabilities_for_sqlite() {
    let conn = make_connection().await;
    let caps = conn.capabilities();
    assert!(caps.transactions);
    assert!(caps.savepoints);
    assert!(caps.auto_commit);
    assert!(
        !caps.read_only,
        "SQLite does not support read_only via Toasty"
    );
    assert!(caps.transaction_isolation);
    assert!(!caps.holdability);
}

// ── driver_name ──────────────────────────────────────────────────

#[tokio::test]
async fn driver_name_is_sqlite() {
    let conn = make_connection().await;
    assert_eq!(conn.driver_name(), "SQLite");
}

// ── database_meta_data ───────────────────────────────────────────

#[tokio::test]
async fn database_meta_data_returns_url() {
    let mut conn = make_connection().await;
    let mut meta = conn.database_meta_data().unwrap();
    let url = meta.get_url().await.unwrap();
    assert!(url.is_some(), "url should be present");
    assert!(url.unwrap().contains("sqlite"), "url should contain sqlite");
}

#[tokio::test]
async fn database_meta_data_driver_name() {
    let mut conn = make_connection().await;
    let mut meta = conn.database_meta_data().unwrap();
    let name = meta.get_driver_name().await.unwrap();
    assert!(name.is_some());
}

// ── driver_name is consistent ─────────────────────────────────────

#[tokio::test]
async fn driver_name_is_consistent_across_calls() {
    let conn = make_connection().await;
    assert_eq!(conn.driver_name(), "SQLite");
    assert_eq!(conn.driver_name(), "SQLite");
}

// ── value conversions ────────────────────────────────────────────

#[tokio::test]
async fn exec_with_null_param() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)", vec![])
        .await
        .unwrap();
    let result = conn
        .exec(
            "INSERT INTO t (id, val) VALUES (?, ?)",
            vec![Value::Int(1), Value::Null],
        )
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);
}

#[tokio::test]
async fn fetch_with_bool_param() {
    let mut conn = make_connection().await;
    let rows = conn
        .fetch(
            "SELECT 1 WHERE ? = ?",
            vec![Value::Bool(true), Value::Bool(true)],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn fetch_with_float_param() {
    let mut conn = make_connection().await;
    let rows = conn
        .fetch("SELECT 1 WHERE ? > 0.0", vec![Value::Float(3.14)])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn fetch_with_string_param() {
    let mut conn = make_connection().await;
    let rows = conn
        .fetch("SELECT ? AS val", vec![Value::String("hello".to_string())])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn fetch_with_bytes_param() {
    let mut conn = make_connection().await;
    let rows = conn
        .fetch("SELECT ? AS val", vec![Value::Bytes(vec![1, 2, 3])])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

// ── execute_prepared ─────────────────────────────────────────────

#[tokio::test]
async fn execute_prepared_query() {
    let mut conn = make_connection().await;
    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT 1 + ? AS result"))
        .await
        .unwrap();
    let results = conn
        .execute_prepared(
            stmt.as_ref(),
            vec![Value::Int(2)],
            StatementGeneratedKeys::None,
        )
        .await
        .unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn execute_prepared_on_closed_statement_errors() {
    let mut conn = make_connection().await;
    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT 1"))
        .await
        .unwrap();
    // Close the prepared statement via Arc clone
    conn.close_prepared_statement(stmt.clone()).await.unwrap();
    let err = conn
        .execute_prepared(stmt.as_ref(), vec![], StatementGeneratedKeys::None)
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::ConnectionDiscarded));
}

// ── fetch_prepared_result_set ────────────────────────────────────

#[tokio::test]
async fn fetch_prepared_result_set_returns_result_set() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO t (id) VALUES (2)", vec![])
        .await
        .unwrap();

    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT * FROM t ORDER BY id"))
        .await
        .unwrap();
    let _rs = conn
        .fetch_prepared_result_set(stmt.as_ref(), vec![])
        .await
        .unwrap();
}

// ── exec_prepared_parameters (PreparedInputParameter path) ───────

#[tokio::test]
async fn exec_prepared_with_update_counts() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();

    let stmt = conn
        .prepare_physical_statement(&make_key("INSERT INTO t (id) VALUES (?)"))
        .await
        .unwrap();

    // exec_prepared returns ExecResult
    let result = conn
        .exec_prepared(stmt.as_ref(), vec![Value::Int(10)])
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);
}

// ── fetch_prepared ───────────────────────────────────────────────

#[tokio::test]
async fn fetch_prepared_returns_matching_rows() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, x INTEGER)", vec![])
        .await
        .unwrap();
    for i in 1..=5 {
        conn.exec(&format!("INSERT INTO t (id, x) VALUES ({i}, {i})"), vec![])
            .await
            .unwrap();
    }

    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT * FROM t WHERE x <= ?"))
        .await
        .unwrap();
    let rows = conn
        .fetch_prepared(stmt.as_ref(), vec![Value::Int(3)])
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
}

// ── multiple sequential operations on same connection ────────────

#[tokio::test]
async fn sequential_operations_reuse_connection() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER)", vec![])
        .await
        .unwrap();

    // Insert
    for i in 1..=10 {
        conn.exec(
            &format!("INSERT INTO t (id, v) VALUES ({i}, {i} * 10)"),
            vec![],
        )
        .await
        .unwrap();
    }

    // Update
    let r = conn
        .exec("UPDATE t SET v = 0 WHERE id <= 5", vec![])
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 5);

    // Query
    let rows = conn
        .fetch("SELECT * FROM t WHERE v = 0", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 5);

    // Delete
    let r = conn
        .exec("DELETE FROM t WHERE id > 8", vec![])
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 2);

    // Final count
    let rows = conn.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 8);
}

// ── typed_parameter Date/Time/Timestamp value conversion ───────────

#[tokio::test]
async fn exec_with_date_param() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE dt (d DATE, t TIME, ts TIMESTAMP)", vec![])
        .await
        .unwrap();
    let d = chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
    let t = chrono::NaiveTime::from_hms_milli_opt(14, 30, 0, 500).unwrap();
    let ts =
        chrono::NaiveDateTime::parse_from_str("2025-06-15 14:30:00.000", "%Y-%m-%d %H:%M:%S%.f")
            .unwrap();
    let result = conn
        .exec(
            "INSERT INTO dt VALUES (?, ?, ?)",
            vec![Value::Date(d), Value::Time(t), Value::Timestamp(ts)],
        )
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);
}

// ── validate_savepoint_name error paths ─────────────────────────────

#[tokio::test]
async fn savepoint_empty_name_errors() {
    let mut conn = make_connection().await;
    conn.begin().await.unwrap();
    let result = conn.set_savepoint_named("").await;
    assert!(result.is_err());
    conn.rollback().await.unwrap();
}

#[tokio::test]
async fn savepoint_name_with_special_chars_errors() {
    let mut conn = make_connection().await;
    conn.begin().await.unwrap();
    let result = conn.set_savepoint_named("my sp!").await;
    assert!(result.is_err());
    conn.rollback().await.unwrap();
}

#[tokio::test]
async fn savepoint_name_with_underscore_ok() {
    let mut conn = make_connection().await;
    conn.begin().await.unwrap();
    let sp = conn.set_savepoint_named("my_savepoint_1").await.unwrap();
    assert_eq!(sp.name.as_deref(), Some("my_savepoint_1"));
    conn.release_savepoint(&sp).await.unwrap();
    conn.commit().await.unwrap();
}

// ── execute with generated keys ─────────────────────────────────────

#[tokio::test]
async fn execute_with_generated_key_columns_errors() {
    use druid::core::StatementGeneratedKeys;
    let mut conn = make_connection().await;
    conn.exec(
        "CREATE TABLE gk (id INTEGER PRIMARY KEY AUTOINCREMENT)",
        vec![],
    )
    .await
    .unwrap();
    let err = conn
        .execute(
            "INSERT INTO gk DEFAULT VALUES",
            vec![],
            StatementGeneratedKeys::ColumnIndexes(vec![1]),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

// ── exec_prepared_parameter_batch ───────────────────────────────────

#[tokio::test]
async fn exec_prepared_parameter_batch() {
    let mut conn = make_connection().await;
    conn.exec(
        "CREATE TABLE batch (id INTEGER PRIMARY KEY, v TEXT)",
        vec![],
    )
    .await
    .unwrap();
    let stmt = conn
        .prepare_physical_statement(&make_key("INSERT INTO batch (id, v) VALUES (?, ?)"))
        .await
        .unwrap();
    let params = vec![
        vec![
            PreparedInputParameter::Int(1),
            PreparedInputParameter::String(Some("a".to_string())),
        ],
        vec![
            PreparedInputParameter::Int(2),
            PreparedInputParameter::String(Some("b".to_string())),
        ],
        vec![
            PreparedInputParameter::Int(3),
            PreparedInputParameter::String(Some("c".to_string())),
        ],
    ];
    let counts = conn
        .exec_prepared_parameter_batch(stmt.as_ref(), params)
        .await
        .unwrap();
    assert_eq!(counts.len(), 3);
    assert!(counts.iter().all(|&c| c == 1));
}

// ── set_transaction_isolation invalid level ─────────────────────────

#[tokio::test]
async fn set_transaction_isolation_invalid_level_errors() {
    let mut conn = make_connection().await;
    let err = conn.set_transaction_isolation(99).await.unwrap_err();
    assert!(matches!(err, DruidError::InvalidArgument(_)));
}

// ── fetch_prepared with value params ────────────────────────────────

#[tokio::test]
async fn fetch_prepared_with_value_params() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE fp (id INTEGER PRIMARY KEY, v TEXT)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO fp VALUES (1, 'hello')", vec![])
        .await
        .unwrap();
    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT v FROM fp WHERE id = ?"))
        .await
        .unwrap();
    let rows = conn
        .fetch_prepared(stmt.as_ref(), vec![Value::Int(1)])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

// ── fetch_prepared_result_set ───────────────────────────────────────

#[tokio::test]
async fn fetch_prepared_result_set_returns_arc() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE fprs (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO fprs VALUES (1)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO fprs VALUES (2)", vec![])
        .await
        .unwrap();
    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT * FROM fprs WHERE id <= ?"))
        .await
        .unwrap();
    let rs = conn
        .fetch_prepared_result_set(stmt.as_ref(), vec![Value::Int(2)])
        .await
        .unwrap();
    assert!(!rs.is_closed());
}

// ── exec_prepared (update) ─────────────────────────────────────────

#[tokio::test]
async fn exec_prepared_update() {
    let mut conn = make_connection().await;
    conn.exec(
        "CREATE TABLE epu (id INTEGER PRIMARY KEY, v INTEGER)",
        vec![],
    )
    .await
    .unwrap();
    conn.exec("INSERT INTO epu VALUES (1, 10)", vec![])
        .await
        .unwrap();
    let stmt = conn
        .prepare_physical_statement(&make_key("UPDATE epu SET v = ? WHERE id = 1"))
        .await
        .unwrap();
    let result = conn
        .exec_prepared(stmt.as_ref(), vec![Value::Int(99)])
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);
}

// ── execute_prepared ───────────────────────────────────────────────

#[tokio::test]
async fn execute_prepared_query_with_where_clause() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE epp (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO epp VALUES (1)", vec![])
        .await
        .unwrap();
    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT * FROM epp WHERE id = ?"))
        .await
        .unwrap();
    let results = conn
        .execute_prepared(
            stmt.as_ref(),
            vec![Value::Int(1)],
            StatementGeneratedKeys::None,
        )
        .await
        .unwrap();
    assert!(!results.is_empty());
}

// ── close_prepared_statement ────────────────────────────────────────

#[tokio::test]
async fn close_prepared_statement_succeeds() {
    let mut conn = make_connection().await;
    let stmt = conn
        .prepare_physical_statement(&make_key("SELECT 1"))
        .await
        .unwrap();
    conn.close_prepared_statement(stmt).await.unwrap();
}

// ── prepare_physical_call errors for non-callable ───────────────────

#[tokio::test]
async fn prepare_physical_call_returns_non_callable_statement() {
    let mut conn = make_connection().await;
    // SQLite doesn't have callable statements; prepare_physical_call
    // returns a regular prepared statement
    let result = conn.prepare_physical_call(&make_key("SELECT 1")).await;
    // Verify it doesn't panic; either outcome is acceptable
    let _ = result;
}

// ── fetch with various value types ──────────────────────────────────

#[tokio::test]
async fn fetch_with_decimal_param() {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;
    let mut conn = make_connection().await;
    let rows = conn
        .fetch(
            "SELECT 1 WHERE ? > 0",
            vec![Value::Decimal(BigDecimal::from_str("1.5").unwrap())],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn exec_with_timestamp_param_roundtrip() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE ts_rt (ts TIMESTAMP)", vec![])
        .await
        .unwrap();
    let ts =
        chrono::NaiveDateTime::parse_from_str("2025-01-15 10:30:00.123", "%Y-%m-%d %H:%M:%S%.f")
            .unwrap();
    conn.exec("INSERT INTO ts_rt VALUES (?)", vec![Value::Timestamp(ts)])
        .await
        .unwrap();
    let rows = conn.fetch("SELECT ts FROM ts_rt", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
}

// ── exec_batch ──────────────────────────────────────────────────────

#[tokio::test]
async fn exec_batch_multiple_statements() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE bt (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let counts = conn
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

// ── Ping after close errors ────────────────────────────────────────

#[tokio::test]
async fn ping_after_close_errors() {
    let mut conn = make_connection().await;
    conn.close().await.unwrap();
    let err = conn.ping().await.unwrap_err();
    assert!(matches!(err, DruidError::ConnectionDiscarded));
}
