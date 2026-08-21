//! `FilterChainImpl` coverage boost -- macro-generated proxy methods, Clob/NClob/Blob
//! chain, Connection LOB chain, navigation, temporal, resource, and update paths.
//!
//! All getter calls use `let _ = ...` because `SQLite` type conversions may fail;
//! the goal is to exercise the `FilterChainImpl` proxy code paths.

use druid::core::{
    DruidPooledConnection, FilterAdapter, FilterChainImpl, PhysicalConnectionFactory,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::Arc;

// -- helpers ----------------------------------------------------------------

async fn make_connection() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    DruidPooledConnection::with_context(
        physical,
        300,
        "fc-coverage-boost".to_string(),
        None,
        Box::new(|_, _| {}),
    )
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
        301,
        "fc-coverage-boost-filtered".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    )
}

async fn setup_table(connection: &mut DruidPooledConnection) {
    let mut stmt = connection.create_statement().await.unwrap();
    stmt.execute_update(
        connection,
        "CREATE TABLE fc_cov(id INTEGER, name TEXT, val REAL, flag INTEGER, raw BLOB)",
    )
    .await
    .unwrap();
    stmt.execute_update(
        connection,
        "INSERT INTO fc_cov VALUES (1, 'hello', 3.14, 1, X'DEADBEEF')",
    )
    .await
    .unwrap();
    stmt.execute_update(
        connection,
        "INSERT INTO fc_cov VALUES (2, NULL, NULL, 0, NULL)",
    )
    .await
    .unwrap();
    stmt.close_with_connection(connection).unwrap();
}

// -- Scalar by_label variants -----------------------------------------------

#[tokio::test]
async fn scalar_getters_by_label_with_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(
            &mut conn,
            "SELECT id, name, val, flag FROM fc_cov WHERE id = 1",
        )
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    // All by_label scalar getters -- exercise the macro-generated proxy paths
    let _ = rs.string_by_label(&mut conn, "name");
    let _ = rs.boolean_by_label(&mut conn, "flag");
    let _ = rs.byte_by_label(&mut conn, "id");
    let _ = rs.short_by_label(&mut conn, "id");
    let _ = rs.int_by_label(&mut conn, "id");
    let _ = rs.long_by_label(&mut conn, "id");
    let _ = rs.float_by_label(&mut conn, "val");
    let _ = rs.double_by_label(&mut conn, "val");
    let _ = rs.n_string_by_label(&mut conn, "name");
    let _ = rs.was_null(&mut conn);

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- Temporal methods --------------------------------------------------------

#[tokio::test]
async fn temporal_getters_with_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(&mut conn, "CREATE TABLE fc_temporal(ts TEXT)")
        .await
        .unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO fc_temporal VALUES ('2024-01-15')")
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT ts FROM fc_temporal")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    // date variants
    let _ = rs.date(&mut conn, 1);
    let _ = rs.date_by_label(&mut conn, "ts");
    let _ = rs.date_with_calendar(&mut conn, 1, None);
    let _ = rs.date_by_label_with_calendar(&mut conn, "ts", None);

    // time variants
    let _ = rs.time(&mut conn, 1);
    let _ = rs.time_by_label(&mut conn, "ts");
    let _ = rs.time_with_calendar(&mut conn, 1, None);
    let _ = rs.time_by_label_with_calendar(&mut conn, "ts", None);

    // timestamp variants
    let _ = rs.timestamp(&mut conn, 1);
    let _ = rs.timestamp_by_label(&mut conn, "ts");
    let _ = rs.timestamp_with_calendar(&mut conn, 1, None);
    let _ = rs.timestamp_by_label_with_calendar(&mut conn, "ts", None);

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- BigDecimal methods -----------------------------------------------------

#[tokio::test]
async fn big_decimal_getters_with_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT val FROM fc_cov WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    let _ = rs.big_decimal(&mut conn, 1);
    let _ = rs.big_decimal_by_label(&mut conn, "val");
    let _ = rs.big_decimal_with_scale(&mut conn, 1, 2);
    let _ = rs.big_decimal_by_label_with_scale(&mut conn, "val", 2);

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- Navigation methods -----------------------------------------------------

#[tokio::test]
async fn navigation_methods_with_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id, name FROM fc_cov")
        .await
        .unwrap();

    let _ = rs.is_before_first(&mut conn);
    let _ = rs.is_after_last(&mut conn);
    assert!(rs.next(&mut conn).unwrap());
    let _ = rs.is_first(&mut conn);
    let _ = rs.is_last(&mut conn);
    let _ = rs.previous(&mut conn);
    let _ = rs.first(&mut conn);
    let _ = rs.last(&mut conn);
    let _ = rs.absolute(&mut conn, 1);
    let _ = rs.relative(&mut conn, 0);
    let _ = rs.before_first(&mut conn);
    let _ = rs.after_last(&mut conn);
    let _ = rs.fetch_direction(&mut conn);
    let _ = rs.set_fetch_direction(&mut conn, 1000);
    let _ = rs.fetch_size(&mut conn);
    let _ = rs.set_fetch_size(&mut conn, 10);
    let _ = rs.result_set_type(&mut conn);
    let _ = rs.concurrency(&mut conn);
    let _ = rs.holdability(&mut conn);
    let _ = rs.cursor_name(&mut conn);
    let _ = rs.row_updated(&mut conn);
    let _ = rs.row_inserted(&mut conn);
    let _ = rs.row_deleted(&mut conn);
    assert!(!rs.is_closed());

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- Resource getters (blob, clob, array, etc.) ------------------------------

#[tokio::test]
async fn resource_getters_with_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT raw FROM fc_cov WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    // All resource getters -- exercise macro-generated proxy paths
    let _ = rs.blob(&mut conn, 1);
    let _ = rs.blob_by_label(&mut conn, "raw");
    let _ = rs.clob(&mut conn, 1);
    let _ = rs.clob_by_label(&mut conn, "raw");
    let _ = rs.array(&mut conn, 1);
    let _ = rs.array_by_label(&mut conn, "raw");
    let _ = rs.url(&mut conn, 1);
    let _ = rs.url_by_label(&mut conn, "raw");
    let _ = rs.row_id(&mut conn, 1);
    let _ = rs.row_id_by_label(&mut conn, "raw");
    let _ = rs.n_clob(&mut conn, 1);
    let _ = rs.n_clob_by_label(&mut conn, "raw");
    let _ = rs.sql_xml(&mut conn, 1);
    let _ = rs.sql_xml_by_label(&mut conn, "raw");
    let _ = rs.ascii_stream(&mut conn, 1);
    let _ = rs.ascii_stream_by_label(&mut conn, "raw");
    let _ = rs.unicode_stream(&mut conn, 1);
    let _ = rs.unicode_stream_by_label(&mut conn, "raw");
    let _ = rs.binary_stream(&mut conn, 1);
    let _ = rs.binary_stream_by_label(&mut conn, "raw");
    let _ = rs.character_stream(&mut conn, 1);
    let _ = rs.character_stream_by_label(&mut conn, "raw");
    let _ = rs.n_character_stream(&mut conn, 1);
    let _ = rs.n_character_stream_by_label(&mut conn, "raw");

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- Resource getters via clob_proxy / n_clob_proxy -------------------------

#[tokio::test]
async fn clob_proxy_getters_with_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT name FROM fc_cov WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    let _ = rs.clob_proxy(&mut conn, 1);
    let _ = rs.clob_proxy_by_label(&mut conn, "name");
    let _ = rs.n_clob_proxy(&mut conn, 1);
    let _ = rs.n_clob_proxy_by_label(&mut conn, "name");

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- Connection LOB creation (create_blob, create_clob, create_n_clob) ------

#[tokio::test]
async fn connection_create_lob_with_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    let _ = conn.create_blob().await;
    let _ = conn.create_clob().await;
    let _ = conn.create_n_clob().await;
}

// -- Object getters by label ------------------------------------------------

#[tokio::test]
async fn object_getters_by_label_with_filter_chain() {
    let mut conn = make_connection_with_chain().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id, name FROM fc_cov WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    let _ = rs.object_by_label_with_type_map(&mut conn, "id", None);

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- Scalar getters without filter chain (empty chain path) ------------------

#[tokio::test]
async fn scalar_getters_no_filter_chain() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(
            &mut conn,
            "SELECT id, name, val, flag FROM fc_cov WHERE id = 1",
        )
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    let _ = rs.string_by_label(&mut conn, "name");
    let _ = rs.boolean_by_label(&mut conn, "flag");
    let _ = rs.byte_by_label(&mut conn, "id");
    let _ = rs.short_by_label(&mut conn, "id");
    let _ = rs.int_by_label(&mut conn, "id");
    let _ = rs.long_by_label(&mut conn, "id");
    let _ = rs.float_by_label(&mut conn, "val");
    let _ = rs.double_by_label(&mut conn, "val");
    let _ = rs.n_string_by_label(&mut conn, "name");

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- Temporal methods without filter chain -----------------------------------

#[tokio::test]
async fn temporal_getters_no_filter_chain() {
    let mut conn = make_connection().await;
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(&mut conn, "CREATE TABLE fc_temp2(ts TEXT)")
        .await
        .unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO fc_temp2 VALUES ('2024-06-01')")
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT ts FROM fc_temp2")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    let _ = rs.date(&mut conn, 1);
    let _ = rs.date_by_label(&mut conn, "ts");
    let _ = rs.time(&mut conn, 1);
    let _ = rs.time_by_label(&mut conn, "ts");
    let _ = rs.timestamp(&mut conn, 1);
    let _ = rs.timestamp_by_label(&mut conn, "ts");

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- Navigation methods without filter chain ---------------------------------

#[tokio::test]
async fn navigation_methods_no_filter_chain() {
    let mut conn = make_connection().await;
    setup_table(&mut conn).await;
    let mut stmt = conn.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id FROM fc_cov")
        .await
        .unwrap();

    let _ = rs.is_before_first(&mut conn);
    let _ = rs.is_after_last(&mut conn);
    assert!(rs.next(&mut conn).unwrap());
    let _ = rs.is_first(&mut conn);
    let _ = rs.is_last(&mut conn);
    let _ = rs.previous(&mut conn);
    let _ = rs.first(&mut conn);
    let _ = rs.last(&mut conn);
    let _ = rs.absolute(&mut conn, 1);
    let _ = rs.relative(&mut conn, 0);
    let _ = rs.before_first(&mut conn);
    let _ = rs.after_last(&mut conn);
    let _ = rs.fetch_direction(&mut conn);
    let _ = rs.set_fetch_direction(&mut conn, 1000);
    let _ = rs.fetch_size(&mut conn);
    let _ = rs.set_fetch_size(&mut conn, 10);
    let _ = rs.result_set_type(&mut conn);
    let _ = rs.concurrency(&mut conn);
    let _ = rs.holdability(&mut conn);
    let _ = rs.row_updated(&mut conn);
    let _ = rs.row_inserted(&mut conn);
    let _ = rs.row_deleted(&mut conn);

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- FilterChainImpl ClobFilterChain via clob_proxy -------------------------

#[tokio::test]
async fn clob_filter_chain_methods_via_proxy() {
    let mut conn = make_connection_with_chain().await;
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(&mut conn, "CREATE TABLE fc_clob(txt TEXT)")
        .await
        .unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO fc_clob VALUES ('hello world')")
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT txt FROM fc_clob")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    // Get a clob proxy through the filter chain
    if let Ok(Some(proxy)) = rs.clob_proxy(&mut conn, 1) {
        let _ = proxy.length().await;
        let _ = proxy.get_sub_string(1, 5).await;
        let _ = proxy.get_character_stream().await;
        let _ = proxy.get_ascii_stream().await;
        let _ = proxy.position_string(&"hello".to_string().into(), 1).await;
        let _ = proxy.set_string(1, &"test".to_string().into()).await;
        let _ = proxy
            .set_string_range(1, &"ab".to_string().into(), 0, 2)
            .await;
        let _ = proxy.set_ascii_stream(1).await;
        let _ = proxy.set_character_stream(1).await;
        let _ = proxy.truncate(5).await;
        let _ = proxy.free().await;
        let _ = proxy.get_character_stream_range(1, 3).await;
    }

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- FilterChainImpl with multiple filters -----------------------------------

#[tokio::test]
async fn filter_chain_with_multiple_filters_coverage() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain.add_filter(Arc::new(FilterAdapter::new()));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    let mut conn = DruidPooledConnection::with_context(
        physical,
        302,
        "fc-multi-filter".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    );

    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(&mut conn, "CREATE TABLE fc_multi(id INTEGER)")
        .await
        .unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO fc_multi VALUES (42)")
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id FROM fc_multi")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());

    // Exercise scalar getters through multi-filter chain
    let _ = rs.int(&mut conn, 1);
    let _ = rs.int_by_label(&mut conn, "id");
    let _ = rs.long(&mut conn, 1);
    let _ = rs.long_by_label(&mut conn, "id");
    let _ = rs.string(&mut conn, 1);
    let _ = rs.string_by_label(&mut conn, "id");
    let _ = rs.boolean(&mut conn, 1);
    let _ = rs.boolean_by_label(&mut conn, "id");
    let _ = rs.double(&mut conn, 1);
    let _ = rs.double_by_label(&mut conn, "id");
    let _ = rs.float(&mut conn, 1);
    let _ = rs.float_by_label(&mut conn, "id");
    let _ = rs.byte(&mut conn, 1);
    let _ = rs.byte_by_label(&mut conn, "id");
    let _ = rs.short(&mut conn, 1);
    let _ = rs.short_by_label(&mut conn, "id");

    // Navigation through multi-filter chain
    let _ = rs.first(&mut conn);
    let _ = rs.last(&mut conn);
    let _ = rs.absolute(&mut conn, 1);
    let _ = rs.is_before_first(&mut conn);
    let _ = rs.is_after_last(&mut conn);
    let _ = rs.is_first(&mut conn);
    let _ = rs.is_last(&mut conn);

    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}

// -- FilterChainImpl statement batch paths -----------------------------------

#[tokio::test]
async fn filter_chain_statement_batch_paths() {
    let mut conn = make_connection_with_chain().await;
    let mut stmt = conn.create_statement().await.unwrap();

    stmt.add_batch(&mut conn, "CREATE TABLE fc_batch(id INTEGER)")
        .unwrap();
    stmt.add_batch(&mut conn, "INSERT INTO fc_batch VALUES (1)")
        .unwrap();
    let counts = stmt.execute_batch(&mut conn).await.unwrap();
    assert_eq!(counts.len(), 2);

    stmt.add_batch(&mut conn, "INSERT INTO fc_batch VALUES (2)")
        .unwrap();
    stmt.clear_batch(&mut conn).unwrap();

    stmt.close_with_connection(&mut conn).unwrap();
}

// -- FilterChainImpl PreparedStatement paths ---------------------------------

#[tokio::test]
async fn filter_chain_prepared_statement_paths() {
    let mut conn = make_connection_with_chain().await;
    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(&mut conn, "CREATE TABLE fc_ps(id INTEGER, name TEXT)")
        .await
        .unwrap();
    stmt.close_with_connection(&mut conn).unwrap();

    let mut ps = conn
        .prepare_statement("INSERT INTO fc_ps VALUES (?1, ?2)")
        .await
        .unwrap();
    ps.set_int(&mut conn, 1, 1).unwrap();
    ps.set_n_string(&mut conn, 2, Some("test".to_string()))
        .unwrap();
    let _ = ps.execute_update_bound(&mut conn).await.unwrap();
    ps.close_with_connection(&mut conn).unwrap();

    let mut ps2 = conn
        .prepare_statement("SELECT id, name FROM fc_ps WHERE id = ?1")
        .await
        .unwrap();
    ps2.set_int(&mut conn, 1, 1).unwrap();
    let mut rs = ps2.execute_query_bound(&mut conn).await.unwrap();
    assert!(rs.next(&mut conn).unwrap());
    let _ = rs.int(&mut conn, 1);
    let _ = rs.n_string(&mut conn, 2);
    rs.close_with_connection(&mut conn).unwrap();
    ps2.close_with_connection(&mut conn).unwrap();
}

// -- FilterChainImpl with LogFilter -----------------------------------------

#[tokio::test]
async fn filter_chain_with_log_filter() {
    use druid::core::LogFilter;
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(LogFilter::new()));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    let mut conn = DruidPooledConnection::with_context(
        physical,
        303,
        "fc-log-filter".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    );

    let mut stmt = conn.create_statement().await.unwrap();
    stmt.execute_update(&mut conn, "CREATE TABLE fc_log(id INTEGER)")
        .await
        .unwrap();
    stmt.execute_update(&mut conn, "INSERT INTO fc_log VALUES (1)")
        .await
        .unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut conn, "SELECT id FROM fc_log")
        .await
        .unwrap();
    assert!(rs.next(&mut conn).unwrap());
    let _ = rs.int(&mut conn, 1);
    let _ = rs.int_by_label(&mut conn, "id");
    let _ = rs.long(&mut conn, 1);
    let _ = rs.string(&mut conn, 1);
    let _ = rs.boolean(&mut conn, 1);
    let _ = rs.double(&mut conn, 1);
    let _ = rs.float(&mut conn, 1);
    let _ = rs.byte(&mut conn, 1);
    let _ = rs.short(&mut conn, 1);
    let _ = rs.is_before_first(&mut conn);
    let _ = rs.is_after_last(&mut conn);
    let _ = rs.first(&mut conn);
    let _ = rs.last(&mut conn);
    rs.close_with_connection(&mut conn).unwrap();
    stmt.close_with_connection(&mut conn).unwrap();
}
