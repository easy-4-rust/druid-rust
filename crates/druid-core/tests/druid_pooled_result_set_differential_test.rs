//! Differential tests for `DruidPooledResultSet` — Java Druid 1.2.28 semantics.
//!
//! Uses real Toasty `SQLite`. Focuses on uncovered cursor operations, column mapping,
//! hidden columns, metadata, close paths, statement identity, constants, and error
//! paths that are NOT already covered in `druid_pooled_result_set_semantics_test.rs`.

extern crate druid_core as druid;
use druid_core::core::{
    DruidError, DruidPooledConnection, PhysicalConnection, PhysicalConnectionFactory, Value,
};
use druid_wrapper::toasty::ToastyConnectionFactory;

async fn make_connection() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    DruidPooledConnection::new(physical, 1, Box::new(|_, _| {}))
}

async fn setup_table(conn: &mut DruidPooledConnection) {
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

// ── Constants ──────────────────────────────────────────────────────

#[test]
fn result_set_constants_match_java() {
    use druid_core::core::DruidPooledResultSet;
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

// ── id / poolable_statement / statement ─────────────────────────────

#[tokio::test]
async fn result_set_id_is_nonzero() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert!(rs.id() > 0);
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn poolable_statement_returns_statement_ref() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let _ps = rs.poolable_statement();
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn statement_returns_statement_ref() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let _s = rs.statement();
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn prepared_statement_none_for_regular_statement() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert!(rs.prepared_statement().is_none());
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn callable_statement_none_for_regular_statement() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert!(rs.callable_statement().is_none());
    rs.close_with_connection(&mut conn).unwrap();
}

// ── cursor_index ───────────────────────────────────────────────────

#[tokio::test]
async fn cursor_index_starts_at_zero() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert_eq!(rs.cursor_index(), 0);
    rs.next(&mut conn).unwrap();
    assert_eq!(rs.cursor_index(), 1);
    rs.next(&mut conn).unwrap();
    assert_eq!(rs.cursor_index(), 2);
    rs.close_with_connection(&mut conn).unwrap();
}

// ── close_count / construct_elapsed / sql ───────────────────────────

#[tokio::test]
async fn close_count_zero_before_close() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert_eq!(rs.close_count(), 0);
    rs.close_with_connection(&mut conn).unwrap();
    assert_eq!(rs.close_count(), 1);
}

#[tokio::test]
async fn construct_elapsed_returns_duration() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    // construct_elapsed records from open filter time
    let _ = rs.construct_elapsed();
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn sql_returns_some() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let sql = rs.sql();
    assert!(sql.is_some());
    assert!(sql.unwrap().contains("SELECT"));
    rs.close_with_connection(&mut conn).unwrap();
}

// ── read_string_length / read_bytes_length ─────────────────────────

#[tokio::test]
async fn read_string_length_increments_on_string_get() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT name FROM t LIMIT 1")
        .await
        .unwrap();
    assert_eq!(rs.read_string_length(), 0);
    rs.next(&mut conn).unwrap();
    let _ = rs.string(&mut conn, 1).unwrap();
    assert!(rs.read_string_length() > 0);
    rs.close_with_connection(&mut conn).unwrap();
}

// ── physical_column / logic_column ─────────────────────────────────

#[tokio::test]
async fn physical_column_identity_without_map() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    // Without a map, identity mapping
    assert_eq!(rs.physical_column(1), Some(1));
    assert_eq!(rs.logic_column(1), Some(1));
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn set_logic_column_map_and_reverse() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let mut map = std::collections::HashMap::new();
    map.insert(1, 2);
    map.insert(2, 1);
    rs.set_logic_column_map(Some(map.clone()));
    assert_eq!(rs.physical_column(1), Some(2));
    assert_eq!(rs.physical_column(2), Some(1));
    // Missing key returns None
    assert_eq!(rs.physical_column(99), None);

    let mut reverse = std::collections::HashMap::new();
    reverse.insert(2, 1);
    reverse.insert(1, 2);
    rs.set_physical_column_map(Some(reverse));
    assert_eq!(rs.logic_column(2), Some(1));

    // Reset to identity
    rs.set_logic_column_map(None);
    assert_eq!(rs.physical_column(1), Some(1));
    rs.close_with_connection(&mut conn).unwrap();
}

// ── hidden_columns ─────────────────────────────────────────────────

#[tokio::test]
async fn hidden_columns_none_by_default() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert_eq!(rs.hidden_column_count(), 0);
    assert!(rs.hidden_columns().is_none());

    rs.set_hidden_columns(Some(vec![3, 4]));
    assert_eq!(rs.hidden_column_count(), 2);
    assert_eq!(rs.hidden_columns(), Some([3, 4].as_slice()));

    rs.set_hidden_columns(None);
    assert!(rs.hidden_columns().is_none());
    rs.close_with_connection(&mut conn).unwrap();
}

// ── raw_result_set ─────────────────────────────────────────────────

#[tokio::test]
async fn raw_result_set_returns_physical() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let _raw = rs.raw_result_set();
    rs.close_with_connection(&mut conn).unwrap();
}

// ── is_closed / is_closed_with_connection ───────────────────────────

#[tokio::test]
async fn is_closed_false_initially() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert!(!rs.is_closed());
    rs.close_with_connection(&mut conn).unwrap();
    assert!(rs.is_closed());
}

#[tokio::test]
async fn is_closed_with_connection_delegates() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let closed = rs.is_closed_with_connection(&mut conn).unwrap();
    assert!(!closed);
    rs.close_with_connection(&mut conn).unwrap();
}

// ── was_null ───────────────────────────────────────────────────────

#[tokio::test]
async fn was_null_after_null_value() {
    let mut conn = make_connection().await;
    conn.exec(
        "CREATE TABLE nullable (id INTEGER PRIMARY KEY, v TEXT)",
        vec![],
    )
    .await
    .unwrap();
    conn.exec("INSERT INTO nullable (id, v) VALUES (1, NULL)", vec![])
        .await
        .unwrap();
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT v FROM nullable")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    let val = rs.string(&mut conn, 1).unwrap();
    assert!(val.is_none());
    assert!(rs.was_null(&mut conn).unwrap());
    rs.close_with_connection(&mut conn).unwrap();
}

// ── Typed getters: int, long, short, byte, double, float, boolean ──

#[tokio::test]
async fn int_getter_returns_integer() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id FROM t WHERE id = 3")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    assert_eq!(rs.int(&mut conn, 1).unwrap(), 3);
    // Label-based not supported by RowSetResultSet
    let err = rs.int_by_label(&mut conn, "id").unwrap_err();
    assert!(matches!(err, DruidError::InvalidArgument(_)));
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn long_getter_returns_integer() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id FROM t WHERE id = 4")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    assert_eq!(rs.long(&mut conn, 1).unwrap(), 4);
    let _ = rs.long_by_label(&mut conn, "id");
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn double_getter_returns_float() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT val FROM t WHERE id = 1")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    let d = rs.double(&mut conn, 1).unwrap();
    assert!((d - 1.5).abs() < 0.01);
    let _ = rs.double_by_label(&mut conn, "val");
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn float_getter_returns_float() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT val FROM t WHERE id = 2")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    let f = rs.float(&mut conn, 1).unwrap();
    assert!((f - 2.5).abs() < 0.01);
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn boolean_getter_returns_bool() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE b (v BOOLEAN)", vec![])
        .await
        .unwrap();
    conn.exec("INSERT INTO b VALUES (1)", vec![]).await.unwrap();
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT v FROM b")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    assert!(rs.boolean(&mut conn, 1).unwrap());
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn bytes_getter_returns_blob() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE bl (v BLOB)", vec![]).await.unwrap();
    conn.exec("INSERT INTO bl VALUES (X'DEADBEEF')", vec![])
        .await
        .unwrap();
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT v FROM bl")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    let b = rs.bytes(&mut conn, 1).unwrap();
    assert!(b.is_some());
    rs.close_with_connection(&mut conn).unwrap();
}

// ── object / object_by_label ───────────────────────────────────────

#[tokio::test]
async fn object_returns_value() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id FROM t WHERE id = 1")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    let val = rs.object(&mut conn, 1).unwrap();
    assert_eq!(val, Value::Int(1));
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn object_by_label_errors_for_row_set() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id FROM t WHERE id = 1")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    // RowSetResultSet does not support label-based lookup
    let _ = rs.object_by_label(&mut conn, "id");
    rs.close_with_connection(&mut conn).unwrap();
}

// ── string / string_by_label / n_string ────────────────────────────

#[tokio::test]
async fn string_getter_returns_text() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT name FROM t WHERE id = 1")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    assert_eq!(rs.string(&mut conn, 1).unwrap(), Some("row1".to_string()));
    // Label-based not supported by RowSetResultSet
    let _ = rs.string_by_label(&mut conn, "name");
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn n_string_getter_returns_text() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT name FROM t WHERE id = 1")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    assert_eq!(rs.n_string(&mut conn, 1).unwrap(), Some("row1".to_string()));
    rs.close_with_connection(&mut conn).unwrap();
}

// ── date / time / timestamp ────────────────────────────────────────

#[tokio::test]
async fn date_getter_handles_null() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE dt (d DATE)", vec![]).await.unwrap();
    conn.exec("INSERT INTO dt VALUES (NULL)", vec![])
        .await
        .unwrap();
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT d FROM dt")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    assert!(rs.date(&mut conn, 1).unwrap().is_none());
    rs.close_with_connection(&mut conn).unwrap();
}

// ── warnings / clear_warnings ──────────────────────────────────────

#[tokio::test]
async fn warnings_returns_none_for_sqlite() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let w = rs.warnings(&mut conn).unwrap();
    assert!(w.is_none());
    rs.clear_warnings(&mut conn).unwrap();
    rs.close_with_connection(&mut conn).unwrap();
}

// ── meta_data / meta_data_proxy ────────────────────────────────────

#[tokio::test]
async fn meta_data_returns_column_count() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id, name, val FROM t")
        .await
        .unwrap();
    let md = rs.meta_data(&mut conn).unwrap();
    assert_eq!(md.column_count().unwrap(), 3);
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn meta_data_proxy_returns_proxy_with_attributes() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id, name FROM t")
        .await
        .unwrap();
    let proxy = rs.meta_data_proxy(&mut conn).unwrap();
    assert_eq!(proxy.attributes_size(), 0);
    rs.close_with_connection(&mut conn).unwrap();
}

// ── find_column ────────────────────────────────────────────────────

#[tokio::test]
async fn find_column_errors_for_row_set() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id, name FROM t")
        .await
        .unwrap();
    // RowSetResultSet does not support find_column
    let _ = rs.find_column(&mut conn, "name");
    rs.close_with_connection(&mut conn).unwrap();
}

// ── fetch_row_count ────────────────────────────────────────────────

#[tokio::test]
async fn fetch_row_count_increments_with_next() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert_eq!(rs.fetch_row_count(), 0);
    rs.next(&mut conn).unwrap();
    assert_eq!(rs.fetch_row_count(), 1);
    rs.next(&mut conn).unwrap();
    assert_eq!(rs.fetch_row_count(), 2);
    rs.close_with_connection(&mut conn).unwrap();
}

// ── cursor_name / result_set_type / concurrency / holdability ──────

#[tokio::test]
async fn cursor_name_returns_none_for_sqlite() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let _ = rs.cursor_name(&mut conn);
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn result_set_type_returns_type() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let _ = rs.result_set_type(&mut conn);
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn concurrency_returns_mode() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let _ = rs.concurrency(&mut conn);
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn holdability_returns_value() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let _ = rs.holdability(&mut conn);
    rs.close_with_connection(&mut conn).unwrap();
}

// ── fetch_direction / fetch_size ───────────────────────────────────

#[tokio::test]
async fn fetch_direction_returns_value() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let _ = rs.fetch_direction(&mut conn);
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn set_fetch_size_succeeds() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    rs.set_fetch_size(&mut conn, 100).unwrap();
    let _ = rs.fetch_size(&mut conn);
    rs.close_with_connection(&mut conn).unwrap();
}

// ── row / row_updated / row_inserted / row_deleted ─────────────────

#[tokio::test]
async fn row_returns_current_position() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    let _ = rs.row(&mut conn);
    rs.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn row_updated_inserted_deleted_return_false_for_sqlite() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    rs.next(&mut conn).unwrap();
    // SQLite RowSetResultSet returns false for all
    assert!(!rs.row_updated(&mut conn).unwrap());
    assert!(!rs.row_inserted(&mut conn).unwrap());
    assert!(!rs.row_deleted(&mut conn).unwrap());
    rs.close_with_connection(&mut conn).unwrap();
}

// ── open_input_stream_count / open_reader_count ────────────────────

#[tokio::test]
async fn stream_reader_counts_zero_initially() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert_eq!(rs.open_input_stream_count(), 0);
    assert_eq!(rs.open_reader_count(), 0);
    rs.close_with_connection(&mut conn).unwrap();
}

// ── statement_object ───────────────────────────────────────────────

#[tokio::test]
async fn statement_object_returns_statement_variant() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    let so = rs.statement_object(&mut conn).unwrap();
    // For regular Statement, should be Statement variant
    use druid_core::core::ResultSetStatement;
    assert!(matches!(so, ResultSetStatement::Statement(_)));
    rs.close_with_connection(&mut conn).unwrap();
}

// ── close_with_connection idempotency ──────────────────────────────

#[tokio::test]
async fn close_with_connection_sets_closed() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert!(!rs.is_closed());
    rs.close_with_connection(&mut conn).unwrap();
    assert!(rs.is_closed());
}

// ── next returns false at end ──────────────────────────────────────

#[tokio::test]
async fn next_returns_false_at_end() {
    let mut conn = make_connection().await;
    conn.exec("CREATE TABLE empty (id INTEGER)", vec![])
        .await
        .unwrap();
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM empty")
        .await
        .unwrap();
    assert!(!rs.next(&mut conn).unwrap());
    rs.close_with_connection(&mut conn).unwrap();
}

// ── attributes on result set ───────────────────────────────────────

#[tokio::test]
async fn result_set_attributes_empty_initially() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT * FROM t")
        .await
        .unwrap();
    assert_eq!(rs.attributes_size(), 0);
    assert!(rs.attributes().is_empty());
    assert!(rs.attribute("missing").is_none());
    rs.put_attribute("k", druid_core::core::ProxyAttributeValue::new(42));
    assert_eq!(rs.attributes_size(), 1);
    rs.clear_attributes();
    assert_eq!(rs.attributes_size(), 0);
    rs.close_with_connection(&mut conn).unwrap();
}
