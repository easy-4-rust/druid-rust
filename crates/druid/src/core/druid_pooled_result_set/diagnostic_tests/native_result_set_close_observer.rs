use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// 观察原查询的同一原生游标关闭状态与次数，不替换执行后端或合成关闭。
/// 对应 Java: java.sql.ResultSet#close/isClosed。
pub(super) struct NativeResultSetCloseObserver {
    inner: Box<dyn ResultSet>,
    closed_observed: Arc<AtomicBool>,
    close_calls: Arc<AtomicUsize>,
    original_identity: usize,
}
impl NativeResultSetCloseObserver {
    /// 参数为原游标、共享关闭观察、计数及原身份，返回仅委托的观察句柄。
    pub(super) fn new(
        inner: Box<dyn ResultSet>,
        closed_observed: Arc<AtomicBool>,
        close_calls: Arc<AtomicUsize>,
        original_identity: usize,
    ) -> Self {
        Self {
            inner,
            closed_observed,
            close_calls,
            original_identity,
        }
    }
}
druid_rdbc::impl_standard_wrapper!(NativeResultSetCloseObserver => ResultSet);
#[async_trait::async_trait]
impl ResultSet for NativeResultSetCloseObserver {
    fn known_row_count(&self) -> Option<u64> {
        self.inner.known_row_count()
    }
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.inner.next().await
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        assert_eq!(
            std::ptr::from_ref::<dyn ResultSet>(self.inner.as_ref()).cast::<()>() as usize,
            self.original_identity
        );
        self.close_calls.fetch_add(1, Ordering::AcqRel);
        self.inner.close().await?;
        self.closed_observed
            .store(self.inner.is_closed()?, Ordering::Release);
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.inner.is_closed()
    }
}
