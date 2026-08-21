use druid::core::{ClobEvent, DataSourceEvent, StatementPropertyEvent};

// ── StatementPropertyEvent ─────────────────────────────────────

#[test]
fn statement_property_event_debug() {
    let events = vec![
        StatementPropertyEvent::SetQueryTimeout(30),
        StatementPropertyEvent::GetQueryTimeout,
        StatementPropertyEvent::GetUpdateCount,
        StatementPropertyEvent::SetMaxRows(100),
        StatementPropertyEvent::GetMaxRows,
        StatementPropertyEvent::SetMaxFieldSize(1024),
        StatementPropertyEvent::GetMaxFieldSize,
        StatementPropertyEvent::SetFetchDirection(1),
        StatementPropertyEvent::GetFetchDirection,
        StatementPropertyEvent::SetFetchSize(50),
        StatementPropertyEvent::GetFetchSize,
        StatementPropertyEvent::IsPoolable,
        StatementPropertyEvent::IsClosed,
        StatementPropertyEvent::GetMoreResults,
        StatementPropertyEvent::GetResultSetConcurrency,
        StatementPropertyEvent::GetResultSetType,
        StatementPropertyEvent::GetResultSetHoldability,
        StatementPropertyEvent::GetGeneratedKeys,
        StatementPropertyEvent::ClearWarnings,
        StatementPropertyEvent::SetCursorName("cursor".to_owned()),
        StatementPropertyEvent::AddBatch("INSERT INTO t VALUES (1)".to_owned()),
    ];
    for event in events {
        let _ = format!("{event:?}");
    }
}

#[test]
fn statement_property_event_clone_eq() {
    let e1 = StatementPropertyEvent::SetQueryTimeout(30);
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

// ── ClobEvent ──────────────────────────────────────────────────

#[test]
fn clob_event_debug() {
    let events = vec![
        ClobEvent::Length,
        ClobEvent::GetSubString(0, 10),
        ClobEvent::SetString(0, "hello".to_owned()),
        ClobEvent::Truncate(100),
        ClobEvent::Free,
    ];
    for event in events {
        let _ = format!("{event:?}");
    }
}

#[test]
fn clob_event_clone_eq() {
    let e1 = ClobEvent::GetSubString(0, 10);
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

// ── DataSourceEvent ────────────────────────────────────────────

#[test]
fn data_source_event_debug() {
    let events = vec![
        DataSourceEvent::GetConnection,
        DataSourceEvent::GetConnectionWithAuth("user".to_owned(), "pass".to_owned()),
        DataSourceEvent::ReleaseConnection,
        DataSourceEvent::Log("test log".to_owned()),
    ];
    for event in events {
        let _ = format!("{event:?}");
    }
}

#[test]
fn data_source_event_clone_eq() {
    let e1 = DataSourceEvent::GetConnection;
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}
