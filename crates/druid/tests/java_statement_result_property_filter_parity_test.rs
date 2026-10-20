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
use std::sync::Arc;

/// 对应 Java: DruidPooledStatementTest.ErrorFilter；拒绝发生在真实结果状态入口。
struct ResultPropertyFilter {
    reject: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
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
        if matches!(
            event,
            StatementPropertyEvent::GetUpdateCount | StatementPropertyEvent::GetMoreResults
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
        let mut filters = FilterChain::new();
        filters.add_before(Arc::new(ResultPropertyFilter {
            reject: Arc::clone(&reject),
            calls: Arc::clone(&calls),
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
        for operation in 0..2 {
            let error = if operation == 0 {
                statement.get_update_count().unwrap_err()
            } else {
                statement
                    .get_more_results(CurrentResultDisposition::CloseCurrent)
                    .await
                    .unwrap_err()
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
        assert_eq!(calls.load(Ordering::Acquire), 2);
        assert!(
            !cursor.is_closed().unwrap(),
            "filter rejection must retain cursor"
        );
        assert_eq!(
            state.result_set_close_attempt_count.load(Ordering::Acquire),
            closes_before
        );
        statement.close().await.unwrap();
        assert!(statement.get_update_count().is_err());
        assert!(statement
            .get_more_results(CurrentResultDisposition::CloseCurrent)
            .await
            .is_err());
        assert_eq!(
            calls.load(Ordering::Acquire),
            2,
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
