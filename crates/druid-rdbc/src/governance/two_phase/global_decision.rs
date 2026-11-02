//! 全局提交/回滚决策。

use serde::{Deserialize, Serialize};

/// 协调器已经落盘的全局决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalDecision {
    /// 尚未落盘决策；崩溃后必须人工/策略裁决。
    Undecided,
    /// 全部参与者提交。
    Commit,
    /// 全部参与者回滚。
    Rollback,
}
