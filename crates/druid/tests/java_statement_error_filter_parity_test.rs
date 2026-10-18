//! 对照 Java Druid 1.2.28 `DruidPooledStatementTest` 的过滤器错误与池回收语义。

mod standard_test_support;

use druid::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecOperation, ExecResult, FilterChain,
    ResultSetFilter, SqlException,
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
