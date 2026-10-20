//! 对照 Java Druid 1.2.28 DruidPooledStatementTest 的结果状态 SQL 异常及连接回收。

mod standard_test_support;

use druid::core::{
    BeforeFilter, DruidError, ExecContext, FilterChain, SqlException, StatementPropertyEvent,
};
use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{CurrentResultDisposition, Statement, StatementOptions};
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 对应 Java: DruidPooledStatementTest.ErrorFilter；拒绝发生在真实结果状态入口。
struct ResultPropertyFilter {
    reject: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    events: Arc<Mutex<Vec<StatementPropertyEvent>>>,
}

impl ResultPropertyFilter {
    fn enter(&self) -> Result<(), DruidError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        if self.reject.load(Ordering::Acquire) {
            Err(DruidError::SqlException(Box::new(SqlException::new(
                0, None, None,
            ))))
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl BeforeFilter for ResultPropertyFilter {
    fn name(&self) -> &str {
        "java-result-property-filter"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    fn standard_statement_property_read(
        &self,
        event: &StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        self.events.lock().unwrap().push(event.clone());
        if matches!(
            event,
            StatementPropertyEvent::GetUpdateCount
                | StatementPropertyEvent::GetMoreResults
                | StatementPropertyEvent::GetGeneratedKeys
                | StatementPropertyEvent::GetResultSet
                | StatementPropertyEvent::GetResultSetType
                | StatementPropertyEvent::GetResultSetConcurrency
                | StatementPropertyEvent::GetResultSetHoldability
        ) {
            self.enter()?;
        }
        Ok(())
    }
}

#[tokio::test]
async fn all_statement_kinds_count_result_filter_errors_once_and_reject_closed_access() {
    for kind in ["statement", "prepared", "callable"] {
        let state = TrackingState::configured();
        let reject = Arc::new(AtomicBool::new(true));
        let calls = Arc::new(AtomicUsize::new(0));
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        filters.add_before(Arc::new(ResultPropertyFilter {
            reject: Arc::clone(&reject),
            calls: Arc::clone(&calls),
            events: Arc::clone(&events),
        }));
        let pool = DruidPool::new_standard(
            format!("java-result-property-{kind}"),
            "mock".to_owned(),
            Arc::new(TrackingFactory::new(Arc::clone(&state))),
            PoolInnerConfig {
                max_open: 1,
                max_idle: 1,
                ..PoolInnerConfig::default()
            },
            Some(Arc::new(filters)),
        );
        pool.init().await.unwrap();
        let mut connection = pool.get().await.unwrap();
        let (mut statement, cursor): (Box<dyn Statement>, _) = match kind {
            "prepared" => {
                let mut statement = connection
                    .prepare_statement("select 1", StatementOptions::default())
                    .await
                    .unwrap();
                let cursor = statement.execute_prepared_query().await.unwrap();
                (statement, cursor)
            }
            "callable" => {
                let mut statement = connection
                    .prepare_call("select 1", StatementOptions::default())
                    .await
                    .unwrap();
                let cursor = statement.execute_prepared_query().await.unwrap();
                (statement, cursor)
            }
            _ => {
                let mut statement = connection
                    .create_statement(StatementOptions::default())
                    .await
                    .unwrap();
                let cursor = statement.execute_query("select 1").await.unwrap();
                (statement, cursor)
            }
        };
        assert!(!cursor.is_closed().unwrap());
        let closes_before = state.result_set_close_attempt_count.load(Ordering::Acquire);
        for operation in 0..8 {
            let error = if operation == 0 {
                statement.get_update_count().unwrap_err()
            } else if operation == 1 {
                statement
                    .get_more_results(CurrentResultDisposition::CloseCurrent)
                    .await
                    .unwrap_err()
            } else if operation == 2 {
                statement
                    .generated_keys()
                    .await
                    .err()
                    .expect("generated keys filter must reject")
            } else if operation == 3 {
                statement
                    .get_result_set()
                    .await
                    .err()
                    .expect("current cursor filter rejects")
            } else if operation == 4 {
                statement.result_set_type().unwrap_err()
            } else if operation == 5 {
                statement.result_set_concurrency().unwrap_err()
            } else if operation == 6 {
                statement.result_set_holdability().unwrap_err()
            } else {
                statement.get_more_results_current(0).await.unwrap_err()
            };
            let sql = error.sql_exception().unwrap();
            assert_eq!(sql.class_name(), "java.sql.SQLException");
            assert_eq!(sql.error_code(), 0);
            assert_eq!(sql.sql_state(), None);
            assert_eq!(
                pool.stat_value_and_reset().error_count,
                1,
                "{kind}/{operation}"
            );
        }
        assert_eq!(calls.load(Ordering::Acquire), 8);
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                StatementPropertyEvent::GetUpdateCount,
                StatementPropertyEvent::GetMoreResults,
                StatementPropertyEvent::GetGeneratedKeys,
                StatementPropertyEvent::GetResultSet,
                StatementPropertyEvent::GetResultSetType,
                StatementPropertyEvent::GetResultSetConcurrency,
                StatementPropertyEvent::GetResultSetHoldability,
                StatementPropertyEvent::GetMoreResults,
            ],
            "each Java getter must use its own exact filter event"
        );
        assert!(
            !cursor.is_closed().unwrap(),
            "filter rejection must retain cursor"
        );
        assert_eq!(
            state.result_set_close_attempt_count.load(Ordering::Acquire),
            closes_before
        );
        reject.store(false, Ordering::Release);
        // options 默认不支持，实际独立 getter 的非默认值仍必须穿过两层转发。
        assert_eq!(statement.result_set_type().unwrap().rdbc_code(), 1005);
        assert_eq!(
            statement.result_set_concurrency().unwrap().rdbc_code(),
            1008
        );
        assert_eq!(statement.result_set_holdability().unwrap().rdbc_code(), 1);
        assert!(matches!(
            statement.get_more_results_current(0).await,
            Err(druid_rdbc::error::RdbcError::InvalidArgument(_))
        ));
        for operation in [
            "statement_result_set_type",
            "statement_result_set_concurrency",
            "statement_result_set_holdability",
        ] {
            state.fail_next(operation);
            let error = match operation {
                "statement_result_set_type" => statement.result_set_type().unwrap_err(),
                "statement_result_set_concurrency" => {
                    statement.result_set_concurrency().unwrap_err()
                }
                _ => statement.result_set_holdability().unwrap_err(),
            };
            assert_eq!(error.sql_exception().unwrap().error_code(), 0);
            assert_eq!(
                pool.stat_value_and_reset().error_count,
                1,
                "driver SQL error counted once"
            );
        }
        let calls_before_close = calls.load(Ordering::Acquire);
        statement.close().await.unwrap();
        assert!(statement.get_update_count().is_err());
        assert!(statement
            .get_more_results(CurrentResultDisposition::CloseCurrent)
            .await
            .is_err());
        assert!(statement.generated_keys().await.is_err());
        assert!(statement.get_result_set().await.is_err());
        assert!(statement.result_set_type().is_err());
        assert!(statement.result_set_concurrency().is_err());
        assert!(statement.result_set_holdability().is_err());
        assert!(statement.get_more_results_current(0).await.is_err());
        assert_eq!(
            calls.load(Ordering::Acquire),
            calls_before_close,
            "closed access must not enter filters"
        );
        assert_eq!(pool.stat_value_and_reset().error_count, 0);
        connection.close().await.unwrap();
        assert_eq!(pool.state().active_count, 0);
        assert_eq!(pool.state().idle_count, 1);
        let mut next = pool.get().await.unwrap();
        assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
        next.close().await.unwrap();
        pool.close().await;
    }
}
