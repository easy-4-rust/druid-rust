//! DruidDataSourceFactory property parsing differential coverage tests (Java Druid 1.2.28).
//!
//! Expands data_source_factory_differential_test.rs with more property parsing
//! branches: parse_transaction_isolation variants, wall_config_from_properties
//! full boolean set, parse_connection_properties edges, successful creation with
//! various property combinations.

extern crate druid_core as druid;
use druid::core::DruidError;
use druid::pool::DruidDataSourceFactory;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::collections::HashMap;
use std::sync::Arc;

fn expect_err<T>(result: Result<T, DruidError>) -> DruidError {
    match result {
        Ok(_) => panic!("expected error, got Ok"),
        Err(e) => e,
    }
}

async fn toasty_factory() -> Arc<ToastyConnectionFactory> {
    Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    )
}

/// Helper: create data source and verify it's alive.
async fn create_ok(props: &HashMap<String, String>) {
    let factory = toasty_factory().await;
    let ds = DruidDataSourceFactory::create_data_source_with_factory(props, factory, "sqlite")
        .await
        .unwrap();
    assert!(!ds.is_closed());
}

// ===========================================================================
// 1. parse_transaction_isolation full branch coverage
// ===========================================================================

#[tokio::test]
async fn factory_isolation_none() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultTransactionIsolation".to_owned(), "NONE".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_isolation_read_uncommitted() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "defaultTransactionIsolation".to_owned(),
        "READ_UNCOMMITTED".to_owned(),
    );
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_isolation_repeatable_read() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "defaultTransactionIsolation".to_owned(),
        "REPEATABLE_READ".to_owned(),
    );
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_isolation_numeric_value() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultTransactionIsolation".to_owned(), "2".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_isolation_invalid_numeric() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultTransactionIsolation".to_owned(), "999".to_owned());
    let factory = toasty_factory().await;
    let e = expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
    match e {
        DruidError::InvalidArgument(msg) => {
            assert!(msg.contains("defaultTransactionIsolation"), "msg: {msg}")
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

// ===========================================================================
// 2. wall_config_from_properties full boolean set
// ===========================================================================

#[tokio::test]
async fn factory_wall_all_boolean_properties() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    let wall_bools = [
        "druid.wall.selectAllow",
        "druid.wall.selectAllColumnAllow",
        "druid.wall.selectIntoAllow",
        "druid.wall.insertAllow",
        "druid.wall.updateAllow",
        "druid.wall.deleteAllow",
        "druid.wall.dropTableAllow",
        "druid.wall.truncateAllow",
        "druid.wall.alterTableAllow",
        "druid.wall.createTableAllow",
        "druid.wall.commitAllow",
        "druid.wall.rollbackAllow",
        "druid.wall.startTransactionAllow",
        "druid.wall.setAllow",
        "druid.wall.updateWhereAlwayTrueCheck",
        "druid.wall.deleteWhereAlwayTrueCheck",
        "druid.wall.selectWhereAlwayTrueCheck",
        "druid.wall.selectHavingAlwayTrueCheck",
        "druid.wall.updateMustHaveWhere",
        "druid.wall.deleteMustHaveWhere",
        "druid.wall.multiStatementAllow",
        "druid.wall.commentAllow",
        "druid.wall.mustParameterized",
        "druid.wall.limitZeroAllow",
        "druid.wall.noneBaseStatementAllow",
    ];
    for key in &wall_bools {
        props.insert((*key).to_owned(), "true".to_owned());
    }
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_wall_invalid_boolean_property() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.wall.selectAllow".to_owned(), "not_bool".to_owned());
    let factory = toasty_factory().await;
    let e = expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
    match e {
        DruidError::InvalidArgument(msg) => {
            assert!(msg.contains("druid.wall.selectAllow"), "msg: {msg}")
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

// ===========================================================================
// 3. Successful creation with various property combinations
// ===========================================================================

#[tokio::test]
async fn factory_creates_with_max_active() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxActive".to_owned(), "10".to_owned());
    let factory = toasty_factory().await;
    let ds = DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite")
        .await
        .unwrap();
    assert_eq!(ds.state().max_open, 10);
}

#[tokio::test]
async fn factory_creates_with_name() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("name".to_owned(), "my-datasource".to_owned());
    let factory = toasty_factory().await;
    let ds = DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite")
        .await
        .unwrap();
    assert_eq!(ds.state().name, "my-datasource");
}

#[tokio::test]
async fn factory_creates_with_login_timeout() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("loginTimeout".to_owned(), "10".to_owned());
    let factory = toasty_factory().await;
    let ds = DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite")
        .await
        .unwrap();
    assert_eq!(ds.login_timeout(), 10);
}

#[tokio::test]
async fn factory_creates_with_reset_stat_enable_false() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.resetStatEnable".to_owned(), "false".to_owned());
    let factory = toasty_factory().await;
    let ds = DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite")
        .await
        .unwrap();
    assert!(!ds.is_reset_stat_enable());
}

// ===========================================================================
// 4. connectionProperties edge cases
// ===========================================================================

#[tokio::test]
async fn factory_connection_properties_single_entry() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("connectionProperties".to_owned(), "key=value".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_connection_properties_no_equals() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("connectionProperties".to_owned(), "just_a_key".to_owned());
    create_ok(&props).await;
}

// ===========================================================================
// 5. Various property combinations - just verify successful creation
// ===========================================================================

#[tokio::test]
async fn factory_default_auto_commit_false() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultAutoCommit".to_owned(), "false".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_default_read_only_true() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultReadOnly".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_query_timeout() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("queryTimeout".to_owned(), "30".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_remove_abandoned() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("removeAbandoned".to_owned(), "true".to_owned());
    props.insert("removeAbandonedTimeout".to_owned(), "60".to_owned());
    props.insert("logAbandoned".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_max_wait() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxWait".to_owned(), "5000".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_max_wait_zero() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxWait".to_owned(), "0".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_time_between_eviction_runs() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "timeBetweenEvictionRunsMillis".to_owned(),
        "30000".to_owned(),
    );
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_keep_alive() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("keepAlive".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_test_on_borrow() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("testOnBorrow".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_test_on_return() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("testOnReturn".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_test_while_idle() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("testWhileIdle".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_default_catalog() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultCatalog".to_owned(), "my_catalog".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_init_connection_sqls() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "druid.initConnectionSqls".to_owned(),
        "SELECT 1;SELECT 2".to_owned(),
    );
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_max_idle() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxIdle".to_owned(), "5".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_pool_prepared_statements() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("poolPreparedStatements".to_owned(), "true".to_owned());
    props.insert("maxOpenPreparedStatements".to_owned(), "20".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_share_prepared_statements() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("sharePreparedStatements".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_clear_filters_enable() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.clearFiltersEnable".to_owned(), "false".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_load_spi_filter_skip() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.loadSpifilterSkip".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_async_init() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.asyncInit".to_owned(), "true".to_owned());
    create_ok(&props).await;
}

#[tokio::test]
async fn factory_init_exception_throw() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.initExceptionThrow".to_owned(), "false".to_owned());
    create_ok(&props).await;
}

// ===========================================================================
// 6. resolve_config_properties
// ===========================================================================

#[tokio::test]
async fn factory_resolve_config_returns_cloned() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxActive".to_owned(), "10".to_owned());
    let resolved = DruidDataSourceFactory::resolve_config_properties(&props)
        .await
        .unwrap();
    assert_eq!(resolved.get("url").unwrap(), "sqlite::memory:");
    assert_eq!(resolved.get("maxActive").unwrap(), "10");
}

// ===========================================================================
// 7. db_type inference
// ===========================================================================

#[tokio::test]
async fn factory_explicit_db_type() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("dbType".to_owned(), "sqlite".to_owned());
    create_ok(&props).await;
}

// ===========================================================================
// 8. init=true triggers auto-initialization
// ===========================================================================

#[tokio::test]
async fn factory_init_true_triggers_initialization() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("init".to_owned(), "true".to_owned());
    let factory = toasty_factory().await;
    let ds = DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite")
        .await
        .unwrap();
    assert!(ds.is_initialized());
}
