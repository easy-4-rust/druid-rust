//! 属性读取失败后的语句缓存资格、一次关闭及替代句柄复用。
mod standard_test_support;

use druid::core::{
    BeforeFilter, DruidError, ExecContext, FilterChain, SqlException, StatementPropertyEvent,
};
use druid::pool::{config::PoolInnerConfig, DruidPool};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{Statement, StatementOptions};
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

/// 对应 Java: Filter 的属性 getter；仅在目标逻辑句柄创建后启用故障。
struct PropertyFailureFilter {
    target: StatementPropertyEvent,
    enabled: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl BeforeFilter for PropertyFailureFilter {
    fn name(&self) -> &str {
        "cache-property-failure"
    }
    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
    fn standard_statement_property_read(
        &self,
        event: &StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        if event == &self.target && self.enabled.load(Ordering::Acquire) {
            self.calls.fetch_add(1, Ordering::AcqRel);
            return Err(DruidError::SqlException(Box::new(SqlException::new(
                0, None, None,
            ))));
        }
        Ok(())
    }
}

fn property_cases() -> [(StatementPropertyEvent, &'static str); 8] {
    [
        (
            StatementPropertyEvent::GetMaxFieldSize,
            "statement_max_field_size",
        ),
        (StatementPropertyEvent::GetMaxRows, "statement_max_rows"),
        (
            StatementPropertyEvent::GetQueryTimeout,
            "statement_query_timeout",
        ),
        (
            StatementPropertyEvent::GetFetchDirection,
            "statement_fetch_direction",
        ),
        (StatementPropertyEvent::GetFetchSize, "statement_fetch_size"),
        (
            StatementPropertyEvent::GetResultSetType,
            "statement_result_set_type",
        ),
        (
            StatementPropertyEvent::GetResultSetConcurrency,
            "statement_result_set_concurrency",
        ),
        (
            StatementPropertyEvent::GetResultSetHoldability,
            "statement_result_set_holdability",
        ),
    ]
}

fn read_property(
    statement: &dyn Statement,
    event: &StatementPropertyEvent,
) -> Result<(), RdbcError> {
    match event {
        StatementPropertyEvent::GetMaxFieldSize => statement.max_field_size().map(|_| ()),
        StatementPropertyEvent::GetMaxRows => statement.max_rows().map(|_| ()),
        StatementPropertyEvent::GetQueryTimeout => statement.query_timeout().map(|_| ()),
        StatementPropertyEvent::GetFetchDirection => statement.fetch_direction().map(|_| ()),
        StatementPropertyEvent::GetFetchSize => statement.fetch_size().map(|_| ()),
        StatementPropertyEvent::GetResultSetType => statement.result_set_type().map(|_| ()),
        StatementPropertyEvent::GetResultSetConcurrency => {
            statement.result_set_concurrency().map(|_| ())
        }
        StatementPropertyEvent::GetResultSetHoldability => {
            statement.result_set_holdability().map(|_| ())
        }
        other => panic!("not a property case: {other:?}"),
    }
}

async fn prepare(connection: &mut dyn Connection, callable: bool) -> Box<dyn Statement> {
    if callable {
        connection
            .prepare_call("select 1", StatementOptions::default())
            .await
            .unwrap()
    } else {
        connection
            .prepare_statement("select 1", StatementOptions::default())
            .await
            .unwrap()
    }
}

fn prepare_count(state: &TrackingState) -> usize {
    state
        .event_snapshot()
        .iter()
        .filter(|event| event.starts_with("connection.prepare"))
        .count()
}

async fn verify_property_failures(callable: bool) {
    let mut executed = 0;
    for cached_checkout in [true, false] {
        for filter_error in [true, false] {
            for (event, operation) in property_cases() {
                let state = TrackingState::configured();
                let enabled = Arc::new(AtomicBool::new(false));
                let calls = Arc::new(AtomicUsize::new(0));
                let mut filters = FilterChain::new();
                filters.add_before(Arc::new(PropertyFailureFilter {
                    target: event.clone(),
                    enabled: Arc::clone(&enabled),
                    calls: Arc::clone(&calls),
                }));
                let pool = DruidPool::new_standard(
                    format!("cache-property-{callable}-{operation}"),
                    "mock".to_owned(),
                    Arc::new(TrackingFactory::new(Arc::clone(&state))),
                    PoolInnerConfig {
                        max_open: 1,
                        max_idle: 1,
                        pool_prepared_statements: true,
                        max_pool_prepared_statements_per_connection: 3,
                        ..PoolInnerConfig::default()
                    },
                    Some(Arc::new(filters)),
                );
                pool.init().await.unwrap();
                let mut connection = pool.get().await.unwrap();
                if cached_checkout {
                    let mut seed = prepare(&mut connection, callable).await;
                    seed.close().await.unwrap();
                    assert_eq!(pool.state().cached_prepared_statement_count, 1);
                }
                let mut statement = prepare(&mut connection, callable).await;
                assert_eq!(
                    prepare_count(&state),
                    1,
                    "cache hit must reuse its actual raw statement"
                );
                assert_eq!(pool.state().cached_prepared_statement_count, 0);
                assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 0);
                assert_eq!(pool.stat_value_and_reset().error_count, 0);

                if filter_error {
                    enabled.store(true, Ordering::Release);
                } else {
                    state.fail_next(operation);
                }
                let error = read_property(statement.as_ref(), &event).unwrap_err();
                let sql = error.sql_exception().expect("original SQL exception");
                assert_eq!(sql.class_name(), "java.sql.SQLException");
                assert_eq!(sql.error_code(), 0);
                assert_eq!(sql.sql_state(), None);
                let expected_message =
                    (!filter_error).then(|| format!("forced {operation} SQL failure"));
                assert_eq!(sql.message(), expected_message.as_deref());
                assert_eq!(
                    pool.stat_value_and_reset().error_count,
                    1,
                    "classify the property error once"
                );
                assert_eq!(calls.load(Ordering::Acquire), usize::from(filter_error));
                enabled.store(false, Ordering::Release);
                statement.close().await.unwrap();
                statement.close().await.unwrap();
                // 保留已有 Java 缓存计数规则；首次创建失败与缓存检出失败的计数不同。
                assert_eq!(pool.state().cached_prepared_statement_count, if cached_checkout { 0 } else { -1 },
                    "failed getter must not return raw handle to cache: {event:?}, callable={callable}, filter={filter_error}");
                assert_eq!(pool.state().cached_prepared_statement_delete_count, 1);
                assert_eq!(pool.state().closed_prepared_statement_count, 1);
                assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 1);
                assert_eq!(
                    pool.stat_value_and_reset().error_count,
                    0,
                    "close must not classify again"
                );

                let mut replacement = prepare(&mut connection, callable).await;
                assert_eq!(
                    prepare_count(&state),
                    2,
                    "same SQL requires a new raw statement after property failure"
                );
                read_property(replacement.as_ref(), &event).unwrap();
                replacement.close().await.unwrap();
                let mut healthy_hit = prepare(&mut connection, callable).await;
                assert_eq!(
                    prepare_count(&state),
                    2,
                    "healthy replacement remains cacheable"
                );
                healthy_hit.close().await.unwrap();
                assert_eq!(
                    state.factory_create_count.load(Ordering::Acquire),
                    1,
                    "nonfatal error must retain physical connection"
                );
                connection.close().await.unwrap();
                pool.close().await;
                assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 2);
                executed += 1;
            }
        }
    }
    assert_eq!(
        executed, 32,
        "eight getters x two error sources x two cache paths"
    );
}

#[tokio::test]
async fn prepared_property_errors_evict_failed_handles_and_reuse_healthy_replacements() {
    verify_property_failures(false).await;
}

#[tokio::test]
async fn callable_property_errors_evict_failed_handles_and_reuse_healthy_replacements() {
    verify_property_failures(true).await;
}
