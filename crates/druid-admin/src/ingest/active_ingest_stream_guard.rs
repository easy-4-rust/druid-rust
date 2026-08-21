use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 一个 owned ingest stream 处理任务的活动计数守卫。
pub(crate) struct ActiveIngestStreamGuard(Arc<AtomicUsize>);

impl ActiveIngestStreamGuard {
    /// 登记一个新处理任务；Drop 时严格减一。
    pub(crate) fn new(active_streams: Arc<AtomicUsize>) -> Self {
        active_streams.fetch_add(1, Ordering::AcqRel);
        Self(active_streams)
    }
}

impl Drop for ActiveIngestStreamGuard {
    fn drop(&mut self) {
        let previous = self.0.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "active ingest stream counter underflow");
    }
}
