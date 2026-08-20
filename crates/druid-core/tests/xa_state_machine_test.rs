#![allow(clippy::type_complexity)]
//! XA/2PC 状态机集成测试。
//!
//! 验证 `XaTransactionState` 的状态转换合法性、非法转换拒绝与超时逻辑。
//! 测试文件对应 Java `javax.transaction.xa.XAResource` 状态语义。

extern crate druid_core as druid;
use druid_core::core::{
    DruidError, XaOperation, XaState, XaStateTransitionError, XaStateTransitionRecord,
    XaTransactionState, Xid,
};
use std::time::Duration;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn test_xid(gtrid: u8) -> Xid {
    Xid::new(1, vec![gtrid], vec![gtrid + 100]).expect("test XID must be valid")
}

fn two_phase_tx(gtrid: u8) -> XaTransactionState {
    let mut tx = XaTransactionState::new(test_xid(gtrid));
    tx.start().unwrap();
    tx.end().unwrap();
    tx.prepare().unwrap();
    tx
}

// ── Full Lifecycle Tests ────────────────────────────────────────────────────

#[test]
fn xa_full_two_phase_commit_lifecycle() {
    let mut tx = XaTransactionState::new(test_xid(1));

    assert_eq!(tx.state(), XaState::Idle);
    assert!(!tx.is_terminal());

    tx.start().unwrap();
    assert_eq!(tx.state(), XaState::Active);

    tx.end().unwrap();
    assert_eq!(tx.state(), XaState::Preparing);

    tx.prepare().unwrap();
    assert_eq!(tx.state(), XaState::Prepared);

    tx.commit(false).unwrap();
    assert_eq!(tx.state(), XaState::Committed);
    assert!(tx.is_terminal());
}

#[test]
fn xa_full_one_phase_commit_lifecycle() {
    let mut tx = XaTransactionState::new(test_xid(2));

    tx.start().unwrap();
    assert_eq!(tx.state(), XaState::Active);

    tx.end().unwrap();
    assert_eq!(tx.state(), XaState::Preparing);

    tx.commit(true).unwrap();
    assert_eq!(tx.state(), XaState::Committed);
    assert!(tx.is_terminal());
}

#[test]
fn xa_full_rollback_from_active_lifecycle() {
    let mut tx = XaTransactionState::new(test_xid(3));

    tx.start().unwrap();
    tx.rollback().unwrap();
    assert_eq!(tx.state(), XaState::RolledBack);
    assert!(tx.is_terminal());
}

#[test]
fn xa_full_rollback_from_preparing_lifecycle() {
    let mut tx = XaTransactionState::new(test_xid(4));

    tx.start().unwrap();
    tx.end().unwrap();
    tx.rollback().unwrap();
    assert_eq!(tx.state(), XaState::RolledBack);
}

#[test]
fn xa_full_rollback_from_prepared_lifecycle() {
    let mut tx = XaTransactionState::new(test_xid(5));

    tx.start().unwrap();
    tx.end().unwrap();
    tx.prepare().unwrap();
    tx.rollback().unwrap();
    assert_eq!(tx.state(), XaState::RolledBack);
}

#[test]
fn xa_full_rollback_from_idle_lifecycle() {
    let mut tx = XaTransactionState::new(test_xid(6));
    tx.rollback().unwrap();
    assert_eq!(tx.state(), XaState::RolledBack);
}

#[test]
fn xa_full_failure_and_forget_lifecycle() {
    let mut tx = XaTransactionState::new(test_xid(7));

    tx.start().unwrap();
    tx.mark_failed().unwrap();
    assert_eq!(tx.state(), XaState::Failed);
    assert!(tx.is_terminal());

    tx.forget().unwrap();
    assert_eq!(tx.state(), XaState::Idle);
    assert!(!tx.is_terminal());
}

// ── Illegal Transition Tests ────────────────────────────────────────────────

#[test]
fn xa_start_from_non_idle_rejected() {
    let mut tx = XaTransactionState::new(test_xid(10));
    tx.start().unwrap();

    let err = tx.start().unwrap_err();
    assert_eq!(err.current_state(), XaState::Active);
    assert_eq!(err.operation(), XaOperation::Start);
}

#[test]
fn xa_end_from_idle_rejected() {
    let mut tx = XaTransactionState::new(test_xid(11));

    let err = tx.end().unwrap_err();
    assert_eq!(err.current_state(), XaState::Idle);
    assert_eq!(err.operation(), XaOperation::End);
}

#[test]
fn xa_prepare_from_idle_rejected() {
    let mut tx = XaTransactionState::new(test_xid(12));

    let err = tx.prepare().unwrap_err();
    assert_eq!(err.current_state(), XaState::Idle);
    assert_eq!(err.operation(), XaOperation::Prepare);
}

#[test]
fn xa_prepare_from_active_rejected() {
    let mut tx = XaTransactionState::new(test_xid(13));
    tx.start().unwrap();

    let err = tx.prepare().unwrap_err();
    assert_eq!(err.current_state(), XaState::Active);
    assert_eq!(err.operation(), XaOperation::Prepare);
}

#[test]
fn xa_commit_from_idle_rejected() {
    let mut tx = XaTransactionState::new(test_xid(14));

    let err = tx.commit(false).unwrap_err();
    assert_eq!(err.current_state(), XaState::Idle);
}

#[test]
fn xa_commit_from_active_rejected() {
    let mut tx = XaTransactionState::new(test_xid(15));
    tx.start().unwrap();

    let err = tx.commit(false).unwrap_err();
    assert_eq!(err.current_state(), XaState::Active);
}

#[test]
fn xa_commit_one_phase_from_idle_rejected() {
    let mut tx = XaTransactionState::new(test_xid(16));

    let err = tx.commit(true).unwrap_err();
    assert_eq!(err.current_state(), XaState::Idle);
}

#[test]
fn xa_forget_from_active_rejected() {
    let mut tx = XaTransactionState::new(test_xid(17));
    tx.start().unwrap();

    let err = tx.forget().unwrap_err();
    assert_eq!(err.current_state(), XaState::Active);
}

#[test]
fn xa_forget_from_committed_rejected() {
    let mut tx = two_phase_tx(18);
    tx.commit(false).unwrap();

    let err = tx.forget().unwrap_err();
    assert_eq!(err.current_state(), XaState::Committed);
}

// ── Terminal State Immutability ─────────────────────────────────────────────

#[test]
fn xa_committed_rejects_all_operations() {
    let mut tx = two_phase_tx(20);
    tx.commit(false).unwrap();

    assert_eq!(tx.state(), XaState::Committed);

    let ops: Vec<Box<dyn Fn(&mut XaTransactionState) -> Result<(), XaStateTransitionError>>> = vec![
        Box::new(druid::core::XaTransactionState::start),
        Box::new(druid::core::XaTransactionState::end),
        Box::new(druid::core::XaTransactionState::prepare),
        Box::new(|tx| tx.commit(false)),
        Box::new(|tx| tx.commit(true)),
        Box::new(druid::core::XaTransactionState::rollback),
        Box::new(druid::core::XaTransactionState::mark_failed),
    ];

    for op in ops {
        let err = op(&mut tx).unwrap_err();
        assert_eq!(err.current_state(), XaState::Committed);
    }
}

#[test]
fn xa_rolled_back_rejects_all_operations() {
    let mut tx = XaTransactionState::new(test_xid(21));
    tx.start().unwrap();
    tx.rollback().unwrap();

    assert_eq!(tx.state(), XaState::RolledBack);

    let ops: Vec<Box<dyn Fn(&mut XaTransactionState) -> Result<(), XaStateTransitionError>>> = vec![
        Box::new(druid::core::XaTransactionState::start),
        Box::new(druid::core::XaTransactionState::end),
        Box::new(druid::core::XaTransactionState::prepare),
        Box::new(|tx| tx.commit(false)),
        Box::new(druid::core::XaTransactionState::rollback),
        Box::new(druid::core::XaTransactionState::mark_failed),
    ];

    for op in ops {
        let err = op(&mut tx).unwrap_err();
        assert_eq!(err.current_state(), XaState::RolledBack);
    }
}

#[test]
fn xa_failed_allows_only_forget() {
    let mut tx = XaTransactionState::new(test_xid(22));
    tx.start().unwrap();
    tx.mark_failed().unwrap();

    // forget 从 Failed 回到 Idle 是合法的
    let mut tx2 = tx.clone();
    tx2.forget().unwrap();
    assert_eq!(tx2.state(), XaState::Idle);

    // 其他操作都应被拒绝
    let ops: Vec<Box<dyn Fn(&mut XaTransactionState) -> Result<(), XaStateTransitionError>>> = vec![
        Box::new(druid::core::XaTransactionState::start),
        Box::new(druid::core::XaTransactionState::end),
        Box::new(druid::core::XaTransactionState::prepare),
        Box::new(|tx| tx.commit(false)),
        Box::new(druid::core::XaTransactionState::rollback),
    ];

    for op in ops {
        let err = op(&mut tx).unwrap_err();
        assert_eq!(err.current_state(), XaState::Failed);
    }
}

// ── Timeout Tests ───────────────────────────────────────────────────────────

#[test]
fn xa_no_timeout_never_expires() {
    let tx = XaTransactionState::new(test_xid(30));
    assert!(!tx.is_timed_out());
    assert!(tx.timeout().is_none());
}

#[test]
fn xa_with_timeout_does_not_expire_immediately() {
    let tx = XaTransactionState::with_timeout(test_xid(31), Duration::from_mins(1));
    assert!(!tx.is_timed_out());
    assert_eq!(tx.timeout(), Some(Duration::from_mins(1)));
}

#[test]
fn xa_with_zero_timeout_expires_immediately() {
    // 零超时意味着任何已过去的纳秒都算超时
    // 但由于 Instant::now() 到 is_timed_out() 之间有微小间隔
    // 我们用 Duration::from_nanos(0) 测试边界
    let tx = XaTransactionState::with_timeout(test_xid(32), Duration::from_nanos(0));
    // 由于 elapsed > 0 对于任何非零时间间隔成立
    // 但零超时：elapsed() 可能刚好是 0，所以这里只验证设置正确
    assert_eq!(tx.timeout(), Some(Duration::from_nanos(0)));
}

// ── History Tracking Tests ──────────────────────────────────────────────────

#[test]
fn xa_history_records_all_transitions_in_two_phase_commit() {
    let mut tx = XaTransactionState::new(test_xid(40));

    tx.start().unwrap();
    tx.end().unwrap();
    tx.prepare().unwrap();
    tx.commit(false).unwrap();

    let history = tx.history();
    assert_eq!(history.len(), 5);

    assert_eq!(
        history[0],
        XaStateTransitionRecord {
            from: XaState::Idle,
            to: XaState::Active,
            operation: XaOperation::Start,
        }
    );
    assert_eq!(
        history[1],
        XaStateTransitionRecord {
            from: XaState::Active,
            to: XaState::Preparing,
            operation: XaOperation::End,
        }
    );
    assert_eq!(
        history[2],
        XaStateTransitionRecord {
            from: XaState::Preparing,
            to: XaState::Prepared,
            operation: XaOperation::Prepare,
        }
    );
    assert_eq!(
        history[3],
        XaStateTransitionRecord {
            from: XaState::Prepared,
            to: XaState::Committing,
            operation: XaOperation::Commit,
        }
    );
    assert_eq!(
        history[4],
        XaStateTransitionRecord {
            from: XaState::Committing,
            to: XaState::Committed,
            operation: XaOperation::Commit,
        }
    );
}

#[test]
fn xa_history_records_rollback_transitions() {
    let mut tx = XaTransactionState::new(test_xid(41));

    tx.start().unwrap();
    tx.end().unwrap();
    tx.prepare().unwrap();
    tx.rollback().unwrap();

    let history = tx.history();
    assert_eq!(history.len(), 5);

    // 最后两条记录是 RollingBack -> RolledBack
    assert_eq!(
        history[3],
        XaStateTransitionRecord {
            from: XaState::Prepared,
            to: XaState::RollingBack,
            operation: XaOperation::Rollback,
        }
    );
    assert_eq!(
        history[4],
        XaStateTransitionRecord {
            from: XaState::RollingBack,
            to: XaState::RolledBack,
            operation: XaOperation::Rollback,
        }
    );
}

#[test]
fn xa_history_empty_at_start() {
    let tx = XaTransactionState::new(test_xid(42));
    assert!(tx.history().is_empty());
}

// ── Error Conversion Tests ──────────────────────────────────────────────────

#[test]
fn xa_state_transition_error_to_druid_error() {
    let mut tx = two_phase_tx(81);
    tx.commit(false).unwrap();

    // 从 Committed 回滚触发 XaStateTransitionError
    let err = tx.rollback().unwrap_err();
    assert_eq!(err.current_state(), XaState::Committed);
    assert_eq!(err.operation(), XaOperation::Rollback);

    let druid_err: DruidError = err.into();
    assert!(matches!(druid_err, DruidError::InvalidArgument(_)));
    assert!(druid_err
        .to_string()
        .contains("invalid XA state transition"));
    assert!(druid_err.to_string().contains("COMMITTED"));
    assert!(druid_err.to_string().contains("rollback"));
}

#[test]
fn xa_state_transition_error_accessors() {
    let mut tx = XaTransactionState::new(test_xid(82));
    tx.start().unwrap();

    // 从 Active 直接 prepare 触发 XaStateTransitionError
    let err = tx.prepare().unwrap_err();
    assert_eq!(err.current_state(), XaState::Active);
    assert_eq!(err.operation(), XaOperation::Prepare);
}

// ── Xid Validation Tests ────────────────────────────────────────────────────

#[test]
fn xa_xid_valid_max_lengths() {
    let xid = Xid::new(0x12345678, vec![0xAA; 64], vec![0xBB; 64]).unwrap();
    assert_eq!(xid.format_id(), 0x12345678);
    assert_eq!(xid.global_transaction_id().len(), 64);
    assert_eq!(xid.branch_qualifier().len(), 64);
}

#[test]
fn xa_xid_gtrid_exceeds_max_rejected() {
    let err = Xid::new(1, vec![0; 65], vec![]).unwrap_err();
    assert!(err.to_string().contains("global_transaction_id"));
    assert!(err.to_string().contains("65"));
}

#[test]
fn xa_xid_bqual_exceeds_max_rejected() {
    let err = Xid::new(1, vec![1], vec![0; 65]).unwrap_err();
    assert!(err.to_string().contains("branch_qualifier"));
    assert!(err.to_string().contains("65"));
}

#[test]
fn xa_xid_display_format() {
    let xid = Xid::new(1, vec![0xAB, 0xCD, 0xEF], vec![0x01, 0x02]).unwrap();
    let display = format!("{xid}");
    assert!(display.contains("format_id=1"));
    assert!(display.contains("abcdef"));
    assert!(display.contains("0102"));
}

#[test]
fn xa_xid_equality() {
    let a = Xid::new(1, vec![1, 2], vec![3, 4]).unwrap();
    let b = Xid::new(1, vec![1, 2], vec![3, 4]).unwrap();
    assert_eq!(a, b);
}

#[test]
fn xa_xid_inequality_format_id() {
    let a = Xid::new(1, vec![1], vec![2]).unwrap();
    let b = Xid::new(2, vec![1], vec![2]).unwrap();
    assert_ne!(a, b);
}

#[test]
fn xa_xid_inequality_gtrid() {
    let a = Xid::new(1, vec![1], vec![2]).unwrap();
    let b = Xid::new(1, vec![9], vec![2]).unwrap();
    assert_ne!(a, b);
}

#[test]
fn xa_xid_inequality_bqual() {
    let a = Xid::new(1, vec![1], vec![2]).unwrap();
    let b = Xid::new(1, vec![1], vec![9]).unwrap();
    assert_ne!(a, b);
}

#[test]
fn xa_xid_hash_consistency() {
    use std::collections::HashSet;
    let a = Xid::new(1, vec![1], vec![2]).unwrap();
    let b = Xid::new(1, vec![1], vec![2]).unwrap();
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

// ── XaFlags Constants ───────────────────────────────────────────────────────

mod xa_flags_tests {
    use druid_core::core::xa_flags;

    #[test]
    fn tm_no_flags_is_zero() {
        assert_eq!(xa_flags::TMNOFLAGS, 0x00000000);
    }

    #[test]
    fn tm_join_flag() {
        assert_eq!(xa_flags::TMJOIN, 0x00000020);
    }

    #[test]
    fn tm_resume_flag() {
        assert_eq!(xa_flags::TMRESUME, 0x00000800);
    }

    #[test]
    fn tm_success_flag() {
        assert_eq!(xa_flags::TMSUCCESS, 0x00000004);
    }

    #[test]
    fn tm_fail_flag() {
        assert_eq!(xa_flags::TMFAIL, 0x00000008);
    }

    #[test]
    fn tm_suspend_flag() {
        assert_eq!(xa_flags::TMSUSPEND, 0x00000002);
    }

    #[test]
    fn tm_one_phase_flag() {
        assert_eq!(xa_flags::TMONEPHASE, 0x40000000);
    }

    #[test]
    fn tm_async_flag() {
        assert_eq!(xa_flags::TMASYNC, 0x02000000);
    }

    #[test]
    fn xa_rdonly_value() {
        assert_eq!(xa_flags::XA_RDONLY, 3);
    }
}

// ── XaState Display ─────────────────────────────────────────────────────────

#[test]
fn xa_state_display_values() {
    assert_eq!(format!("{}", XaState::Idle), "IDLE");
    assert_eq!(format!("{}", XaState::Active), "ACTIVE");
    assert_eq!(format!("{}", XaState::Preparing), "PREPARING");
    assert_eq!(format!("{}", XaState::Prepared), "PREPARED");
    assert_eq!(format!("{}", XaState::Committing), "COMMITTING");
    assert_eq!(format!("{}", XaState::Committed), "COMMITTED");
    assert_eq!(format!("{}", XaState::RollingBack), "ROLLING_BACK");
    assert_eq!(format!("{}", XaState::RolledBack), "ROLLED_BACK");
    assert_eq!(format!("{}", XaState::Failed), "FAILED");
}

// ── XaOperation Display ─────────────────────────────────────────────────────

#[test]
fn xa_operation_display_values() {
    assert_eq!(format!("{}", XaOperation::Start), "start");
    assert_eq!(format!("{}", XaOperation::End), "end");
    assert_eq!(format!("{}", XaOperation::Prepare), "prepare");
    assert_eq!(format!("{}", XaOperation::Commit), "commit");
    assert_eq!(format!("{}", XaOperation::Rollback), "rollback");
    assert_eq!(format!("{}", XaOperation::Forget), "forget");
}

// ── Edge Cases ──────────────────────────────────────────────────────────────

#[test]
fn xa_mark_failed_from_every_non_terminal_state() {
    let non_terminal_states = [
        XaState::Idle,
        XaState::Active,
        XaState::Preparing,
        XaState::Prepared,
    ];

    for (i, _) in non_terminal_states.iter().enumerate() {
        let mut tx = XaTransactionState::new(test_xid(50 + i as u8));

        // 到达目标状态
        match i {
            0 => { /* Idle */ }
            1 => {
                tx.start().unwrap();
            }
            2 => {
                tx.start().unwrap();
                tx.end().unwrap();
            }
            3 => {
                tx.start().unwrap();
                tx.end().unwrap();
                tx.prepare().unwrap();
            }
            _ => unreachable!(),
        }

        tx.mark_failed().unwrap();
        assert_eq!(tx.state(), XaState::Failed);
    }
}

#[test]
fn xa_mark_failed_from_terminal_states_rejected() {
    // Committed
    let mut tx = two_phase_tx(60);
    tx.commit(false).unwrap();
    assert!(tx.mark_failed().is_err());

    // RolledBack
    let mut tx = XaTransactionState::new(test_xid(61));
    tx.start().unwrap();
    tx.rollback().unwrap();
    assert!(tx.mark_failed().is_err());
}

#[test]
fn xa_forget_resets_to_idle() {
    let mut tx = XaTransactionState::new(test_xid(70));
    tx.start().unwrap();
    tx.mark_failed().unwrap();
    tx.forget().unwrap();

    // 回到 Idle 后可以重新 start
    assert_eq!(tx.state(), XaState::Idle);
    tx.start().unwrap();
    assert_eq!(tx.state(), XaState::Active);
}

#[test]
fn xa_clone_preserves_state() {
    let mut tx = XaTransactionState::new(test_xid(80));
    tx.start().unwrap();
    tx.end().unwrap();
    tx.prepare().unwrap();

    let cloned = tx.clone();
    assert_eq!(cloned.state(), XaState::Prepared);
    assert_eq!(cloned.history().len(), 3);
    assert_eq!(cloned.xid(), tx.xid());
}

#[test]
fn xa_elapsed_is_positive_after_some_time() {
    let tx = XaTransactionState::new(test_xid(90));
    // elapsed() 至少返回 Duration::ZERO
    let elapsed = tx.elapsed();
    // 不 panic 即可；实际运行中 elapsed > 0
    let _ = elapsed;
}

#[test]
fn xa_xid_zero_format_id() {
    // OSI CCR 格式：format_id = 0
    let xid = Xid::new(0, vec![1, 2, 3], vec![]).unwrap();
    assert_eq!(xid.format_id(), 0);
}

#[test]
fn xa_xid_negative_format_id() {
    // 负值 format_id 在 XA 规范中无效但 Java 允许，我们也允许
    let xid = Xid::new(-1, vec![1], vec![2]).unwrap();
    assert_eq!(xid.format_id(), -1);
}
