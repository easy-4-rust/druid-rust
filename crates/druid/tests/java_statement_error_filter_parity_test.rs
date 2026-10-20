//! 对照 Java Druid 1.2.28 `DruidPooledStatementTest` 的过滤器错误与池回收语义。

mod standard_test_support;

use druid::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecOperation, ExecResult, FilterChain,
    ResultSetFilter, SqlException, StatementPropertyEvent,
};
use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{
    GeneratedKeys, ParameterIndex, PreparedParameter, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::toasty::ToastyConnectionFactory;
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// 对应 Java: `DruidPooledStatementTest.ErrorFilter` 的普通 SQL 异常注入。
struct StatementErrorFilter;

#[async_trait::async_trait]
impl BeforeFilter for StatementErrorFilter {
    fn name(&self) -> &str {
        "java-statement-error-filter"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        assert!(matches!(
            context.operation,
            ExecOperation::Query
                | ExecOperation::Update
                | ExecOperation::Execute
                | ExecOperation::Batch
        ));
        Err(DruidError::SqlException(Box::new(SqlException::new(
            0,
            None,
            Some("injected statement error".to_owned()),
        ))))
    }
}

#[async_trait::async_trait]
impl AfterFilter for StatementErrorFilter {
    fn name(&self) -> &str {
        "java-statement-error-filter"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }
}

impl ResultSetFilter for StatementErrorFilter {}

/// 对应 Java `DruidPooledStatementTest.ErrorFilter` 的五个属性 getter 拒绝入口。
struct StatementGetterErrorFilter;

#[async_trait::async_trait]
impl BeforeFilter for StatementGetterErrorFilter {
    fn name(&self) -> &str {
        "java-statement-getter-error-filter"
    }

    fn standard_statement_property_read(
        &self,
        event: &StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        if matches!(
            event,
            StatementPropertyEvent::GetMaxFieldSize
                | StatementPropertyEvent::GetMaxRows
                | StatementPropertyEvent::GetQueryTimeout
                | StatementPropertyEvent::GetFetchDirection
                | StatementPropertyEvent::GetFetchSize
        ) {
            return Err(DruidError::SqlException(Box::new(SqlException::new(
                0,
                None,
                Some("injected statement getter error".to_owned()),
            ))));
        }
        Ok(())
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for StatementGetterErrorFilter {
    fn name(&self) -> &str {
        "java-statement-getter-error-filter"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }
}

impl ResultSetFilter for StatementGetterErrorFilter {}

/// 对应 Java `ErrorFilter#statement_addBatch` 的非执行操作异常。
struct StatementBatchAdmissionErrorFilter;

#[async_trait::async_trait]
impl BeforeFilter for StatementBatchAdmissionErrorFilter {
    fn name(&self) -> &str {
        "java-statement-batch-admission-error"
    }

    fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        if sql == "REJECT BATCH" {
            return Err(DruidError::SqlException(Box::new(SqlException::new(
                0,
                None,
                Some("injected batch admission error".to_owned()),
            ))));
        }
        Ok(sql.to_owned())
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
}

#[tokio::test]
async fn java_batch_management_sql_errors_count_once_without_consuming_pending_batch() {
    let state = TrackingState::configured();
    let mut chain = FilterChain::new();
    chain.add_before(Arc::new(StatementBatchAdmissionErrorFilter));
    let pool = DruidPool::new_standard(
        "java-batch-management-parity".to_owned(),
        "mock".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("borrow");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    statement.add_batch("SELECT 1".to_owned()).unwrap();

    let filter_error = statement.add_batch("REJECT BATCH".to_owned()).unwrap_err();
    let sql_error = filter_error.sql_exception().expect("plain SQL exception");
    assert_eq!(sql_error.class_name(), "java.sql.SQLException");
    assert_eq!(sql_error.error_code(), 0);
    assert_eq!(sql_error.sql_state(), None);
    assert_eq!(
        state
            .statement_batch_add_attempt_count
            .load(Ordering::Acquire),
        1
    );

    state.fail_next("statement_add_batch");
    let add_error = statement.add_batch("SELECT 2".to_owned()).unwrap_err();
    state.fail_next("statement_clear_batch");
    let clear_error = statement.clear_batch().unwrap_err();
    for error in [add_error, clear_error] {
        let sql_error = error.sql_exception().expect("driver SQL exception");
        assert_eq!(sql_error.error_code(), 9002);
        assert_eq!(sql_error.sql_state(), Some("08006"));
    }
    assert_eq!(statement.batch().unwrap(), vec!["SELECT 1"]);
    assert_eq!(pool.stat_value_and_reset().error_count, 3);
    assert_eq!(
        state.statement_batch_execute_count.load(Ordering::Acquire),
        0
    );

    statement.clear_batch().expect("clear retry");
    assert!(statement.batch().unwrap().is_empty());
    statement.close().await.expect("statement close");
    connection.close().await.expect("logical close");
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    let mut replacement = pool.get().await.expect("reuse after batch errors");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
    replacement.close().await.expect("replacement close");
    pool.close().await;
}

#[tokio::test]
async fn java_statement_filter_getter_errors_count_once_and_keep_pool_reusable() {
    let state = TrackingState::configured();
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(StatementGetterErrorFilter));
    let pool = DruidPool::new_standard(
        "java-statement-filter-getter-parity".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("borrow");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    for error in [
        statement.max_field_size().unwrap_err(),
        statement.max_rows().unwrap_err(),
        statement.query_timeout().unwrap_err(),
        statement.fetch_direction().unwrap_err(),
        statement.fetch_size().unwrap_err(),
    ] {
        let sql = error.sql_exception().expect("plain SQL exception");
        assert_eq!(sql.class_name(), "java.sql.SQLException");
        assert_eq!(sql.error_code(), 0);
        assert_eq!(sql.sql_state(), None);
    }
    assert_eq!(pool.stat_value_and_reset().error_count, 5);
    assert_eq!(pool.state().active_count, 1);
    assert_eq!(pool.state().idle_count, 0);
    statement.close().await.expect("statement close");
    connection.close().await.expect("logical close");
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    let mut replacement = pool.get().await.expect("reuse after filter getter errors");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
    replacement.close().await.expect("replacement close");
    pool.close().await;
}

#[tokio::test]
async fn java_statement_property_getter_sql_errors_count_once_and_keep_holder_reusable() {
    // 对应 Java 1.2.28 DruidPooledStatementTest 的五个 getter 错误用例。
    let state = TrackingState::configured();
    let pool = DruidPool::new_standard(
        "java-statement-getter-parity".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        None,
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("borrow");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");

    assert_eq!(statement.max_field_size().unwrap(), 0);
    assert_eq!(statement.max_rows().unwrap(), 0);
    assert_eq!(statement.query_timeout().unwrap(), 0);
    assert_eq!(statement.fetch_direction().unwrap().rdbc_code(), 1002);
    assert_eq!(statement.fetch_size().unwrap(), 0);

    state.fail_next("statement_max_field_size");
    assert!(matches!(
        statement.max_field_size(),
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));
    state.fail_next("statement_max_rows");
    assert!(matches!(
        statement.max_rows(),
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));
    state.fail_next("statement_query_timeout");
    assert!(matches!(
        statement.query_timeout(),
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));
    state.fail_next("statement_fetch_direction");
    assert!(matches!(
        statement.fetch_direction(),
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));
    state.fail_next("statement_fetch_size");
    assert!(matches!(
        statement.fetch_size(),
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));
    assert_eq!(pool.stat_value_and_reset().error_count, 5);
    assert_eq!(pool.state().active_count, 1);
    statement.close().await.expect("statement close");
    connection.close().await.expect("connection close");
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    let mut replacement = pool.get().await.expect("same holder remains usable");
    replacement.close().await.expect("replacement close");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
    pool.close().await;
}

#[tokio::test]
async fn java_statement_filter_errors_count_once_and_keep_pool_reusable() {
    let state = TrackingState::configured();
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(StatementErrorFilter));
    let pool = DruidPool::new_standard(
        "java-statement-error-parity".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("borrow");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    assert_eq!(pool.state().active_count, 1);
    assert_eq!(pool.state().idle_count, 0);

    let query_error = statement
        .execute_query("SELECT 1")
        .await
        .err()
        .expect("query must fail at filter");
    let update_error = statement
        .execute_update("UPDATE t SET x = 1", GeneratedKeys::None)
        .await
        .expect_err("update must fail at filter");
    let execute_error = statement
        .execute("SELECT 1", GeneratedKeys::None)
        .await
        .expect_err("execute must fail at filter");
    statement
        .add_batch("INSERT INTO t VALUES (1)".to_owned())
        .expect("add batch");
    let batch_error = statement
        .execute_batch()
        .await
        .expect_err("batch must fail at filter");
    for error in [query_error, update_error, execute_error, batch_error] {
        let sql_exception = error.sql_exception().expect("ordinary SQL exception");
        assert_eq!(sql_exception.class_name(), "java.sql.SQLException");
        assert_eq!(sql_exception.error_code(), 0);
        assert_eq!(sql_exception.sql_state(), None);
    }
    assert_eq!(pool.stat_value_and_reset().error_count, 4);

    Statement::close(statement.as_mut())
        .await
        .expect("statement close");
    connection.close().await.expect("logical close");
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    let mut replacement = pool.get().await.expect("reuse after filter error");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
    replacement.close().await.expect("replacement close");
    pool.close().await;
}

#[tokio::test]
async fn prepared_batch_filter_error_uses_the_same_error_accounting_path() {
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(StatementErrorFilter));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("SQLite factory");
    let pool = DruidPool::new_standard(
        "prepared-batch-filter-error".to_owned(),
        "toasty-sqlite".to_owned(),
        Arc::new(factory),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("borrow");
    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "INSERT INTO t VALUES (?1)",
        StatementOptions::default(),
    )
    .await
    .expect("prepared statement");
    prepared
        .add_parameter_batch(vec![PreparedParameter {
            index: ParameterIndex::new(1).expect("parameter index"),
            value: Value::Int(1),
        }])
        .expect("add prepared batch");

    let error = prepared
        .execute_prepared_batch()
        .await
        .expect_err("filter rejects prepared batch");
    assert_eq!(
        error
            .sql_exception()
            .expect("ordinary SQL exception")
            .class_name(),
        "java.sql.SQLException"
    );
    assert_eq!(pool.stat_value_and_reset().error_count, 1);
    Statement::close(prepared.as_mut())
        .await
        .expect("prepared close");
    connection.close().await.expect("logical close");
    assert_eq!(pool.state().idle_count, 1);
    pool.close().await;
}
