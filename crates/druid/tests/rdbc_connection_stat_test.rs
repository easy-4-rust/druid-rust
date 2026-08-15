use druid::stats::{RdbcConnectionStat, RdbcConnectionStatEntry};
use std::sync::Arc;
use std::time::Duration;

// ── RdbcConnectionStatEntry ────────────────────────────────────

#[test]
fn connection_stat_entry_new() {
    let entry = RdbcConnectionStatEntry::new("test-ds", 42);
    let snap = entry.snapshot();
    assert_eq!(snap.id, 42);
    assert_eq!(snap.data_source, "test-ds");
}

#[test]
fn connection_stat_entry_mark_established() {
    let entry = RdbcConnectionStatEntry::new("test-ds", 1);
    entry.mark_established();
    let snap = entry.snapshot();
    assert!(snap.establish_time_millis.is_some());
}

#[test]
fn connection_stat_entry_set_connect_time() {
    let entry = RdbcConnectionStatEntry::new("test-ds", 1);
    entry.set_connect_time_millis(1000);
    let snap = entry.snapshot();
    assert_eq!(snap.connect_time_millis, Some(1000));
}

#[test]
fn connection_stat_entry_set_connect_timespan() {
    let entry = RdbcConnectionStatEntry::new("test-ds", 1);
    entry.set_connect_timespan_nanos(5000);
}

#[test]
fn connection_stat_entry_set_last_sql() {
    let entry = RdbcConnectionStatEntry::new("test-ds", 1);
    entry.set_last_sql(Some("SELECT 1".to_owned()));
    let snap = entry.snapshot();
    assert_eq!(snap.last_sql, Some("SELECT 1".to_owned()));
    entry.set_last_sql(None);
    let snap = entry.snapshot();
    assert!(snap.last_sql.is_none());
}

#[test]
fn connection_stat_entry_set_stack_traces() {
    let entry = RdbcConnectionStatEntry::new("test-ds", 1);
    entry.set_connect_stack_trace(Some("stack".to_owned()));
    entry.set_last_statement_stack_trace(Some("stmt-stack".to_owned()));
}

#[test]
fn connection_stat_entry_error() {
    let entry = RdbcConnectionStatEntry::new("test-ds", 1);
    entry.error("connection lost");
    let snap = entry.snapshot();
    assert!(snap.last_error.is_some());
}

#[test]
fn connection_stat_entry_reset() {
    let entry = RdbcConnectionStatEntry::new("test-ds", 1);
    entry.mark_established();
    entry.error("test");
    entry.reset();
    let snap = entry.snapshot();
    assert!(snap.last_error.is_none());
}

// ── RdbcConnectionStat ─────────────────────────────────────────

#[test]
fn connection_stat_new() {
    let stat = RdbcConnectionStat::new();
    assert_eq!(stat.active_count(), 0);
    assert_eq!(stat.active_max(), 0);
    assert_eq!(stat.connecting_count(), 0);
    assert_eq!(stat.connecting_max(), 0);
    assert_eq!(stat.connect_count(), 0);
}

#[test]
fn connection_stat_before_after_connect() {
    let stat = RdbcConnectionStat::new();
    stat.before_connect();
    assert_eq!(stat.connecting_count(), 1);
    assert_eq!(stat.connecting_max(), 1);
    stat.after_connected(Duration::from_millis(10));
    assert_eq!(stat.connecting_count(), 0);
    assert_eq!(stat.connect_count(), 1);
}

#[test]
fn connection_stat_connect_error() {
    let stat = RdbcConnectionStat::new();
    stat.before_connect();
    stat.connect_error("timeout");
    // connect_error 不减少 connecting_count（Java 语义）。
    assert_eq!(stat.connecting_count(), 1);
}

#[test]
fn connection_stat_error() {
    let stat = RdbcConnectionStat::new();
    stat.error("some error");
}

#[test]
fn connection_stat_after_close() {
    let stat = RdbcConnectionStat::new();
    stat.after_close(Duration::from_secs(60));
}

#[test]
fn connection_stat_register_remove_entry() {
    let stat = RdbcConnectionStat::new();
    let entry = Arc::new(RdbcConnectionStatEntry::new("test-ds", 1));
    stat.register_entry(entry);
    assert!(stat.entry(1).is_some());
    assert!(stat.entry(999).is_none());
    assert!(stat.remove_entry(1));
    assert!(stat.entry(1).is_none());
    assert!(!stat.remove_entry(1));
}

#[test]
fn connection_stat_connection_entries() {
    let stat = RdbcConnectionStat::new();
    let entry = Arc::new(RdbcConnectionStatEntry::new("test-ds", 1));
    stat.register_entry(entry);
    let entries = stat.connection_entries();
    assert_eq!(entries.len(), 1);
}

#[test]
fn connection_stat_increment_counters() {
    let stat = RdbcConnectionStat::new();
    stat.increment_connection_close_count();
    stat.increment_connection_commit_count();
    stat.increment_connection_rollback_count();
    stat.increment_transaction_start_count();
}
