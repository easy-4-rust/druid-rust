use std::io::{self, Write};

use super::DataSourceEntry;

/// 只计量最新状态投影的序列化字节数，不分配完整 JSON 缓冲区。
pub(crate) struct SerializedSizeBudget {
    bytes: usize,
    limit: usize,
}

impl SerializedSizeBudget {
    /// 在给定预算内计量 datasource、SQL 和 wall 投影；超限或序列化失败返回 `None`。
    pub(crate) fn measure(entry: &DataSourceEntry, limit: usize) -> Option<usize> {
        let mut counter = Self { bytes: 0, limit };
        serde_json::to_writer(
            &mut counter,
            &(&entry.datasource, &entry.sql_stats, &entry.wall),
        )
        .ok()?;
        Some(counter.bytes)
    }
}

impl Write for SerializedSizeBudget {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.limit.saturating_sub(self.bytes) {
            return Err(io::Error::other("repository state byte budget exceeded"));
        }
        self.bytes += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
