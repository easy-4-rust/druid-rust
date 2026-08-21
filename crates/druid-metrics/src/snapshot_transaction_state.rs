use crate::protocol::SnapshotEntry;

/// 服务端按 stream 暂存的权威快照事务。
///
/// Rust-only 协议状态对象；只有完整的 end chunk 通过 repository commit 后，
/// 暂存条目才会替换当前可见快照。
#[derive(Debug)]
pub(crate) struct SnapshotTransactionState {
    transaction_id: String,
    next_chunk_index: u32,
    chunk_count: u32,
    entries: Vec<SnapshotEntry>,
}

impl SnapshotTransactionState {
    /// 从 index 0 创建新的暂存事务。
    pub(crate) fn new(transaction_id: String, chunk_count: u32) -> Self {
        Self {
            transaction_id,
            next_chunk_index: 0,
            chunk_count,
            entries: Vec::new(),
        }
    }

    /// 返回当前事务是否接受指定 transaction/chunk。
    pub(crate) fn accepts(&self, transaction_id: &str, chunk_index: u32, chunk_count: u32) -> bool {
        self.transaction_id == transaction_id
            && self.next_chunk_index == chunk_index
            && self.chunk_count == chunk_count
    }

    /// 暂存一个非 end chunk；最终由 repository 在一次写事务中校验 identity。
    pub(crate) fn stage(&mut self, entries: &[SnapshotEntry]) {
        self.entries.extend_from_slice(entries);
        self.next_chunk_index = self.next_chunk_index.saturating_add(1);
    }

    /// 返回当前已经暂存的 datasource 条目数。
    pub(crate) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// 构造包含 end chunk 的完整候选快照，但不改变暂存状态。
    pub(crate) fn complete_candidate(&self, entries: &[SnapshotEntry]) -> Vec<SnapshotEntry> {
        let mut complete = self.entries.clone();
        complete.extend_from_slice(entries);
        complete
    }
}
