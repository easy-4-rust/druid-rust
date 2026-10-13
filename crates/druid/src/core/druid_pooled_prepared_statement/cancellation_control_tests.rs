use super::{PreparedStatementCancellation, PreparedStatementResource};
use crate::core::PreparedStatementCacheStats;
use async_trait::async_trait;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{PreparedStatement, Statement, StatementCancellation};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

struct CountingCancellation(Arc<AtomicUsize>);

#[async_trait]
impl StatementCancellation for CountingCancellation {
    async fn cancel(&self) -> Result<(), RdbcError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct ReusablePreparedStatement {
    cancellation_count: Arc<AtomicUsize>,
    closed: AtomicBool,
}

impl Wrapper for ReusablePreparedStatement {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }

    fn canonical_unwrap(&self, _target: TypeId) -> Option<StandardUnwrapped<'_>> {
        None
    }
}

#[async_trait]
impl Statement for ReusablePreparedStatement {
    fn cancellation_control(&self) -> Result<Arc<dyn StatementCancellation>, RdbcError> {
        Ok(Arc::new(CountingCancellation(Arc::clone(
            &self.cancellation_count,
        ))))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }
}

impl PreparedStatement for ReusablePreparedStatement {}

fn cancellation_for(resource: &Arc<PreparedStatementResource>) -> Arc<dyn StatementCancellation> {
    let control = resource
        .statement
        .try_lock()
        .expect("test must not hold the statement lock")
        .as_ref()
        .expect("statement must remain owned")
        .cancellation_control()
        .unwrap();
    Arc::new(PreparedStatementCancellation {
        resource: Arc::downgrade(resource),
        control,
    })
}

#[tokio::test]
async fn old_cancellation_handle_cannot_reach_prepared_statement_after_cache_reuse() {
    let cancellation_count = Arc::new(AtomicUsize::new(0));
    let stats = Arc::new(PreparedStatementCacheStats::default());
    let old_resource = PreparedStatementResource::new(
        Box::new(ReusablePreparedStatement {
            cancellation_count: Arc::clone(&cancellation_count),
            closed: AtomicBool::new(false),
        }),
        Arc::clone(&stats),
        false,
    );
    let old_cancellation = cancellation_for(&old_resource);

    // 逻辑关闭使旧 owner 的 gate 失效，并把底层对象交回缓存；同一个 raw handle
    // 随后被包装进新的逻辑 owner，模拟 PreparedStatement cache checkout。
    *old_resource.cancellation_gate.lock().await = false;
    let raw = old_resource.statement.lock().await.take().unwrap();
    old_resource.closed.store(true, Ordering::Release);
    let new_resource = PreparedStatementResource::new(raw, stats, true);
    let new_cancellation = cancellation_for(&new_resource);

    assert!(matches!(
        old_cancellation.cancel().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert_eq!(cancellation_count.load(Ordering::SeqCst), 0);

    new_cancellation
        .cancel()
        .await
        .expect("新租约控制句柄仍可取消复用后的 statement");
    assert_eq!(cancellation_count.load(Ordering::SeqCst), 1);
}
