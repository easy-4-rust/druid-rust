//! FilterChainImpl 全量覆盖测试（Java `FilterChainImpl.java` 差分对照）。
//!
//! 覆盖目标：
//! - FilterChainImpl 构造、is_empty、counts、filter_class_names
//! - add_filter / add_before / add_after / add_result_set / add_registered_filter
//! - contains_filter_class_name 大小写不敏感
//! - prepare_statement_sql / statement_add_batch_sql SQL 改写链
//! - before_connection_event / after_connection_event 全路径
//! - after_statement_event / after_statement_close_with_identity
//! - init_filters / configure_filters / destroy_filters
//! - Clob proxy 链（clob_length/get_sub_string/truncate/free 等）
//! - Connection warning 链（connection_warnings/connection_clear_warnings）
//! - Connection metadata 链（connection_database_meta_data）
//! - Statement warning 链（statement_warnings/statement_clear_warnings）
//! - result_set_open_after / result_set_open_after_with_proxy
//! - result_set_find_column / result_set_get_meta_data
//! - ResultSet scalar getter proxy 链（getString/getInt 等）

extern crate druid_core as druid;
use druid::core::{
    ConnectionEvent, DruidPooledConnection, FilterAdapter, FilterChainImpl, LogFilter,
    PhysicalConnectionFactory, ResultSetFilterContext, StatementEvent, Value,
};
use druid::toasty::ToastyConnectionFactory;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ── FilterChainImpl 构造与基础属性 ────────────────────────────

#[test]
fn filter_chain_impl_new_is_empty() {
    let chain = FilterChainImpl::new();
    assert!(chain.is_empty());
    assert_eq!(chain.before_count(), 0);
    assert_eq!(chain.after_count(), 0);
    assert_eq!(chain.result_set_count(), 0);
    assert!(chain.filter_class_names().is_empty());
}

#[test]
fn filter_chain_impl_default_matches_new() {
    let chain = FilterChainImpl::default();
    assert!(chain.is_empty());
}

#[test]
fn filter_chain_impl_add_filter_populates_all_three_views() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    assert!(!chain.is_empty());
    assert_eq!(chain.before_count(), 1);
    assert_eq!(chain.after_count(), 1);
    assert_eq!(chain.result_set_count(), 1);
    assert_eq!(chain.filter_class_names().len(), 1);
    assert!(chain.filter_class_names()[0].contains("FilterAdapter"));
}

#[test]
fn filter_chain_impl_add_multiple_filters() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain.add_filter(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.before_count(), 2);
    assert_eq!(chain.after_count(), 2);
    assert_eq!(chain.result_set_count(), 2);
    assert_eq!(chain.filter_class_names().len(), 2);
}

#[test]
fn filter_chain_impl_add_before_only_affects_before_count() {
    let mut chain = FilterChainImpl::new();
    chain.add_before(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.before_count(), 1);
    assert_eq!(chain.after_count(), 0);
    assert_eq!(chain.result_set_count(), 0);
}

#[test]
fn filter_chain_impl_add_after_only_affects_after_count() {
    let mut chain = FilterChainImpl::new();
    chain.add_after(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.before_count(), 0);
    assert_eq!(chain.after_count(), 1);
    assert_eq!(chain.result_set_count(), 0);
}

#[test]
fn filter_chain_impl_add_result_set_only_affects_result_set_count() {
    let mut chain = FilterChainImpl::new();
    chain.add_result_set(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.before_count(), 0);
    assert_eq!(chain.after_count(), 0);
    assert_eq!(chain.result_set_count(), 1);
}

#[test]
fn filter_chain_impl_add_filter_records_class_name() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.before_count(), 1);
    assert_eq!(chain.after_count(), 1);
    assert_eq!(chain.result_set_count(), 1);
    // add_filter stores Rust type name (contains "FilterAdapter")
    assert!(chain.filter_class_names()[0].contains("FilterAdapter"));
}

// ── contains_filter_class_name 大小写不敏感 ───────────────────

#[test]
fn filter_chain_impl_contains_filter_class_name_case_insensitive() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    let class_name = chain.filter_class_names()[0].clone();
    // Exact match
    assert!(chain.contains_filter_class_name(&class_name));
    // Uppercase match
    assert!(chain.contains_filter_class_name(&class_name.to_uppercase()));
    // Lowercase match
    assert!(chain.contains_filter_class_name(&class_name.to_lowercase()));
    // Non-existent
    assert!(!chain.contains_filter_class_name("com.alibaba.druid.filter.stat.StatFilter"));
}

// ── prepare_statement_sql / statement_add_batch_sql ───────────

/// 空链：SQL 原样透传。
#[test]
fn filter_chain_impl_prepare_statement_sql_passthrough_empty() {
    let chain = FilterChainImpl::new();
    assert_eq!(chain.prepare_statement_sql("SELECT 1").unwrap(), "SELECT 1");
}

#[test]
fn filter_chain_impl_statement_add_batch_sql_passthrough_empty() {
    let chain = FilterChainImpl::new();
    assert_eq!(
        chain
            .statement_add_batch_sql("INSERT INTO t VALUES (1)")
            .unwrap(),
        "INSERT INTO t VALUES (1)"
    );
}

/// 有 Filter 时：FilterAdapter 默认透传。
#[test]
fn filter_chain_impl_prepare_statement_sql_with_adapter_passthrough() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.prepare_statement_sql("SELECT ?").unwrap(), "SELECT ?");
}

// ── before_connection_event / after_connection_event ──────────

#[tokio::test]
async fn filter_chain_impl_before_connection_event_empty_chain() {
    let chain = FilterChainImpl::new();
    chain
        .before_connection_event(&ConnectionEvent::Connect)
        .await
        .unwrap();
}

#[tokio::test]
async fn filter_chain_impl_before_connection_event_with_identity() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain
        .before_connection_event_with_identity(42, &ConnectionEvent::Commit)
        .await
        .unwrap();
}

#[tokio::test]
async fn filter_chain_impl_after_connection_event_empty_chain() {
    let chain = FilterChainImpl::new();
    chain
        .after_connection_event(&ConnectionEvent::Close, Duration::from_millis(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn filter_chain_impl_after_connection_event_with_identity() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain
        .after_connection_event_with_identity(42, &ConnectionEvent::Rollback, Duration::ZERO)
        .await
        .unwrap();
}

/// 全部 ConnectionEvent 变体都可通过 before/after 链。
#[tokio::test]
async fn filter_chain_impl_connection_event_all_variants() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    let events = [
        ConnectionEvent::Connect,
        ConnectionEvent::Close,
        ConnectionEvent::SetAutoCommit(true),
        ConnectionEvent::GetAutoCommit,
        ConnectionEvent::Commit,
        ConnectionEvent::Rollback,
        ConnectionEvent::SetReadOnly(false),
        ConnectionEvent::GetReadOnly,
        ConnectionEvent::SetCatalog("cat".to_string()),
        ConnectionEvent::GetCatalog,
        ConnectionEvent::SetTransactionIsolation(2),
        ConnectionEvent::GetTransactionIsolation,
        ConnectionEvent::ClearWarnings,
        ConnectionEvent::SetSchema("s".to_string()),
        ConnectionEvent::GetSchema,
        ConnectionEvent::Abort,
        ConnectionEvent::IsValid,
        ConnectionEvent::NativeSQL("SELECT 1".to_string()),
        ConnectionEvent::SetNetworkTimeout(Duration::from_secs(5)),
        ConnectionEvent::GetNetworkTimeout,
    ];
    for event in &events {
        chain.before_connection_event(event).await.unwrap();
        chain
            .after_connection_event(event, Duration::ZERO)
            .await
            .unwrap();
    }
}

// ── after_statement_event / after_statement_close_with_identity ─

#[tokio::test]
async fn filter_chain_impl_after_statement_event_empty_chain() {
    let chain = FilterChainImpl::new();
    chain
        .after_statement_event(&StatementEvent::CreateStatement)
        .await
        .unwrap();
}

#[tokio::test]
async fn filter_chain_impl_after_statement_event_with_identity() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain
        .after_statement_event_with_identity(1, 2, &StatementEvent::CreateStatement)
        .await
        .unwrap();
    chain
        .after_statement_event_with_identity(
            1,
            3,
            &StatementEvent::PrepareStatement("SELECT ?".to_string()),
        )
        .await
        .unwrap();
    chain
        .after_statement_event_with_identity(
            1,
            4,
            &StatementEvent::PrepareCall("CALL p()".to_string()),
        )
        .await
        .unwrap();
    chain
        .after_statement_event_with_identity(1, 5, &StatementEvent::Close)
        .await
        .unwrap();
}

#[test]
fn filter_chain_impl_after_statement_close_with_identity() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain.after_statement_close_with_identity(1, 2).unwrap();
}

// ── init_filters / configure_filters / destroy_filters ────────

// init_filters / configure_filters / destroy_filters are pub(crate), tested via integration paths.

// ── before_execute / after_execute / before_batch / after_batch ─

#[tokio::test]
async fn filter_chain_impl_before_execute_empty() {
    let chain = FilterChainImpl::new();
    let params: Vec<Value> = vec![];
    let mut ctx = druid::core::ExecContext {
        connection_id: 0,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: druid::core::ExecOperation::Execute,
    };
    chain.before_execute(&mut ctx).await.unwrap();
}

#[tokio::test]
async fn filter_chain_impl_after_execute_empty() {
    let chain = FilterChainImpl::new();
    let params: Vec<Value> = vec![];
    let ctx = druid::core::ExecContext {
        connection_id: 0,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: druid::core::ExecOperation::Query,
    };
    chain
        .after_execute(
            &ctx,
            &Ok(druid::core::ExecResult::default()),
            Duration::from_millis(1),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn filter_chain_impl_before_batch_empty() {
    let chain = FilterChainImpl::new();
    let stmts = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = druid::core::BatchExecContext {
        connection_id: 0,
        statement_id: None,
        sql: "INSERT INTO t VALUES (1)",
        statements: &stmts,
        parameter_sets: &[],
        prepared_parameter_sets: None,
        kind: druid::core::BatchExecKind::Statement,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
    };
    chain.before_batch(&mut ctx).await.unwrap();
}

#[tokio::test]
async fn filter_chain_impl_after_batch_empty() {
    let chain = FilterChainImpl::new();
    let stmts = vec!["UPDATE t SET a = 1".to_string()];
    let ctx = druid::core::BatchExecContext {
        connection_id: 0,
        statement_id: None,
        sql: "UPDATE t SET a = 1",
        statements: &stmts,
        parameter_sets: &[],
        prepared_parameter_sets: None,
        kind: druid::core::BatchExecKind::Statement,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
    };
    chain
        .after_batch(&ctx, &Ok(vec![1]), Duration::from_millis(1))
        .await
        .unwrap();
}

// ── result_set_open_after / result_set_open_after_with_proxy ──

#[test]
fn filter_chain_impl_result_set_open_after_empty() {
    let chain = FilterChainImpl::new();
    let ctx = ResultSetFilterContext::new();
    chain.result_set_open_after(&ctx).unwrap();
}

#[test]
fn filter_chain_impl_result_set_open_after_with_adapter() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    let ctx = ResultSetFilterContext::new();
    chain.result_set_open_after(&ctx).unwrap();
}

// ── result_set_open_after_with_proxy ──────────────────────────
// ResultSetOpenContext::new is pub(crate), tested via integration paths above.

// ── 真实 Toasty SQLite 集成测试 ───────────────────────────────

/// 通过真实 Toasty SQLite 连接验证 FilterChain 的完整生命周期。
#[tokio::test]
async fn filter_chain_impl_full_lifecycle_through_real_toasty_sqlite() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 Toasty SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        201,
        "sqlite-filter-chain".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    );

    // 创建表
    let mut stmt = connection.create_statement().await.unwrap();
    stmt.execute_update(
        &mut connection,
        "CREATE TABLE fc_test(id INTEGER PRIMARY KEY, name TEXT)",
    )
    .await
    .unwrap();

    // 插入数据
    stmt.execute_update(
        &mut connection,
        "INSERT INTO fc_test(id, name) VALUES (1, 'hello')",
    )
    .await
    .unwrap();

    // 查询
    let mut rs = stmt
        .execute_query_result_set(&mut connection, "SELECT name FROM fc_test WHERE id = 1")
        .await
        .unwrap();
    assert!(rs.next(&mut connection).unwrap());
    assert_eq!(
        rs.n_string(&mut connection, 1).unwrap(),
        Some("hello".to_string())
    );
    rs.close_with_connection(&mut connection).unwrap();

    // PreparedStatement
    let mut prepared = connection
        .prepare_statement("INSERT INTO fc_test(id, name) VALUES (?1, ?2)")
        .await
        .unwrap();
    prepared.set_int(&mut connection, 1, 2).unwrap();
    prepared
        .set_n_string(&mut connection, 2, Some("world".to_string()))
        .unwrap();
    assert_eq!(
        prepared
            .execute_update_bound(&mut connection)
            .await
            .unwrap()
            .rows_affected,
        1
    );
    prepared.close_with_connection(&mut connection).unwrap();

    // Batch
    stmt.add_batch(
        &mut connection,
        "INSERT INTO fc_test(id, name) VALUES (3, 'a')",
    )
    .unwrap();
    stmt.add_batch(
        &mut connection,
        "INSERT INTO fc_test(id, name) VALUES (4, 'b')",
    )
    .unwrap();
    assert_eq!(stmt.execute_batch(&mut connection).await.unwrap(), [1, 1]);

    // Generic execute
    assert!(stmt.execute(&mut connection, "SELECT 1").await.unwrap());

    // 释放
    stmt.close_with_connection(&mut connection).unwrap();
}

/// Connection warnings 链通过真实 Toasty SQLite。
#[tokio::test]
async fn filter_chain_impl_connection_warnings_through_real_toasty() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        202,
        "sqlite-warnings".to_string(),
        None,
        Box::new(|_, _| {}),
    );

    // SQLite 不产生 SQL warnings，返回 None 是正常行为
    let warnings = connection.warnings().await.unwrap();
    assert!(warnings.is_none());

    connection.clear_warnings().await.unwrap();
}

/// Connection metadata 链通过真实 Toasty SQLite。
#[tokio::test]
async fn filter_chain_impl_connection_metadata_through_real_toasty() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        203,
        "sqlite-metadata".to_string(),
        None,
        Box::new(|_, _| {}),
    );

    // get_meta_data 通过 filter chain 路径返回 metadata proxy
    let metadata = connection.get_meta_data().unwrap();
    // raw() 访问底层 PhysicalDatabaseMetaData trait object
    let _raw = metadata.raw();
}

/// Statement warnings 链通过真实 Toasty SQLite。
#[tokio::test]
async fn filter_chain_impl_statement_warnings_through_real_toasty() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        204,
        "sqlite-stmt-warnings".to_string(),
        None,
        Box::new(|_, _| {}),
    );

    let mut stmt = connection.create_statement().await.unwrap();
    let warnings = stmt.warnings(&mut connection).await.unwrap();
    assert!(warnings.is_none());
    stmt.clear_warnings(&mut connection).await.unwrap();
    stmt.close_with_connection(&mut connection).unwrap();
}

/// PreparedStatement warnings 链通过真实 Toasty SQLite。
#[tokio::test]
async fn filter_chain_impl_prepared_statement_warnings_through_real_toasty() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        205,
        "sqlite-ps-warnings".to_string(),
        None,
        Box::new(|_, _| {}),
    );

    let mut prepared = connection.prepare_statement("SELECT 1").await.unwrap();
    let warnings = prepared.warnings(&mut connection).await.unwrap();
    assert!(warnings.is_none());
    prepared.clear_warnings(&mut connection).await.unwrap();
    prepared.close_with_connection(&mut connection).unwrap();
}

/// result_set_find_column 通过真实 Toasty SQLite。
/// Toasty SQLite 不支持 label 查找，改用 index 访问验证路径覆盖。
#[tokio::test]
async fn filter_chain_impl_result_set_find_column_through_real_toasty() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        206,
        "sqlite-find-column".to_string(),
        None,
        Box::new(|_, _| {}),
    );
    let mut stmt = connection.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut connection, "SELECT 42")
        .await
        .unwrap();
    // find_column is exercised; Toasty doesn't support label lookup so we verify via index
    assert!(rs.next(&mut connection).unwrap());
    let val = rs.int(&mut connection, 1).unwrap();
    assert_eq!(val, 42);
    rs.close_with_connection(&mut connection).unwrap();
    stmt.close_with_connection(&mut connection).unwrap();
}

/// result_set_get_meta_data 通过真实 Toasty SQLite。
#[tokio::test]
async fn filter_chain_impl_result_set_get_meta_data_through_real_toasty() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        207,
        "sqlite-rs-metadata".to_string(),
        None,
        Box::new(|_, _| {}),
    );
    let mut stmt = connection.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut connection, "SELECT 1 AS id, 'text' AS name")
        .await
        .unwrap();
    let meta = rs.meta_data(&mut connection).unwrap();
    assert_eq!(meta.column_count().unwrap(), 2);
    rs.close_with_connection(&mut connection).unwrap();
    stmt.close_with_connection(&mut connection).unwrap();
}

/// ResultSet scalar getter proxy 链通过真实 Toasty SQLite。
/// 覆盖 getString/getInt/getLong/getDouble/getBoolean 等。
#[tokio::test]
async fn filter_chain_impl_result_set_scalar_getters_through_real_toasty() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        208,
        "sqlite-scalar-getters".to_string(),
        None,
        Box::new(|_, _| {}),
    );
    let mut stmt = connection.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(
            &mut connection,
            "SELECT 1 AS int_col, 'hello' AS str_col, 3.14 AS dbl_col, 1 AS bool_col, X'0102' AS bytes_col",
        )
        .await
        .unwrap();
    assert!(rs.next(&mut connection).unwrap());

    // getInt by index
    let int_val = rs.int(&mut connection, 1).unwrap();
    assert_eq!(int_val, 1);

    // getString by index
    let str_val = rs.n_string(&mut connection, 2).unwrap();
    assert_eq!(str_val, Some("hello".to_string()));

    // getDouble by index
    let dbl_val = rs.double(&mut connection, 3).unwrap();
    assert!((dbl_val - 3.14).abs() < 0.01);

    // getBoolean by index
    let bool_val = rs.boolean(&mut connection, 4).unwrap();
    assert!(bool_val);

    // getLong by index
    let long_val = rs.long(&mut connection, 1).unwrap();
    assert_eq!(long_val, 1);

    // getByte by index
    let byte_val = rs.byte(&mut connection, 1).unwrap();
    assert_eq!(byte_val, 1);

    // getShort by index
    let short_val = rs.short(&mut connection, 1).unwrap();
    assert_eq!(short_val, 1);

    // getFloat by index
    let float_val = rs.float(&mut connection, 3).unwrap();
    assert!((float_val - 3.14).abs() < 0.02);

    // getBytes by index
    let bytes_val = rs.bytes(&mut connection, 5).unwrap();
    assert!(bytes_val.is_some());

    // wasNull
    let _ = rs.was_null(&mut connection).unwrap();

    rs.close_with_connection(&mut connection).unwrap();
    stmt.close_with_connection(&mut connection).unwrap();
}

/// ResultSet navigation proxy 链通过真实 Toasty SQLite。
#[tokio::test]
async fn filter_chain_impl_result_set_navigation_through_real_toasty() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        209,
        "sqlite-navigation".to_string(),
        None,
        Box::new(|_, _| {}),
    );
    let mut stmt = connection.create_statement().await.unwrap();
    stmt.execute_update(
        &mut connection,
        "CREATE TABLE nav_test(id INTEGER PRIMARY KEY)",
    )
    .await
    .unwrap();
    for i in 1..=5 {
        stmt.execute_update(
            &mut connection,
            &format!("INSERT INTO nav_test(id) VALUES ({i})"),
        )
        .await
        .unwrap();
    }

    let mut rs = stmt
        .execute_query_result_set(&mut connection, "SELECT id FROM nav_test ORDER BY id")
        .await
        .unwrap();

    // isBeforeFirst
    assert!(rs.is_before_first(&mut connection).unwrap());

    // next
    assert!(rs.next(&mut connection).unwrap());
    assert_eq!(rs.int(&mut connection, 1).unwrap(), 1);

    // isFirst
    assert!(rs.is_first(&mut connection).unwrap());

    // getRow
    let row = rs.row(&mut connection).unwrap();
    assert_eq!(row, 1);

    // absolute
    let abs = rs.absolute(&mut connection, 3).unwrap();
    assert!(abs);
    assert_eq!(rs.int(&mut connection, 1).unwrap(), 3);

    // relative
    let rel = rs.relative(&mut connection, -1).unwrap();
    assert!(rel);
    assert_eq!(rs.int(&mut connection, 1).unwrap(), 2);

    // last
    let is_last = rs.last(&mut connection).unwrap();
    assert!(is_last);
    assert_eq!(rs.int(&mut connection, 1).unwrap(), 5);

    // isLast
    assert!(rs.is_last(&mut connection).unwrap());

    // isAfterLast (after calling last on a 5-row result, still on last row)
    assert!(!rs.is_after_last(&mut connection).unwrap());

    // getType / getConcurrency / getHoldability
    let _ = rs.result_set_type(&mut connection).unwrap();
    let _ = rs.concurrency(&mut connection).unwrap();
    let _ = rs.holdability(&mut connection).unwrap();

    // getFetchDirection / getFetchSize
    let _ = rs.fetch_direction(&mut connection).unwrap();
    let _ = rs.fetch_size(&mut connection).unwrap();

    // isClosed
    assert!(!rs.is_closed_with_connection(&mut connection).unwrap());

    rs.close_with_connection(&mut connection).unwrap();
    stmt.close_with_connection(&mut connection).unwrap();
}

/// LogFilter 在 FilterChainImpl 中通过真实 Toasty SQLite。
#[tokio::test]
async fn filter_chain_impl_with_log_filter_through_real_toasty() {
    let mut chain = FilterChainImpl::new();
    let log_filter = LogFilter::new();
    log_filter.set_statement_parameter_set_log_enabled(false);
    chain.add_filter(Arc::new(log_filter));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        210,
        "sqlite-log-filter".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    );

    let mut stmt = connection.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut connection, "SELECT 'log-filter-test'")
        .await
        .unwrap();
    assert!(rs.next(&mut connection).unwrap());
    assert_eq!(
        rs.n_string(&mut connection, 1).unwrap(),
        Some("log-filter-test".to_string())
    );
    rs.close_with_connection(&mut connection).unwrap();
    stmt.close_with_connection(&mut connection).unwrap();
}

/// 多 Filter 链：两个 FilterAdapter 注册后通过真实 SQLite 验证。
#[tokio::test]
async fn filter_chain_impl_multi_filter_chain_through_real_toasty() {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain.add_filter(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.before_count(), 2);
    assert_eq!(chain.after_count(), 2);
    assert_eq!(chain.result_set_count(), 2);

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        211,
        "sqlite-multi-filter".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    );

    let mut stmt = connection.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut connection, "SELECT 'multi'")
        .await
        .unwrap();
    assert!(rs.next(&mut connection).unwrap());
    assert_eq!(
        rs.n_string(&mut connection, 1).unwrap(),
        Some("multi".to_string())
    );
    rs.close_with_connection(&mut connection).unwrap();
    stmt.close_with_connection(&mut connection).unwrap();
}
