extern crate druid_core as druid;
use druid_core::core::{ConnectionEvent, ExecOperation};

// ── ExecOperation ──────────────────────────────────────────────

#[test]
fn exec_operation_debug() {
    let ops = [
        ExecOperation::Execute,
        ExecOperation::Query,
        ExecOperation::Update,
        ExecOperation::Batch,
    ];
    for op in &ops {
        let dbg = format!("{op:?}");
        assert!(!dbg.is_empty());
    }
}

#[test]
fn exec_operation_clone_eq() {
    let op = ExecOperation::Query;
    let op2 = op;
    assert_eq!(op, op2);
}

#[test]
fn exec_operation_ne() {
    assert_ne!(ExecOperation::Query, ExecOperation::Update);
    assert_ne!(ExecOperation::Execute, ExecOperation::Batch);
}

// ── ConnectionEvent ────────────────────────────────────────────

#[test]
fn connection_event_debug() {
    let events = [
        ConnectionEvent::Connect,
        ConnectionEvent::Close,
        ConnectionEvent::SetAutoCommit(true),
        ConnectionEvent::SetAutoCommit(false),
        ConnectionEvent::GetAutoCommit,
        ConnectionEvent::Commit,
        ConnectionEvent::Rollback,
    ];
    for event in &events {
        let dbg = format!("{event:?}");
        assert!(!dbg.is_empty());
    }
}

#[test]
fn connection_event_clone_eq() {
    let e1 = ConnectionEvent::Connect;
    let e2 = e1.clone();
    assert_eq!(e1, e2);

    let e3 = ConnectionEvent::SetAutoCommit(true);
    let e4 = e3.clone();
    assert_eq!(e3, e4);
}

#[test]
fn connection_event_ne() {
    assert_ne!(ConnectionEvent::Connect, ConnectionEvent::Close);
    assert_ne!(
        ConnectionEvent::SetAutoCommit(true),
        ConnectionEvent::SetAutoCommit(false)
    );
}
