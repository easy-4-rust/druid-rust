//! 协调日志的内存模型。
//!
//! 持久化键固定为 `2pc:tx:{id}`；SQLite/WAL 适配器在 8.2 实现，本结构
//! 只冻结字段和决策顺序，不代表已经具备 durable 存储。

use serde::{Deserialize, Serialize};

use super::global_decision::GlobalDecision;
use super::participant_record::ParticipantRecord;
use super::transaction_state::TransactionState;

/// 当前日志 schema 版本。
pub const TWO_PHASE_LOG_SCHEMA_VERSION: u16 = 1;

/// 一次两阶段提交的完整协调记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoPhaseLog {
    /// 日志 schema 版本；恢复时拒绝未知版本。
    pub schema_version: u16,
    /// 事务标识；持久化键为 `2pc:tx:{transaction_id}`。
    pub transaction_id: String,
    /// 协调器状态。
    pub state: TransactionState,
    /// 已落盘的全局决策。
    pub decision: GlobalDecision,
    /// 逐个参与者的分支状态。
    pub participants: Vec<ParticipantRecord>,
}

impl TwoPhaseLog {
    /// 创建版本为 1 的新日志。
    #[must_use]
    pub fn new(transaction_id: impl Into<String>, participants: Vec<ParticipantRecord>) -> Self {
        Self {
            schema_version: TWO_PHASE_LOG_SCHEMA_VERSION,
            transaction_id: transaction_id.into(),
            state: TransactionState::Active,
            decision: GlobalDecision::Undecided,
            participants,
        }
    }

    /// 返回 durable 存储键 `2pc:tx:{id}`。
    #[must_use]
    pub fn log_key(&self) -> String {
        format!("2pc:tx:{}", self.transaction_id)
    }

    /// 返回所有参与者状态，便于规划器做只读判断。
    #[must_use]
    pub fn participant_states(&self) -> impl Iterator<Item = &ParticipantRecord> {
        self.participants.iter()
    }
}
