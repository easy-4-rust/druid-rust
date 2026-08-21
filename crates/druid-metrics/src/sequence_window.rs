//! 客户端 sequence/ACK 窗口。
//!
//! 窗口保存已经分配 sequence 的完整 protobuf frame 字节。重连时只恢复并发送这些
//! 不可变字节，不能重新构造 header、时间戳或 payload。

use std::collections::BTreeMap;

use thiserror::Error;

/// Sequence/ACK 窗口错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SequenceWindowError {
    /// 窗口已满；调用方必须等待 ACK 或取消，不能丢弃 frame。
    #[error("window full: capacity {capacity}, pending {pending}")]
    WindowFull {
        /// 最大未 ACK frame 数。
        capacity: usize,
        /// 当前未 ACK frame 数。
        pending: usize,
    },

    /// ACK 的 sequence 从未进入当前窗口，或者已经 ACK。
    #[error("unknown sequence: {0}")]
    UnknownSequence(u64),

    /// 恢复的 sequence 为协议保留值 0。
    #[error("invalid sequence: {0}")]
    InvalidSequence(u64),

    /// 相同 sequence 已经存在，拒绝用另一份字节覆盖。
    #[error("duplicate sequence: {0}")]
    DuplicateSequence(u64),
}

/// 已发送但尚未 ACK 的完整、不可变客户端 frame。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFrame {
    /// 单调递增的流内 sequence。
    pub sequence: u64,
    /// 完整 [`crate::protocol::ClientFrame`] 的 protobuf 编码字节。
    pub encoded_frame: Vec<u8>,
}

/// 有界、按 sequence 排序的未 ACK frame 窗口。
#[derive(Debug)]
pub struct SequenceWindow {
    pending: BTreeMap<u64, PendingFrame>,
    capacity: usize,
    next_sequence: u64,
}

impl SequenceWindow {
    /// 创建指定容量的空窗口；第一个新 frame 的 sequence 为 1。
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            pending: BTreeMap::new(),
            capacity,
            next_sequence: 1,
        }
    }

    /// 分配 sequence，调用一次编码闭包，并保存闭包返回的完整 frame 字节。
    ///
    /// 窗口已满时不会调用 `encode_frame`，sequence 也不会前移。
    pub fn push_with(
        &mut self,
        encode_frame: impl FnOnce(u64) -> Vec<u8>,
    ) -> Result<PendingFrame, SequenceWindowError> {
        self.ensure_capacity()?;
        let sequence = self.next_sequence;
        let encoded_frame = encode_frame(sequence);
        let pending = PendingFrame {
            sequence,
            encoded_frame,
        };
        self.pending.insert(sequence, pending.clone());
        self.next_sequence = self.next_sequence.saturating_add(1);
        Ok(pending)
    }

    /// 恢复重连前保存的完整 frame，并保证后续新 sequence 大于已恢复 sequence。
    pub fn restore(&mut self, pending: PendingFrame) -> Result<(), SequenceWindowError> {
        if pending.sequence == 0 {
            return Err(SequenceWindowError::InvalidSequence(pending.sequence));
        }
        if self.pending.contains_key(&pending.sequence) {
            return Err(SequenceWindowError::DuplicateSequence(pending.sequence));
        }
        self.ensure_capacity()?;
        self.next_sequence = self.next_sequence.max(pending.sequence.saturating_add(1));
        self.pending.insert(pending.sequence, pending);
        Ok(())
    }

    /// ACK 一个已发送 frame 并释放窗口容量。
    pub fn ack(&mut self, sequence: u64) -> Result<(), SequenceWindowError> {
        self.pending
            .remove(&sequence)
            .map(|_| ())
            .ok_or(SequenceWindowError::UnknownSequence(sequence))
    }

    /// 按 sequence 返回所有未 ACK frame 的不可变字节副本。
    #[must_use]
    pub fn pending_frames(&self) -> Vec<PendingFrame> {
        self.pending.values().cloned().collect()
    }

    /// 返回当前未 ACK frame 数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// 返回窗口是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// 返回窗口是否已满。
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.pending.len() >= self.capacity
    }

    /// 返回最大未 ACK frame 数。
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    fn ensure_capacity(&self) -> Result<(), SequenceWindowError> {
        if self.is_full() {
            Err(SequenceWindowError::WindowFull {
                capacity: self.capacity,
                pending: self.pending.len(),
            })
        } else {
            Ok(())
        }
    }
}
