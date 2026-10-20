//! 对照 Java Druid 1.2.28 DruidPooledStatementTest 的警告 SQL 异常及连接回收。

mod standard_test_support;

use druid::core::{
    BeforeFilter, DruidError, ExecContext, FilterChain, SqlException, SqlWarning,
    StandardStatementWarningFilterChain,
};
use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{Statement, StatementOptions};
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// 对应 Java: DruidPooledStatementTest.ErrorFilter；拒绝和改写均发生在真实警告链入口。
struct WarningFilter {
    reject: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
}

impl WarningFilter {
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
impl BeforeFilter for WarningFilter {
    fn name(&self) -> &str {
        "java-warning-filter"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    fn standard_statement_get_warnings(
        &self,
        _chain: &mut StandardStatementWarningFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        self.enter()?;
        let mut warning = SqlWarning::new("first", Some("01000".to_owned()), 7);
        warning.set_next_warning(SqlWarning::new("second", Some("01001".to_owned()), 8));
        Ok(Some(warning))
    }

    fn standard_statement_clear_warnings(
        &self,
        _chain: &mut StandardStatementWarningFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.enter()
    }
}

#[tokio::test]
async fn all_statement_kinds_count_warning_filter_errors_once_and_reject_closed_access() {
    for kind in ["statement", "prepared", "callable"] {
        let state = TrackingState::configured();
        let reject = Arc::new(AtomicBool::new(true));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut filters = FilterChain::new();
        filters.add_before(Arc::new(WarningFilter {
            reject: Arc::clone(&reject),
            calls: Arc::clone(&calls),
        }));
        let pool = DruidPool::new_standard(
            format!("java-warning-{kind}"),
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
        let mut statement: Box<dyn Statement> = match kind {
            "prepared" => connection
                .prepare_statement("SELECT 1", StatementOptions::default())
                .await
                .unwrap(),
            "callable" => connection
                .prepare_call("SELECT 1", StatementOptions::default())
                .await
                .unwrap(),
            _ => connection
                .create_statement(StatementOptions::default())
                .await
                .unwrap(),
        };
        for error in [
            statement.warnings().unwrap_err(),
            statement.clear_warnings().unwrap_err(),
        ] {
            let sql = error.sql_exception().unwrap();
            assert_eq!(sql.class_name(), "java.sql.SQLException");
            assert_eq!(sql.error_code(), 0);
            assert_eq!(sql.sql_state(), None);
        }
        assert_eq!(pool.stat_value_and_reset().error_count, 2, "{kind}");
        assert_eq!(calls.load(Ordering::Acquire), 2);
        reject.store(false, Ordering::Release);
        assert_eq!(
            statement.warning_messages().unwrap(),
            vec!["first", "second"]
        );
        statement.clear_warnings().unwrap();
        assert_eq!(calls.load(Ordering::Acquire), 4);
        statement.close().await.unwrap();
        assert!(statement.warnings().is_err());
        assert!(statement.clear_warnings().is_err());
        assert_eq!(
            calls.load(Ordering::Acquire),
            4,
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
