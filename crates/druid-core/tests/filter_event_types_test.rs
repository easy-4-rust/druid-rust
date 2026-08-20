extern crate druid_core as druid;
use druid_core::core::{
    ConnectionEvent, ConnectionEventContext, ResultSetEvent, StatementEvent, StatementEventContext,
};
use std::time::Duration;

// ── ConnectionEvent extended variants ──────────────────────────

#[test]
fn connection_event_set_read_only() {
    let e = ConnectionEvent::SetReadOnly(true);
    assert_eq!(e, ConnectionEvent::SetReadOnly(true));
    assert_ne!(e, ConnectionEvent::SetReadOnly(false));
}

#[test]
fn connection_event_set_catalog() {
    let e = ConnectionEvent::SetCatalog("mydb".to_owned());
    match &e {
        ConnectionEvent::SetCatalog(s) => assert_eq!(s, "mydb"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn connection_event_set_transaction_isolation() {
    let e = ConnectionEvent::SetTransactionIsolation(2);
    assert_eq!(e, ConnectionEvent::SetTransactionIsolation(2));
}

#[test]
fn connection_event_set_schema() {
    let e = ConnectionEvent::SetSchema("public".to_owned());
    match &e {
        ConnectionEvent::SetSchema(s) => assert_eq!(s, "public"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn connection_event_set_network_timeout() {
    let e = ConnectionEvent::SetNetworkTimeout(Duration::from_secs(30));
    match &e {
        ConnectionEvent::SetNetworkTimeout(d) => assert_eq!(d.as_secs(), 30),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn connection_event_debug_all_variants() {
    let events = vec![
        ConnectionEvent::Connect,
        ConnectionEvent::Close,
        ConnectionEvent::SetAutoCommit(true),
        ConnectionEvent::GetAutoCommit,
        ConnectionEvent::Commit,
        ConnectionEvent::Rollback,
        ConnectionEvent::SetReadOnly(false),
        ConnectionEvent::GetReadOnly,
        ConnectionEvent::SetCatalog("db".to_owned()),
        ConnectionEvent::GetCatalog,
        ConnectionEvent::SetTransactionIsolation(2),
        ConnectionEvent::GetTransactionIsolation,
        ConnectionEvent::ClearWarnings,
        ConnectionEvent::SetSchema("s".to_owned()),
        ConnectionEvent::GetSchema,
        ConnectionEvent::Abort,
        ConnectionEvent::IsValid,
        ConnectionEvent::NativeSQL("sql".to_owned()),
        ConnectionEvent::SetNetworkTimeout(Duration::from_secs(1)),
        ConnectionEvent::GetNetworkTimeout,
    ];
    for event in events {
        let _ = format!("{:?}", event);
    }
}

// ── StatementEvent ─────────────────────────────────────────────

#[test]
fn statement_event_debug() {
    let events = vec![
        StatementEvent::CreateStatement,
        StatementEvent::PrepareStatement("SELECT 1".to_owned()),
        StatementEvent::PrepareCall("{call proc()}".to_owned()),
        StatementEvent::Execute("SELECT 1".to_owned()),
        StatementEvent::ExecuteQuery("SELECT 1".to_owned()),
        StatementEvent::ExecuteUpdate("UPDATE t SET x=1".to_owned()),
        StatementEvent::Close,
        StatementEvent::ExecuteBatch,
    ];
    for event in events {
        let _ = format!("{:?}", event);
    }
}

#[test]
fn statement_event_clone_eq() {
    let e1 = StatementEvent::PrepareStatement("SELECT 1".to_owned());
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

// ── ResultSetEvent ─────────────────────────────────────────────

#[test]
fn result_set_event_debug() {
    let events = vec![
        ResultSetEvent::Next,
        ResultSetEvent::Close,
        ResultSetEvent::GetString,
        ResultSetEvent::GetBoolean,
        ResultSetEvent::GetInt,
        ResultSetEvent::First,
        ResultSetEvent::Last,
    ];
    for event in events {
        let _ = format!("{:?}", event);
    }
}

#[test]
fn result_set_event_clone_eq() {
    let e1 = ResultSetEvent::Next;
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

// ── Context structs ────────────────────────────────────────────

#[test]
fn connection_event_context_debug() {
    let event = ConnectionEvent::Connect;
    let ctx = ConnectionEventContext {
        connection_id: 42,
        event: &event,
    };
    let dbg = format!("{:?}", ctx);
    assert!(dbg.contains("42"));
}

#[test]
fn statement_event_context_debug() {
    let event = StatementEvent::Close;
    let ctx = StatementEventContext {
        connection_id: 1,
        statement_id: 2,
        event: &event,
    };
    let dbg = format!("{:?}", ctx);
    assert!(dbg.contains("1"));
    assert!(dbg.contains("2"));
}
