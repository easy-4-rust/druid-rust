//! Durable 2PC 恢复模型的安全边界。
//!
//! 覆盖 8.1 冻结的决策顺序：Mixed 禁止任何 commit 探测、Unknown 只读核验、
//! 无决策的 prepared 事务必须裁决、已落盘决策才可幂等重放。

use druid_rdbc::governance::two_phase::{
    plan_recovery, GlobalDecision, ParticipantRecord, ParticipantState, RecoveryAction,
    TransactionState, TwoPhaseLog, TWO_PHASE_LOG_SCHEMA_VERSION,
};

fn log_with(
    state: TransactionState,
    decision: GlobalDecision,
    participants: &[ParticipantState],
) -> TwoPhaseLog {
    let mut log = TwoPhaseLog::new(
        "tx-42",
        participants
            .iter()
            .enumerate()
            .map(|(index, state)| ParticipantRecord::new(format!("resource-{index}"), *state))
            .collect(),
    );
    log.state = state;
    log.decision = decision;
    log
}

#[test]
fn log_key_and_schema_are_frozen() {
    let log = log_with(
        TransactionState::Active,
        GlobalDecision::Undecided,
        &[ParticipantState::Unprepared],
    );
    assert_eq!(log.schema_version, TWO_PHASE_LOG_SCHEMA_VERSION);
    assert_eq!(log.log_key(), "2pc:tx:tx-42");
}

#[test]
fn mixed_forbids_any_commit_probe() {
    let log = log_with(
        TransactionState::Mixed,
        GlobalDecision::Commit,
        &[ParticipantState::Committed, ParticipantState::Mixed],
    );
    let action = plan_recovery(&log);
    assert_eq!(action, RecoveryAction::ManualAdjudication);
    assert!(!action.allows_commit_retry());
}

#[test]
fn unknown_only_allows_read_only_verification() {
    let log = log_with(
        TransactionState::Unknown,
        GlobalDecision::Undecided,
        &[ParticipantState::Unknown],
    );
    let action = plan_recovery(&log);
    assert_eq!(action, RecoveryAction::VerifyOnly);
    assert!(!action.allows_commit_retry());
}

#[test]
fn commit_decision_replays_only_unfinished_branches() {
    let log = log_with(
        TransactionState::Committing,
        GlobalDecision::Commit,
        &[ParticipantState::Committed, ParticipantState::Prepared],
    );
    assert_eq!(plan_recovery(&log), RecoveryAction::ReplayCommit);
}

#[test]
fn commit_decision_with_unprepared_branch_requires_adjudication() {
    let log = log_with(
        TransactionState::Committing,
        GlobalDecision::Commit,
        &[ParticipantState::Committed, ParticipantState::Unprepared],
    );
    assert_eq!(plan_recovery(&log), RecoveryAction::ManualAdjudication);
}

#[test]
fn prepared_without_decision_never_guesses() {
    let log = log_with(
        TransactionState::Prepared,
        GlobalDecision::Undecided,
        &[ParticipantState::Prepared, ParticipantState::Prepared],
    );
    assert_eq!(plan_recovery(&log), RecoveryAction::ManualAdjudication);
}

#[test]
fn rollback_decision_replays_unfinished_branches() {
    let log = log_with(
        TransactionState::Aborting,
        GlobalDecision::Rollback,
        &[ParticipantState::Aborted, ParticipantState::Prepared],
    );
    assert_eq!(plan_recovery(&log), RecoveryAction::ReplayRollback);
}

#[test]
fn fully_completed_logs_are_terminal() {
    let committed = log_with(
        TransactionState::Committed,
        GlobalDecision::Commit,
        &[ParticipantState::Committed, ParticipantState::Committed],
    );
    assert_eq!(plan_recovery(&committed), RecoveryAction::Complete);

    let aborted = log_with(
        TransactionState::Aborted,
        GlobalDecision::Rollback,
        &[ParticipantState::Aborted],
    );
    assert_eq!(plan_recovery(&aborted), RecoveryAction::Complete);
}
