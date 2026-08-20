extern crate druid_core as druid;
use druid::core::{BatchExecKind, ExecOperation};

// ── BatchExecKind ──────────────────────────────────────────────

#[test]
fn batch_exec_kind_debug() {
    let k1 = BatchExecKind::Statement;
    let k2 = BatchExecKind::PreparedStatement;
    assert!(format!("{:?}", k1).contains("Statement"));
    assert!(format!("{:?}", k2).contains("PreparedStatement"));
}

#[test]
fn batch_exec_kind_clone_eq() {
    let k1 = BatchExecKind::Statement;
    let k2 = k1.clone();
    assert_eq!(k1, k2);
}

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
        let _ = format!("{:?}", op);
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
