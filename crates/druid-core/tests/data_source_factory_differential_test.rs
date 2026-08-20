extern crate druid_core as druid;
use druid::core::DruidError;
use druid::pool::DruidDataSourceFactory;
use std::collections::HashMap;
use std::sync::Arc;

fn expect_err<T>(result: Result<T, DruidError>) -> DruidError {
    match result {
        Ok(_) => panic!("expected error, got Ok"),
        Err(e) => e,
    }
}

#[tokio::test]
async fn factory_resolve_config_no_filter() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "jdbc:mysql://localhost/db".to_owned());
    let resolved = DruidDataSourceFactory::resolve_config_properties(&props)
        .await
        .unwrap();
    assert_eq!(resolved.get("url").unwrap(), "jdbc:mysql://localhost/db");
}

#[tokio::test]
async fn factory_missing_url() {
    let props = HashMap::new();
    let e = expect_err(DruidDataSourceFactory::create_data_source(&props).await);
    match e {
        DruidError::InvalidArgument(msg) => assert!(msg.contains("url"), "msg: {msg}"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[tokio::test]
async fn factory_toasty_rejects_credentials() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "jdbc:mysql://localhost/db".to_owned());
    props.insert("username".to_owned(), "root".to_owned());
    let e = expect_err(DruidDataSourceFactory::create_data_source(&props).await);
    match e {
        DruidError::InvalidArgument(msg) => {
            assert!(
                msg.contains("username") || msg.contains("credentials"),
                "msg: {msg}"
            )
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[tokio::test]
async fn factory_toasty_unsupported_url() {
    let mut props = HashMap::new();
    props.insert(
        "url".to_owned(),
        "jdbc:oracle:thin:@localhost:1521:xe".to_owned(),
    );
    let result = DruidDataSourceFactory::create_data_source(&props).await;
    assert!(result.is_err(), "unsupported URL should fail");
}

#[test]
fn factory_property_constants() {
    assert_eq!(DruidDataSourceFactory::PROP_URL, "url");
    assert_eq!(DruidDataSourceFactory::PROP_USERNAME, "username");
    assert_eq!(DruidDataSourceFactory::PROP_PASSWORD, "password");
    assert_eq!(
        DruidDataSourceFactory::PROP_DRIVER_CLASS_NAME,
        "driverClassName"
    );
    assert_eq!(DruidDataSourceFactory::PROP_DB_TYPE, "dbType");
    assert_eq!(DruidDataSourceFactory::PROP_MAX_ACTIVE, "maxActive");
    assert_eq!(DruidDataSourceFactory::PROP_MAX_IDLE, "maxIdle");
    assert_eq!(DruidDataSourceFactory::PROP_MIN_IDLE, "minIdle");
    assert_eq!(DruidDataSourceFactory::PROP_INITIAL_SIZE, "initialSize");
    assert_eq!(DruidDataSourceFactory::PROP_MAX_WAIT, "maxWait");
    assert_eq!(DruidDataSourceFactory::PROP_NAME, "name");
    assert_eq!(DruidDataSourceFactory::PROP_INIT, "init");
    assert_eq!(DruidDataSourceFactory::PROP_FILTERS, "filters");
    assert_eq!(
        DruidDataSourceFactory::PROP_REMOVE_ABANDONED,
        "removeAbandoned"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_REMOVE_ABANDONED_TIMEOUT,
        "removeAbandonedTimeout"
    );
    assert_eq!(DruidDataSourceFactory::PROP_LOG_ABANDONED, "logAbandoned");
    assert_eq!(DruidDataSourceFactory::PROP_KEEP_ALIVE, "keepAlive");
    assert_eq!(
        DruidDataSourceFactory::PROP_KEEP_ALIVE_BETWEEN_TIME_MILLIS,
        "keepAliveBetweenTimeMillis"
    );
    assert_eq!(DruidDataSourceFactory::PROP_TEST_ON_BORROW, "testOnBorrow");
    assert_eq!(DruidDataSourceFactory::PROP_TEST_ON_RETURN, "testOnReturn");
    assert_eq!(
        DruidDataSourceFactory::PROP_TEST_WHILE_IDLE,
        "testWhileIdle"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_VALIDATION_QUERY,
        "validationQuery"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_VALIDATION_QUERY_TIMEOUT,
        "validationQueryTimeout"
    );
    assert_eq!(DruidDataSourceFactory::PROP_QUERY_TIMEOUT, "queryTimeout");
    assert_eq!(
        DruidDataSourceFactory::PROP_TRANSACTION_QUERY_TIMEOUT,
        "transactionQueryTimeout"
    );
    assert_eq!(DruidDataSourceFactory::PROP_LOGIN_TIMEOUT, "loginTimeout");
    assert_eq!(
        DruidDataSourceFactory::PROP_DEFAULT_AUTO_COMMIT,
        "defaultAutoCommit"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_DEFAULT_READ_ONLY,
        "defaultReadOnly"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_DEFAULT_TRANSACTION_ISOLATION,
        "defaultTransactionIsolation"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_DEFAULT_CATALOG,
        "defaultCatalog"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_POOL_PREPARED_STATEMENTS,
        "poolPreparedStatements"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_MAX_OPEN_PREPARED_STATEMENTS,
        "maxOpenPreparedStatements"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_SHARE_PREPARED_STATEMENTS,
        "sharePreparedStatements"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_USE_ORACLE_IMPLICIT_CACHE,
        "useOracleImplicitCache"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_PHY_TIMEOUT_MILLIS,
        "phyTimeoutMillis"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_PHY_MAX_USE_COUNT,
        "phyMaxUseCount"
    );
    assert_eq!(DruidDataSourceFactory::PROP_ASYNC_INIT, "druid.asyncInit");
    assert_eq!(
        DruidDataSourceFactory::PROP_INIT_EXCEPTION_THROW,
        "druid.initExceptionThrow"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_RESET_STAT_ENABLE,
        "druid.resetStatEnable"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_NOT_FULL_TIMEOUT_RETRY_COUNT,
        "druid.notFullTimeoutRetryCount"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_MAX_WAIT_THREAD_COUNT,
        "druid.maxWaitThreadCount"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_CONNECTION_ERROR_RETRY_ATTEMPTS,
        "druid.connectionErrorRetryAttempts"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_BREAK_AFTER_ACQUIRE_FAILURE,
        "druid.breakAfterAcquireFailure"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_TIME_BETWEEN_CONNECT_ERROR_MILLIS,
        "druid.timeBetweenConnectErrorMillis"
    );
    assert_eq!(DruidDataSourceFactory::PROP_FAIL_FAST, "druid.failFast");
    assert_eq!(
        DruidDataSourceFactory::PROP_ON_FATAL_ERROR_MAX_ACTIVE,
        "druid.onFatalErrorMaxActive"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_TIME_BETWEEN_EVICTION_RUNS_MILLIS,
        "timeBetweenEvictionRunsMillis"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_MIN_EVICTABLE_IDLE_TIME_MILLIS,
        "minEvictableIdleTimeMillis"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_MAX_EVICTABLE_IDLE_TIME_MILLIS,
        "maxEvictableIdleTimeMillis"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_STAT_SQL_MAX_SIZE,
        "druid.stat.sql.MaxSize"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_INIT_VARIANTS,
        "druid.initVariants"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_INIT_GLOBAL_VARIANTS,
        "druid.initGlobalVariants"
    );
    assert_eq!(
        DruidDataSourceFactory::PROP_INIT_CONNECTION_SQLS,
        "druid.initConnectionSqls"
    );
}

#[tokio::test]
async fn factory_invalid_boolean_property() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultAutoCommit".to_owned(), "not_a_bool".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let e = expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
    match e {
        DruidError::InvalidArgument(msg) => {
            assert!(msg.contains("defaultAutoCommit"), "msg: {msg}")
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[tokio::test]
async fn factory_invalid_integer_property() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxActive".to_owned(), "not_a_number".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let e = expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
    match e {
        DruidError::InvalidArgument(msg) => assert!(msg.contains("maxActive"), "msg: {msg}"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[tokio::test]
async fn factory_negative_max_wait() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxWait".to_owned(), "-1".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_negative_validation_query_timeout() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("validationQueryTimeout".to_owned(), "-5".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let e = expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
    match e {
        DruidError::InvalidArgument(msg) => {
            assert!(msg.contains("validationQueryTimeout"), "msg: {msg}")
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[tokio::test]
async fn factory_negative_remove_abandoned_timeout() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("removeAbandonedTimeout".to_owned(), "-10".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let e = expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
    match e {
        DruidError::InvalidArgument(msg) => {
            assert!(msg.contains("removeAbandonedTimeout"), "msg: {msg}")
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[tokio::test]
async fn factory_connection_properties() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "connectionProperties".to_owned(),
        "key1=value1;key2=value2".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_connection_properties_empty_entries() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "connectionProperties".to_owned(),
        "key1=value1;;key2=value2;".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_wall_config_from_properties() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.wall.selectAllow".to_owned(), "false".to_owned());
    props.insert("druid.wall.dropTableAllow".to_owned(), "false".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_wall_config_legacy_spelling() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.wall.selelctAllow".to_owned(), "false".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_wall_config_tenant() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.wall.tenantColumn".to_owned(), "tenant_id".to_owned());
    props.insert(
        "druid.wall.tenantTablePattern".to_owned(),
        "t_.*".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_multiple_boolean_properties() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("testOnBorrow".to_owned(), "true".to_owned());
    props.insert("testOnReturn".to_owned(), "false".to_owned());
    props.insert("testWhileIdle".to_owned(), "true".to_owned());
    props.insert("removeAbandoned".to_owned(), "true".to_owned());
    props.insert("logAbandoned".to_owned(), "true".to_owned());
    props.insert("keepAlive".to_owned(), "true".to_owned());
    props.insert("poolPreparedStatements".to_owned(), "true".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_max_wait_thread_count_leq_zero() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.maxWaitThreadCount".to_owned(), "-1".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_connection_error_retry_attempts_leq_zero() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "druid.connectionErrorRetryAttempts".to_owned(),
        "0".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_phy_max_use_count_negative() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("phyMaxUseCount".to_owned(), "-1".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_invalid_on_fatal_error_max_active() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "druid.onFatalErrorMaxActive".to_owned(),
        "99999999999999".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
}

#[tokio::test]
async fn factory_invalid_query_timeout() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("queryTimeout".to_owned(), "99999999999999".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
}

#[tokio::test]
async fn factory_invalid_login_timeout() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("loginTimeout".to_owned(), "99999999999999".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
}

#[tokio::test]
async fn factory_invalid_stat_sql_max_size() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "druid.stat.sql.MaxSize".to_owned(),
        "99999999999999".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
}

#[tokio::test]
async fn factory_invalid_not_full_timeout_retry_count() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "druid.notFullTimeoutRetryCount".to_owned(),
        "99999999999999".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
}

#[tokio::test]
async fn factory_invalid_transaction_query_timeout() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "transactionQueryTimeout".to_owned(),
        "99999999999999".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    expect_err(
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await,
    );
}

#[tokio::test]
async fn factory_connection_properties_no_key() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "connectionProperties".to_owned(),
        "=value;valid=yes".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_boolean_case_insensitive() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultAutoCommit".to_owned(), "TRUE".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_transaction_isolation_read_committed() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "defaultTransactionIsolation".to_owned(),
        "READ_COMMITTED".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_transaction_isolation_serializable() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "defaultTransactionIsolation".to_owned(),
        "SERIALIZABLE".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_transaction_isolation_minus_one() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultTransactionIsolation".to_owned(), "-1".to_owned());
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}

#[tokio::test]
async fn factory_transaction_isolation_invalid_string() {
    use druid::toasty::ToastyConnectionFactory;
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "defaultTransactionIsolation".to_owned(),
        "INVALID_LEVEL".to_owned(),
    );
    let factory = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    );
    let _ =
        DruidDataSourceFactory::create_data_source_with_factory(&props, factory, "sqlite").await;
}
