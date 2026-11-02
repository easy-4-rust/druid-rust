//! 全局事务状态。
//!
//! 对应 durable two-phase commit 协调器的事务生命周期；这是 Rust 扩展，
//! 不对应 Java Druid 的单个 XA 分支状态。

use serde::{Deserialize, Serialize};

/// 协调器视角的事务状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionState {
    /// 已创建，尚未开始 prepare。
    Active,
    /// 正在向参与者收集 prepare 投票。
    Preparing,
    /// 全部参与者已 prepare，等待落盘全局决策。
    Prepared,
    /// 已落盘 commit 决策，正在重放提交。
    Committing,
    /// 全部参与者已提交。
    Committed,
    /// 已落盘 rollback 决策，正在重放回滚。
    Aborting,
    /// 全部参与者已回滚。
    Aborted,
    /// 参与者结果互相矛盾，只能只读核验或人工裁决。
    Mixed,
    /// 崩溃/网络导致协调器无法确认当前状态。
    Unknown,
}
