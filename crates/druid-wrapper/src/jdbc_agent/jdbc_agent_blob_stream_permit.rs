use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_WORKERS: usize = 64;
static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// 限制 Blob 同步桥接的后台线程数，避免过量流耗尽进程线程。
pub(super) struct JdbcAgentBlobStreamPermit;

impl JdbcAgentBlobStreamPermit {
    /// 保留一个工作线程名额；满额时拒绝打开新的远端流。
    pub(super) fn acquire() -> io::Result<Self> {
        ACTIVE_WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_WORKERS).then_some(active + 1)
            })
            .map_err(|_| io::Error::other("JDBC Agent Blob stream worker limit reached"))?;
        Ok(Self)
    }
}

impl Drop for JdbcAgentBlobStreamPermit {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}
