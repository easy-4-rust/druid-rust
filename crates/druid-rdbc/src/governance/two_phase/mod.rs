//! Durable two-phase commit 的协调日志模型。
//!
//! 这是 Rust 侧扩展：Java Druid 只有 XA 资源操作，没有跨资源持久化协调器。
//! 本模块先冻结状态、决策与恢复动作，持久化适配器（SQLite `2pc:tx:{id}`）
//! 在后续任务实现，不能把 XA `recover` 当作本模型已持久化。

mod global_decision;
mod participant_record;
mod participant_state;
mod recovery_action;
mod recovery_plan;
mod transaction_state;
mod two_phase_log;

pub use global_decision::GlobalDecision;
pub use participant_record::ParticipantRecord;
pub use participant_state::ParticipantState;
pub use recovery_action::RecoveryAction;
pub use recovery_plan::plan_recovery;
pub use transaction_state::TransactionState;
pub use two_phase_log::{TwoPhaseLog, TWO_PHASE_LOG_SCHEMA_VERSION};
