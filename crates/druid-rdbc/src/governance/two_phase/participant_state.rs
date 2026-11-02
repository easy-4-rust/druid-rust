//! 单个参与者的分支状态。
//!
//! `Unknown` 表示网络或崩溃导致无法确认，不能等同于“未提交”；`Mixed`
//! 表示同一参与者出现互相矛盾的可观察结果，只能人工核验。

use serde::{Deserialize, Serialize};

/// 单个资源参与者的分支状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantState {
    /// 尚未收到 prepare 响应。
    Unprepared,
    /// 已投票 yes，等待全局决策。
    Prepared,
    /// 已确认提交。
    Committed,
    /// 已确认回滚。
    Aborted,
    /// 无法确认结果；不得视为未提交。
    Unknown,
    /// 观察到互相矛盾的结果；禁止再次 commit 探测。
    Mixed,
}
