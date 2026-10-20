//! 对照 Java Druid 1.2.28 ErrorFilter#statement_clearBatch 的错误、包围顺序及回收。

mod standard_test_support;

use druid::core::{
    BeforeFilter, DruidError, ExecContext, FilterChain, SqlException,
    StandardStatementClearBatchFilterChain,
};
use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::StatementOptions;
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 对应 Java: DruidPooledStatementTest.ErrorFilter；记录真实包围调用及可恢复拒绝。
struct ClearBatchFilter {
    reject: Arc<AtomicBool>,
    trace: Arc<Mutex<Vec<&'static str>>>,
    name: &'static str,
}

#[async_trait::async_trait]
impl BeforeFilter for ClearBatchFilter {
    fn name(&self) -> &str {
        self.name
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    fn standard_statement_clear_batch(
        &self,
        chain: &mut StandardStatementClearBatchFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.trace.lock().unwrap().push(self.name);
        if self.reject.load(Ordering::Acquire) {
            return Err(DruidError::SqlException(Box::new(SqlException::new(
                0,
                None,
                Some("injected clear batch error".to_owned()),
            ))));
        }
        let result = chain.statement_clear_batch();
        self.trace.lock().unwrap().push(self.name);
        result
    }
}

#[tokio::test]
async fn clear_batch_filter_preserves_errors_pending_batch_order_and_holder_reuse() {
    let state = TrackingState::configured();
    let reject = Arc::new(AtomicBool::new(true));
    let trace = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for (name, flag) in [
        ("outer", Arc::new(AtomicBool::new(false))),
        ("inner", Arc::clone(&reject)),
    ] {
        filters.add_before(Arc::new(ClearBatchFilter {
            reject: flag,
            trace: Arc::clone(&trace),
            name,
        }));
    }
    let pool = DruidPool::new_standard(
        "java-clear-batch-filter".to_owned(),
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
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.add_batch("SELECT 1".to_owned()).unwrap();
    state.fail_next("statement_clear_batch");
    let rejected = statement.clear_batch().unwrap_err();
    let sql = rejected.sql_exception().unwrap();
    assert_eq!(sql.class_name(), "java.sql.SQLException");
    assert_eq!(sql.error_code(), 0);
    assert_eq!(sql.sql_state(), None);
    assert_eq!(statement.batch().unwrap(), vec!["SELECT 1"]);
    assert_eq!(pool.stat_value_and_reset().error_count, 1);
    assert_eq!(*trace.lock().unwrap(), vec!["outer", "inner", "outer"]);

    reject.store(false, Ordering::Release);
    trace.lock().unwrap().clear();
    // 驱动失败仍待消费，证明先前的过滤器拒绝没有进入底层清除。
    let driver_error = statement.clear_batch().unwrap_err();
    assert_eq!(driver_error.sql_exception().unwrap().error_code(), 9002);
    assert_eq!(statement.batch().unwrap(), vec!["SELECT 1"]);
    // 上次快照已重置计数；本次驱动失败仍恰好增加一项。
    assert_eq!(pool.stat_value_and_reset().error_count, 1);
    assert_eq!(
        *trace.lock().unwrap(),
        vec!["outer", "inner", "inner", "outer"]
    );
    statement.clear_batch().unwrap();
    assert!(statement.batch().unwrap().is_empty());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    let mut next = pool.get().await.unwrap();
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
    next.close().await.unwrap();
    pool.close().await;
}
