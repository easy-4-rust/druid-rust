//! 恢复流程允许执行的动作。
//!
//! `ReplayCommit` 只用于“已落盘 commit 决策”的幂等重放；它不是再次
//! commit 探测。`Mixed`/`Unknown` 只能得到 `VerifyOnly` 或
//! `ManualAdjudication`。

use serde::{Deserialize, Serialize};

/// 恢复规划器返回的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    /// 日志已终态，无需外部动作。
    Complete,
    /// 等待参与者或协调器继续推进。
    Wait,
    /// 按已落盘 commit 决策重放未完成分支。
    ReplayCommit,
    /// 按已落盘 rollback 决策重放未完成分支。
    ReplayRollback,
    /// 只读核验参与者和日志，不得发 commit/rollback。
    VerifyOnly,
    /// 需要人工或已批准策略裁决。
    ManualAdjudication,
}

impl RecoveryAction {
    /// 是否允许再次向参与者发送 commit。
    ///
    /// 只有已落盘 commit 决策的幂等重放返回 `true`；`Mixed`/`Unknown`
    /// 永远不能借此探测参与者是否已提交。
    #[must_use]
    pub const fn allows_commit_retry(self) -> bool {
        matches!(self, Self::ReplayCommit)
    }
}
