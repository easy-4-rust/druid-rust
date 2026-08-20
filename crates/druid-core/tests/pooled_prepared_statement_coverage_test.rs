//! DruidPooledPreparedStatement coverage boost — parameter binding, batch,
//! property getter/setter, wrapper trait, close paths, execute variants,
//! generated keys, more_results, warnings, and Debug format.

extern crate druid_core as druid;
use druid::core::{
    DruidPooledConnection, FilterAdapter, FilterChainImpl, PhysicalConnectionFactory,
    ProxyAttributeValue, Value, Wrapper,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
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
        "ps-coverage".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    )
}

async fn setup_table(conn: &mut DruidPooledConnection) {
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(
        conn,
        "CREATE TABLE IF NOT EXISTS ps_cov(id INTEGER PRIMARY KEY, name TEXT, val REAL)",
    )
    .await
    .unwrap();
    stmt.close_with_connection(conn).unwrap();
}

// ── parameter binding coverage ─────────────────────────────────────

#[tokio::test]
async fn ps_parameter_binding_all_types() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn
        .prepare_statement("INSERT INTO ps_cov(id, name, val) VALUES (?1, ?2, ?3)")
        .await
        .unwrap();

    // set_int
    ps.set_int(&mut conn, 1, 42).unwrap();
    // set_n_string
    ps.set_n_string(&mut conn, 2, Some("hello".to_string()))
        .unwrap();
    // set_double
    ps.set_double(&mut conn, 3, 3.14).unwrap();
    // set_boolean
    ps.set_boolean(&mut conn, 1, true).unwrap();
    // set_byte
    ps.set_byte(&mut conn, 1, 1).unwrap();
    // set_short
    ps.set_short(&mut conn, 1, 1).unwrap();
    // set_long
    ps.set_long(&mut conn, 1, 1).unwrap();
    // set_float
    ps.set_float(&mut conn, 3, 1.5).unwrap();
    // set_string
    ps.set_string(&mut conn, 2, Some("world".to_string()))
        .unwrap();
    // set_bytes
    ps.set_bytes(&mut conn, 2, Some(vec![1, 2, 3])).unwrap();
    // set_null
    ps.set_null(&mut conn, 2, 12).unwrap();
    // set_null_with_type_name
    ps.set_null_with_type_name(&mut conn, 2, 12, None).unwrap();
    // set_object
    ps.set_object(&mut conn, 1, None).unwrap();
    // set_object_with_sql_type
    ps.set_object_with_sql_type(&mut conn, 1, None, 4).unwrap();
    // set_object_with_sql_type_and_scale
    ps.set_object_with_sql_type_and_scale(&mut conn, 1, None, 4, 0)
        .unwrap();

    // Temporal bindings
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let time = NaiveTime::from_hms_opt(10, 30, 0).unwrap();
    let dt = NaiveDateTime::new(date, time);

    ps.set_date(&mut conn, 2, Some(date)).unwrap();
    ps.set_date_with_calendar(&mut conn, 2, Some(date), None)
        .unwrap();
    ps.set_time(&mut conn, 2, Some(time)).unwrap();
    ps.set_time_with_calendar(&mut conn, 2, Some(time), None)
        .unwrap();
    ps.set_timestamp(&mut conn, 2, Some(dt)).unwrap();
    ps.set_timestamp_with_calendar(&mut conn, 2, Some(dt), None)
        .unwrap();

    // Stream bindings
    ps.set_ascii_stream(&mut conn, 2, None).unwrap();
    ps.set_ascii_stream_with_int_length(&mut conn, 2, None, 100)
        .unwrap();
    ps.set_ascii_stream_with_long_length(&mut conn, 2, None, 100)
        .unwrap();
    ps.set_binary_stream(&mut conn, 2, None).unwrap();
    ps.set_binary_stream_with_int_length(&mut conn, 2, None, 100)
        .unwrap();
    ps.set_binary_stream_with_long_length(&mut conn, 2, None, 100)
        .unwrap();
    ps.set_unicode_stream(&mut conn, 2, None, 100).unwrap();

    // Character stream bindings
    ps.set_character_stream(&mut conn, 2, None).unwrap();
    ps.set_character_stream_with_int_length(&mut conn, 2, None, 100)
        .unwrap();
    ps.set_character_stream_with_long_length(&mut conn, 2, None, 100)
        .unwrap();
    ps.set_n_character_stream(&mut conn, 2, None).unwrap();
    ps.set_n_character_stream_with_long_length(&mut conn, 2, None, 100)
        .unwrap();

    // Blob/Clob reader bindings
    ps.set_blob_stream(&mut conn, 2, None).unwrap();
    ps.set_blob_stream_with_long_length(&mut conn, 2, None, 100)
        .unwrap();
    ps.set_clob_reader(&mut conn, 2, None).unwrap();
    ps.set_clob_reader_with_long_length(&mut conn, 2, None, 100)
        .unwrap();
    ps.set_n_clob_reader(&mut conn, 2, None).unwrap();
    ps.set_n_clob_reader_with_long_length(&mut conn, 2, None, 100)
        .unwrap();

    // clear_parameters
    ps.clear_parameters(&mut conn).unwrap();

    ps.close_with_connection(&mut conn).unwrap();
}

// ── parameter slot count and rdbc_parameters ───────────────────────

#[tokio::test]
async fn ps_parameter_slot_count_and_rdbc_parameters() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn
        .prepare_statement("INSERT INTO ps_cov(id, name) VALUES (?1, ?2)")
        .await
        .unwrap();

    assert_eq!(ps.parameter_slot_count(), 0);

    ps.set_int(&mut conn, 1, 1).unwrap();
    assert_eq!(ps.parameter_slot_count(), 1);

    ps.set_n_string(&mut conn, 2, Some("test".to_string()))
        .unwrap();
    assert_eq!(ps.parameter_slot_count(), 2);

    // rdbc_parameters
    let params = ps.rdbc_parameters();
    assert_eq!(params.len(), 2);

    // rdbc_parameter
    let p0 = ps.rdbc_parameter(0);
    assert!(p0.is_some());
    let p1 = ps.rdbc_parameter(1);
    assert!(p1.is_some());
    let p2 = ps.rdbc_parameter(2);
    assert!(p2.is_none());

    // parameter (1-based)
    let p1 = ps.parameter(1);
    assert!(p1.is_some());
    let p_missing = ps.parameter(99);
    assert!(p_missing.is_none());

    ps.close_with_connection(&mut conn).unwrap();
}

// ── execute_update_bound / execute_query_bound / execute_bound ──────

#[tokio::test]
async fn ps_execute_bound_paths() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    // Insert
    let mut ps = conn
        .prepare_statement("INSERT INTO ps_cov(id, name) VALUES (?1, ?2)")
        .await
        .unwrap();
    ps.set_int(&mut conn, 1, 1).unwrap();
    ps.set_n_string(&mut conn, 2, Some("test".to_string()))
        .unwrap();
    let result = ps.execute_update_bound(&mut conn).await.unwrap();
    assert_eq!(result.rows_affected, 1);
    ps.close_with_connection(&mut conn).unwrap();

    // Query
    let mut ps2 = conn
        .prepare_statement("SELECT id, name FROM ps_cov WHERE id = ?1")
        .await
        .unwrap();
    ps2.set_int(&mut conn, 1, 1).unwrap();
    let mut rs = ps2.execute_query_bound(&mut conn).await.unwrap();
    assert!(rs.next(&mut conn).unwrap());
    let id = rs.int(&mut conn, 1).unwrap();
    assert_eq!(id, 1);
    rs.close_with_connection(&mut conn).unwrap();
    ps2.close_with_connection(&mut conn).unwrap();

    // Generic execute
    let mut ps3 = conn.prepare_statement("SELECT 1").await.unwrap();
    let is_result_set = ps3.execute_bound(&mut conn).await.unwrap();
    assert!(is_result_set);
    ps3.close_with_connection(&mut conn).unwrap();
}

// ── fetch_bound (eager rows) ───────────────────────────────────────

#[tokio::test]
async fn ps_fetch_bound_eager_rows() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO ps_cov(id, name) VALUES (10, 'a')")
        .await
        .unwrap();
    stmt.close_with_connection(&mut conn).unwrap();

    let mut ps = conn
        .prepare_statement("SELECT id, name FROM ps_cov WHERE id = ?1")
        .await
        .unwrap();
    ps.set_int(&mut conn, 1, 10).unwrap();
    let rows = ps.fetch_bound(&mut conn).await.unwrap();
    assert_eq!(rows.len(), 1);
    ps.close_with_connection(&mut conn).unwrap();
}

// ── batch lifecycle ────────────────────────────────────────────────

#[tokio::test]
async fn ps_batch_add_execute_clear() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn
        .prepare_statement("INSERT INTO ps_cov(id, name) VALUES (?1, ?2)")
        .await
        .unwrap();

    // add_batch with params
    ps.add_batch(
        &mut conn,
        vec![Value::Int(1), Value::String("a".to_string())],
    )
    .unwrap();
    ps.add_batch(
        &mut conn,
        vec![Value::Int(2), Value::String("b".to_string())],
    )
    .unwrap();
    assert_eq!(ps.batch_size(), 2);

    let counts = ps.execute_batch(&mut conn).await.unwrap();
    assert_eq!(counts.len(), 2);

    // add_bound_batch
    ps.set_int(&mut conn, 1, 3).unwrap();
    ps.set_n_string(&mut conn, 2, Some("c".to_string()))
        .unwrap();
    ps.add_bound_batch(&mut conn).unwrap();
    assert_eq!(ps.batch_size(), 1);

    // clear_batch
    ps.clear_batch(&mut conn).unwrap();
    assert_eq!(ps.batch_size(), 0);

    ps.close_with_connection(&mut conn).unwrap();
}

// ── property getter/setter families ────────────────────────────────

#[tokio::test]
async fn ps_property_getter_setter_families() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn.prepare_statement("SELECT 1").await.unwrap();

    // result_set_type / result_set_concurrency / result_set_holdability
    let _ = ps.result_set_type(&mut conn).unwrap();
    let _ = ps.result_set_concurrency(&mut conn).unwrap();
    let _ = ps.result_set_holdability(&mut conn).unwrap();

    // max_field_size / set_max_field_size
    let default_mfs = ps.max_field_size(&mut conn).unwrap();
    assert!(default_mfs >= 0);
    ps.set_max_field_size(&mut conn, 2048).unwrap();
    let current_mfs = ps.max_field_size(&mut conn).unwrap();
    assert_eq!(current_mfs, 2048);

    // max_rows / set_max_rows
    let default_mr = ps.max_rows(&mut conn).unwrap();
    assert_eq!(default_mr, 0);
    ps.set_max_rows(&mut conn, 500).unwrap();
    let current_mr = ps.max_rows(&mut conn).unwrap();
    assert_eq!(current_mr, 500);

    // query_timeout / set_query_timeout
    let default_qt = ps.query_timeout(&mut conn).unwrap();
    assert_eq!(default_qt, 0);
    ps.set_query_timeout(&mut conn, 60).unwrap();
    let current_qt = ps.query_timeout(&mut conn).unwrap();
    assert_eq!(current_qt, 60);

    // set_escape_processing
    ps.set_escape_processing(&mut conn, true).unwrap();
    ps.set_escape_processing(&mut conn, false).unwrap();

    // fetch_direction / set_fetch_direction
    let _ = ps.fetch_direction(&mut conn).unwrap();
    ps.set_fetch_direction(&mut conn, 1000).unwrap();
    let fd = ps.fetch_direction(&mut conn).unwrap();
    assert_eq!(fd, 1000);

    // fetch_size / set_fetch_size
    let _ = ps.fetch_size(&mut conn).unwrap();
    ps.set_fetch_size(&mut conn, 25).unwrap();
    let fs = ps.fetch_size(&mut conn).unwrap();
    assert_eq!(fs, 25);

    // set_cursor_name
    let _ = ps.set_cursor_name(&mut conn, "ps_cursor");

    // cancel
    ps.cancel(&mut conn).unwrap();

    // poolable / set_poolable
    assert!(!ps.is_poolable());
    let _ = ps.set_poolable(&mut conn, true);
    let _ = ps.set_poolable(&mut conn, false);

    // close_on_completion / is_close_on_completion
    let _ = ps.close_on_completion(&mut conn);
    let _ = ps.is_close_on_completion(&mut conn);

    ps.close_with_connection(&mut conn).unwrap();
}

// ── generated_keys / more_results ──────────────────────────────────

#[tokio::test]
async fn ps_generated_keys_and_more_results() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(
        &mut conn,
        "CREATE TABLE IF NOT EXISTS ps_gk(id INTEGER PRIMARY KEY, name TEXT)",
    )
    .await
    .unwrap();
    stmt.close_with_connection(&mut conn).unwrap();

    let mut ps = conn
        .prepare_statement("INSERT INTO ps_gk(name) VALUES (?1)")
        .await
        .unwrap();
    ps.set_n_string(&mut conn, 1, Some("test".to_string()))
        .unwrap();
    let _ = ps.execute_update_bound(&mut conn).await.unwrap();

    // generated_keys
    let keys = ps.generated_keys(&mut conn).unwrap();
    assert!(!keys.is_closed());

    // more_results
    let has_more = ps.more_results(&mut conn).unwrap();
    assert!(!has_more);

    // more_results_with_current
    let has_more = ps.more_results_with_current(&mut conn, 1).unwrap();
    assert!(!has_more);
    let has_more = ps.more_results_with_current(&mut conn, 2).unwrap();
    assert!(!has_more);
    let has_more = ps.more_results_with_current(&mut conn, 3).unwrap();
    assert!(!has_more);

    // update_count -- after more_results, update_count resets to -1
    let count = ps.update_count(&mut conn).unwrap();
    assert_eq!(count, -1);

    ps.close_with_connection(&mut conn).unwrap();
}

// ── warnings / clear_warnings ──────────────────────────────────────

#[tokio::test]
async fn ps_warnings_through_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    setup_table(&mut conn).await;

    let mut ps = conn.prepare_statement("SELECT 1").await.unwrap();

    let warnings = ps.warnings(&mut conn).await.unwrap();
    assert!(warnings.is_none());

    ps.clear_warnings(&mut conn).await.unwrap();

    ps.close_with_connection(&mut conn).unwrap();
}

// ── close paths ────────────────────────────────────────────────────

#[tokio::test]
async fn ps_close_idempotent() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn.prepare_statement("SELECT 1").await.unwrap();
    ps.close_with_connection(&mut conn).unwrap();
    assert!(ps.is_closed());
    // Second close should be no-op
    ps.close_with_connection(&mut conn).unwrap();
}

#[tokio::test]
async fn ps_close_without_connection() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn.prepare_statement("SELECT 1").await.unwrap();
    // close() without connection context
    ps.close().unwrap();
    assert!(ps.is_closed());
}

// ── id and identity ────────────────────────────────────────────────

#[tokio::test]
async fn ps_id_and_identity() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let ps1 = conn.prepare_statement("SELECT 1").await.unwrap();
    let ps2 = conn.prepare_statement("SELECT 2").await.unwrap();

    assert!(ps1.id() > 0);
    assert!(ps2.id() > 0);
    // Different prepared statements have different identities
    assert_ne!(ps1.id(), ps2.id());
}

// ── Wrapper trait ──────────────────────────────────────────────────

#[tokio::test]
async fn ps_wrapper_trait() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let ps = conn.prepare_statement("SELECT 1").await.unwrap();

    // as_any
    let any_ref = Wrapper::as_any(&ps);
    assert_eq!(
        any_ref.type_id(),
        TypeId::of::<druid::core::DruidPooledPreparedStatement>()
    );

    // is_wrapper_for
    assert!(Wrapper::is_wrapper_for(
        &ps,
        Some(TypeId::of::<druid::core::DruidPooledPreparedStatement>())
    ));
    assert!(Wrapper::is_wrapper_for(
        &ps,
        Some(TypeId::of::<dyn druid::core::PhysicalPreparedStatement>())
    ));
    assert!(!Wrapper::is_wrapper_for(&ps, None));
    assert!(!Wrapper::is_wrapper_for(&ps, Some(TypeId::of::<String>())));

    // unwrap
    assert!(Wrapper::unwrap(
        &ps,
        Some(TypeId::of::<druid::core::DruidPooledPreparedStatement>())
    )
    .is_some());
    assert!(Wrapper::unwrap(
        &ps,
        Some(TypeId::of::<dyn druid::core::PhysicalPreparedStatement>())
    )
    .is_some());
    assert!(Wrapper::unwrap(&ps, None).is_none());
}

// ── Debug format ───────────────────────────────────────────────────

#[tokio::test]
async fn ps_debug_format() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let ps = conn.prepare_statement("SELECT 1").await.unwrap();
    let debug = format!("{ps:?}");
    assert!(debug.contains("DruidPooledPreparedStatement"));
}

// -- attributes via pooled_statement() ------------------------------------

#[tokio::test]
async fn ps_attributes_via_base() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let ps = conn.prepare_statement("SELECT 1").await.unwrap();
    let base = ps.pooled_statement();
    assert_eq!(base.attributes_size(), 0);
    base.put_attribute("key", ProxyAttributeValue::new(42_i32));
    assert_eq!(base.attributes_size(), 1);
    base.clear_attributes();
    assert_eq!(base.attributes_size(), 0);
}

// ── execute_query_bound ────────────────────────────────────────

#[tokio::test]
async fn ps_execute_query_bound() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO ps_cov(id, name) VALUES (100, 'rs')")
        .await
        .unwrap();
    stmt.close_with_connection(&mut conn).unwrap();

    let mut ps = conn
        .prepare_statement("SELECT id, name FROM ps_cov WHERE id = ?1")
        .await
        .unwrap();
    ps.set_int(&mut conn, 1, 100).unwrap();

    let mut rs = ps.execute_query_bound(&mut conn).await.unwrap();
    assert!(rs.next(&mut conn).unwrap());
    let id = rs.int(&mut conn, 1).unwrap();
    assert_eq!(id, 100);
    let name = rs.n_string(&mut conn, 2).unwrap();
    assert_eq!(name.as_deref(), Some("rs"));

    // statement() on result set should return the prepared statement handle
    let stmt_ref = rs.statement();
    let _ = stmt_ref.id();

    rs.close_with_connection(&mut conn).unwrap();
    ps.close_with_connection(&mut conn).unwrap();
}

// ── execute (generic) ──────────────────────────────────────────────

#[tokio::test]
async fn ps_execute_generic() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn.prepare_statement("SELECT 1").await.unwrap();
    let is_result_set = ps.execute(&mut conn, vec![]).await.unwrap();
    assert!(is_result_set);

    // result_set after execute
    let rs = ps.result_set(&mut conn).unwrap();
    assert!(rs.is_some());

    ps.close_with_connection(&mut conn).unwrap();
}

// ── fetch (eager rows) ─────────────────────────────────────────────

#[tokio::test]
async fn ps_fetch_eager_rows() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(
        &mut conn,
        "INSERT INTO ps_cov(id, name) VALUES (200, 'fetch')",
    )
    .await
    .unwrap();
    stmt.close_with_connection(&mut conn).unwrap();

    let mut ps = conn
        .prepare_statement("SELECT id, name FROM ps_cov WHERE id = ?1")
        .await
        .unwrap();
    ps.set_int(&mut conn, 1, 200).unwrap();
    let rows = ps.fetch(&mut conn, vec![Value::Int(200)]).await.unwrap();
    assert_eq!(rows.len(), 1);
    ps.close_with_connection(&mut conn).unwrap();
}

// ── fetch_result_set ───────────────────────────────────────────────

#[tokio::test]
async fn ps_fetch_result_set() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(
        &mut conn,
        "INSERT INTO ps_cov(id, name) VALUES (300, 'frs')",
    )
    .await
    .unwrap();
    stmt.close_with_connection(&mut conn).unwrap();

    let mut ps = conn
        .prepare_statement("SELECT id, name FROM ps_cov WHERE id = ?1")
        .await
        .unwrap();
    ps.set_int(&mut conn, 1, 300).unwrap();
    let mut rs = ps
        .fetch_result_set(&mut conn, vec![Value::Int(300)])
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());
    rs.close_with_connection(&mut conn).unwrap();
    ps.close_with_connection(&mut conn).unwrap();
}

// ── exec (update with Value params) ────────────────────────────────

#[tokio::test]
async fn ps_exec_update_with_values() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn
        .prepare_statement("INSERT INTO ps_cov(id, name) VALUES (?1, ?2)")
        .await
        .unwrap();
    let result = ps
        .exec(
            &mut conn,
            vec![Value::Int(500), Value::String("exec".to_string())],
        )
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);
    ps.close_with_connection(&mut conn).unwrap();
}

// ── batch through filter chain ─────────────────────────────────────

#[tokio::test]
async fn ps_batch_through_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    setup_table(&mut conn).await;

    let mut ps = conn
        .prepare_statement("INSERT INTO ps_cov(id, name) VALUES (?1, ?2)")
        .await
        .unwrap();

    ps.add_batch(
        &mut conn,
        vec![Value::Int(1001), Value::String("fc1".to_string())],
    )
    .unwrap();
    ps.add_batch(
        &mut conn,
        vec![Value::Int(1002), Value::String("fc2".to_string())],
    )
    .unwrap();

    let counts = ps.execute_batch(&mut conn).await.unwrap();
    assert_eq!(counts.len(), 2);

    ps.close_with_connection(&mut conn).unwrap();
}

// ── property restore on close ──────────────────────────────────────

#[tokio::test]
async fn ps_property_restore_on_close() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;

    let mut ps = conn.prepare_statement("SELECT 1").await.unwrap();

    // Change properties
    ps.set_max_field_size(&mut conn, 9999).unwrap();
    ps.set_max_rows(&mut conn, 9999).unwrap();
    ps.set_query_timeout(&mut conn, 99).unwrap();
    ps.set_fetch_direction(&mut conn, 1001).unwrap();
    ps.set_fetch_size(&mut conn, 999).unwrap();

    // Close should restore defaults
    ps.close_with_connection(&mut conn).unwrap();

    // Prepare a new statement and verify defaults are restored
    let mut ps2 = conn.prepare_statement("SELECT 1").await.unwrap();
    let mfs = ps2.max_field_size(&mut conn).unwrap();
    assert!(mfs >= 0);
    let mr = ps2.max_rows(&mut conn).unwrap();
    assert_eq!(mr, 0);
    ps2.close_with_connection(&mut conn).unwrap();
}
