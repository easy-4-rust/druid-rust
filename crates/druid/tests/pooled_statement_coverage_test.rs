//! DruidPooledStatement coverage boost — batch, cancel, close, attribute,
//! generated keys, more_results, close_on_completion, poolable, and
//! property getter/setter families.

use druid::core::{
    DruidPooledConnection, FilterAdapter, FilterChainImpl, PhysicalConnectionFactory,
    ProxyAttributeValue,
};
use druid::toasty::ToastyConnectionFactory;
use std::sync::Arc;

// ── helpers ────────────────────────────────────────────────────────

async fn make_connection() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    DruidPooledConnection::new(physical, 1, Box::new(|_, _| {}))
}

async fn make_connection_with_chain() -> DruidPooledConnection {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    DruidPooledConnection::with_context(
        physical,
        2,
        "stmt-coverage".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    )
}

// ── batch lifecycle ────────────────────────────────────────────────

#[tokio::test]
async fn statement_batch_add_clear_execute() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.add_batch(&mut conn, "CREATE TABLE ps_batch(id INTEGER)")
        .unwrap();
    stmt.add_batch(&mut conn, "INSERT INTO ps_batch VALUES (1)")
        .unwrap();
    let counts = stmt.execute_batch(&mut conn).await.unwrap();
    assert_eq!(counts.len(), 2);

    // add then clear
    stmt.add_batch(&mut conn, "INSERT INTO ps_batch VALUES (2)")
        .unwrap();
    stmt.clear_batch(&mut conn).unwrap();

    stmt.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn statement_batch_through_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.add_batch(&mut conn, "CREATE TABLE ps_batch2(id INTEGER)")
        .unwrap();
    stmt.add_batch(&mut conn, "INSERT INTO ps_batch2 VALUES (1)")
        .unwrap();
    let counts = stmt.execute_batch(&mut conn).await.unwrap();
    assert_eq!(counts.len(), 2);

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── cancel ─────────────────────────────────────────────────────────

#[tokio::test]
async fn statement_cancel() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.cancel(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── close idempotency ──────────────────────────────────────────────

#[tokio::test]
async fn statement_close_idempotent() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
    // Second close should be a no-op
    stmt.close_with_connection(&mut conn).unwrap();
    assert!(stmt.is_closed());
}

// ── max_field_size / set_max_field_size ─────────────────────────────

#[tokio::test]
async fn statement_max_field_size_getter_setter() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    let default = stmt.max_field_size(&mut conn).unwrap();
    assert!(default >= 0);
    stmt.set_max_field_size(&mut conn, 1024).unwrap();
    let current = stmt.max_field_size(&mut conn).unwrap();
    assert_eq!(current, 1024);
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── max_rows / set_max_rows ────────────────────────────────────────

#[tokio::test]
async fn statement_max_rows_getter_setter() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    let default = stmt.max_rows(&mut conn).unwrap();
    assert_eq!(default, 0);
    stmt.set_max_rows(&mut conn, 100).unwrap();
    let current = stmt.max_rows(&mut conn).unwrap();
    assert_eq!(current, 100);
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── query_timeout / set_query_timeout ──────────────────────────────

#[tokio::test]
async fn statement_query_timeout_getter_setter() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    let default = stmt.query_timeout(&mut conn).unwrap();
    assert_eq!(default, 0);
    stmt.set_query_timeout(&mut conn, 30).unwrap();
    let current = stmt.query_timeout(&mut conn).unwrap();
    assert_eq!(current, 30);
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── set_escape_processing ──────────────────────────────────────────

#[tokio::test]
async fn statement_set_escape_processing() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.set_escape_processing(&mut conn, true).unwrap();
    stmt.set_escape_processing(&mut conn, false).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── fetch_direction / set_fetch_direction ──────────────────────────

#[tokio::test]
async fn statement_fetch_direction_getter_setter() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    let default = stmt.fetch_direction(&mut conn).unwrap();
    assert!(default >= 0);
    stmt.set_fetch_direction(&mut conn, 1000).unwrap();
    let current = stmt.fetch_direction(&mut conn).unwrap();
    assert_eq!(current, 1000);
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── fetch_size / set_fetch_size ────────────────────────────────────

#[tokio::test]
async fn statement_fetch_size_getter_setter() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    let default = stmt.fetch_size(&mut conn).unwrap();
    assert!(default >= 0);
    stmt.set_fetch_size(&mut conn, 50).unwrap();
    let current = stmt.fetch_size(&mut conn).unwrap();
    assert_eq!(current, 50);
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── set_cursor_name ────────────────────────────────────────────────

#[tokio::test]
async fn statement_set_cursor_name() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    // SQLite may not support cursor names, but the code path should be exercised
    let _ = stmt.set_cursor_name(&mut conn, "my_cursor");
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── poolable / set_poolable ────────────────────────────────────────

#[tokio::test]
async fn statement_poolable_getter_setter() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    // Regular statement is not poolable
    assert!(!stmt.is_poolable());
    let _ = stmt.set_poolable(&mut conn, true);
    let _ = stmt.set_poolable(&mut conn, false);
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── close_on_completion / is_close_on_completion ────────────────────

#[tokio::test]
async fn statement_close_on_completion() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    let _ = stmt.close_on_completion(&mut conn);
    let _ = stmt.is_close_on_completion(&mut conn);
    stmt.close_with_connection(&mut conn).unwrap();
}

// ── update_count ───────────────────────────────────────────────────

#[tokio::test]
async fn statement_update_count_after_execute() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.execute_update(&mut conn, "CREATE TABLE ps_uc(id INTEGER)")
        .await
        .unwrap();
    let result = stmt
        .execute_update(&mut conn, "INSERT INTO ps_uc VALUES (1)")
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);

    let count = stmt.update_count(&mut conn).unwrap();
    assert_eq!(count, 1);

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── result_set_type / result_set_concurrency / result_set_holdability ──

#[tokio::test]
async fn statement_result_set_properties() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    let _ = stmt.result_set_type(&mut conn).unwrap();
    let _ = stmt.result_set_concurrency(&mut conn).unwrap();
    let _ = stmt.result_set_holdability(&mut conn).unwrap();

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── generated_keys ─────────────────────────────────────────────────

#[tokio::test]
async fn statement_generated_keys() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.execute_update(
        &mut conn,
        "CREATE TABLE ps_gk(id INTEGER PRIMARY KEY, name TEXT)",
    )
    .await
    .unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO ps_gk(name) VALUES ('test')")
        .await
        .unwrap();

    let keys = stmt.generated_keys(&mut conn).unwrap();
    // SQLite should return the last insert rowid
    assert!(!keys.is_closed());

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── more_results / more_results_with_current ───────────────────────

#[tokio::test]
async fn statement_more_results() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.execute_update(&mut conn, "CREATE TABLE ps_mr(id INTEGER)")
        .await
        .unwrap();

    // more_results on a statement with no results
    let has_more = stmt.more_results(&mut conn).unwrap();
    assert!(!has_more);

    // more_results_with_current with valid constant
    let has_more = stmt.more_results_with_current(&mut conn, 1).unwrap();
    assert!(!has_more);

    let has_more = stmt.more_results_with_current(&mut conn, 2).unwrap();
    assert!(!has_more);

    let has_more = stmt.more_results_with_current(&mut conn, 3).unwrap();
    assert!(!has_more);

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── execute_with_generated_keys ────────────────────────────────────

#[tokio::test]
async fn statement_execute_with_generated_keys() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.execute_update(
        &mut conn,
        "CREATE TABLE ps_egk(id INTEGER PRIMARY KEY, name TEXT)",
    )
    .await
    .unwrap();

    // INSERT returns false (no ResultSet), but the code path is exercised
    let _result = stmt
        .execute_with_generated_keys(
            &mut conn,
            "INSERT INTO ps_egk(name) VALUES ('test')",
            1, // RETURN_GENERATED_KEYS
        )
        .await
        .unwrap();

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── execute_with_column_indexes / execute_with_column_names ─────────

#[tokio::test]
async fn statement_execute_with_column_indexes() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.execute_update(
        &mut conn,
        "CREATE TABLE ps_eci(id INTEGER PRIMARY KEY, name TEXT)",
    )
    .await
    .unwrap();

    // SQLite may not support column-based generated keys; exercise the code path
    let _ = stmt
        .execute_with_column_indexes(&mut conn, "INSERT INTO ps_eci(name) VALUES ('test')", &[1])
        .await;

    stmt.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn statement_execute_with_column_names() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.execute_update(
        &mut conn,
        "CREATE TABLE ps_ecn(id INTEGER PRIMARY KEY, name TEXT)",
    )
    .await
    .unwrap();

    // SQLite may not support column-based generated keys; exercise the code path
    let _ = stmt
        .execute_with_column_names(
            &mut conn,
            "INSERT INTO ps_ecn(name) VALUES ('test')",
            &["id".to_string()],
        )
        .await;

    stmt.close_with_connection(&mut conn).unwrap();
}

// -- result_set after execute -----------------------------------------------

#[tokio::test]
async fn statement_result_set_getter() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.execute_update(&mut conn, "CREATE TABLE ps_rs(id INTEGER)")
        .await
        .unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO ps_rs VALUES (1)")
        .await
        .unwrap();

    // execute generic
    let has_result = stmt
        .execute(&mut conn, "SELECT id FROM ps_rs")
        .await
        .unwrap();
    assert!(has_result);

    // get result set
    let rs = stmt.result_set(&mut conn).unwrap();
    assert!(rs.is_some());

    // get generated keys
    let keys = stmt.generated_keys(&mut conn).unwrap();
    assert!(!keys.is_closed());

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── attributes on statement ────────────────────────────────────────

#[tokio::test]
async fn statement_attributes() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();

    assert_eq!(stmt.attributes_size(), 0);
    assert!(stmt.attributes().is_empty());
    assert!(stmt.attribute("missing").is_none());

    stmt.put_attribute("key1", ProxyAttributeValue::new(42_i32));
    assert_eq!(stmt.attributes_size(), 1);
    let attr = stmt.attribute("key1").unwrap();
    let val: Arc<i32> = attr.downcast::<i32>().unwrap();
    assert_eq!(*val, 42);

    stmt.clear_attributes();
    assert_eq!(stmt.attributes_size(), 0);

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── warnings / clear_warnings through filter chain ─────────────────

#[tokio::test]
async fn statement_warnings_through_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    let mut stmt = conn.create_statement().await.unwrap();

    let warnings = stmt.warnings(&mut conn).await.unwrap();
    assert!(warnings.is_none());

    stmt.clear_warnings(&mut conn).await.unwrap();

    stmt.close_with_connection(&mut conn).unwrap();
}

// ── is_same_statement ──────────────────────────────────────────────

#[tokio::test]
async fn statement_identity() {
    let mut conn = make_connection().await;
    let stmt1 = conn.create_statement().await.unwrap();
    let stmt2 = conn.create_statement().await.unwrap();

    assert!(stmt1.is_same_statement(&stmt1));
    assert!(!stmt1.is_same_statement(&stmt2));
}

// ── id and statement accessor ──────────────────────────────────────

#[tokio::test]
async fn statement_id_and_accessor() {
    let mut conn = make_connection().await;
    let stmt = conn.create_statement().await.unwrap();

    assert!(stmt.id() > 0);
    let _ = stmt.statement();
    let _ = stmt.fetch_row_peak();
    assert_eq!(stmt.exception_count(), 0);
}

// ── Debug format ───────────────────────────────────────────────────

#[tokio::test]
async fn statement_debug_format() {
    let mut conn = make_connection().await;
    let stmt = conn.create_statement().await.unwrap();
    let debug = format!("{stmt:?}");
    assert!(debug.contains("DruidPooledStatement"));
}
