//! 日志中的单个参与者记录。

use serde::{Deserialize, Serialize};

use super::participant_state::ParticipantState;

/// 协调日志保存的参与者身份与分支状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantRecord {
    /// 参与者资源名；用于恢复时重新建立分支连接。
    pub name: String,
    /// 最近一次确认的分支状态。
    pub state: ParticipantState,
}

impl ParticipantRecord {
    /// 创建参与者记录。
    #[must_use]
    pub fn new(name: impl Into<String>, state: ParticipantState) -> Self {
        Self {
            name: name.into(),
            state,
        }
    }
}
