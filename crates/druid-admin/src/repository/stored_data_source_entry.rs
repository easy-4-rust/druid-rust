use std::time::Instant;

use super::DataSourceEntry;

/// repository 内部保存的最新值与服务端接收时刻。
pub(crate) struct StoredDataSourceEntry {
    pub(crate) entry: DataSourceEntry,
    pub(crate) received_at: Instant,
    pub(crate) stream_epoch: Option<u64>,
}

impl StoredDataSourceEntry {
    /// 使用当前单调时钟保存一个已通过预算校验的条目。
    pub(crate) fn new(entry: DataSourceEntry, stream_epoch: Option<u64>) -> Self {
        Self {
            entry,
            received_at: Instant::now(),
            stream_epoch,
        }
    }
}
