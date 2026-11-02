//! 恢复规划：从日志推导允许的外部动作。
//!
//! 决策顺序固定为：先识别 `Mixed`（禁止任何 commit 探测），再识别
//! `Unknown`（只读核验），最后才按已落盘决策重放。没有决策的 prepared
//! 事务必须人工/策略裁决，不能猜测提交或回滚。

use super::global_decision::GlobalDecision;
use super::participant_state::ParticipantState;
use super::recovery_action::RecoveryAction;
use super::transaction_state::TransactionState;
use super::two_phase_log::TwoPhaseLog;

/// 根据协调日志规划恢复动作。
#[must_use]
pub fn plan_recovery(log: &TwoPhaseLog) -> RecoveryAction {
    // Mixed 优先于一切决策：即使日志里已有 commit，也不能再次 commit 探测。
    if log.state == TransactionState::Mixed
        || log
            .participant_states()
            .any(|participant| participant.state == ParticipantState::Mixed)
    {
        return RecoveryAction::ManualAdjudication;
    }
    if log.state == TransactionState::Unknown {
        return RecoveryAction::VerifyOnly;
    }

    match log.decision {
        GlobalDecision::Commit => {
            // commit 决策要求所有参与者至少 prepare 过；出现 unprepared 说明日志损坏。
            if log
                .participant_states()
                .any(|participant| participant.state == ParticipantState::Unprepared)
            {
                return RecoveryAction::ManualAdjudication;
            }
            if log
                .participant_states()
                .all(|participant| participant.state == ParticipantState::Committed)
            {
                RecoveryAction::Complete
            } else {
                // 已落盘 commit 决策的幂等重放，不是再次 commit 探测。
                RecoveryAction::ReplayCommit
            }
        }
        GlobalDecision::Rollback => {
            if log
                .participant_states()
                .all(|participant| participant.state == ParticipantState::Aborted)
            {
                RecoveryAction::Complete
            } else {
                RecoveryAction::ReplayRollback
            }
        }
        GlobalDecision::Undecided => match log.state {
            TransactionState::Committed => {
                if all_in(log, ParticipantState::Committed) {
                    RecoveryAction::Complete
                } else {
                    RecoveryAction::ManualAdjudication
                }
            }
            TransactionState::Aborted => {
                if all_in(log, ParticipantState::Aborted) {
                    RecoveryAction::Complete
                } else {
                    RecoveryAction::ManualAdjudication
                }
            }
            // prepare 后、决策前崩溃：不得猜提交/回滚。
            TransactionState::Prepared
            | TransactionState::Committing
            | TransactionState::Aborting => RecoveryAction::ManualAdjudication,
            TransactionState::Active | TransactionState::Preparing => RecoveryAction::Wait,
            // Mixed/Unknown 已在函数开头处理。
            TransactionState::Mixed | TransactionState::Unknown => {
                RecoveryAction::ManualAdjudication
            }
        },
    }
}

fn all_in(log: &TwoPhaseLog, state: ParticipantState) -> bool {
    log.participant_states()
        .all(|participant| participant.state == state)
}
