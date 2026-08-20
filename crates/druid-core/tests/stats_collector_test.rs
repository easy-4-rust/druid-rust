extern crate druid_core as druid;
use druid::stats::StatsCollector;
use std::time::Duration;

fn make_collector() -> StatsCollector {
    StatsCollector::new("test", Duration::from_secs(10))
}

// ── new ────────────────────────────────────────────────────────

#[test]
fn stats_collector_new() {
    let c = make_collector();
    assert_eq!(c.connect_count(), 0);
    assert_eq!(c.slow_sql_count(), 0);
    assert_eq!(c.execute_batch_count(), 0);
    assert_eq!(c.execute_batch_size_total(), 0);
    assert_eq!(c.skip_sql_count(), 0);
}

// ── record_sql ─────────────────────────────────────────────────

#[test]
fn stats_collector_record_sql_ok() {
    let c = make_collector();
    c.record_sql("SELECT 1", Duration::from_millis(10), true);
    c.record_sql("SELECT 2", Duration::from_millis(5), true);
    assert_eq!(c.connect_count(), 0);
}

#[test]
fn stats_collector_record_sql_error() {
    let c = make_collector();
    c.record_sql("SELECT bad", Duration::from_millis(10), false);
}

// ── record_sql_with_merge ──────────────────────────────────────

#[test]
fn stats_collector_record_sql_with_merge() {
    let c = make_collector();
    c.record_sql_with_merge(
        "SELECT * FROM t WHERE id = 1",
        Duration::from_millis(10),
        true,
        true,
    );
    c.record_sql_with_merge(
        "SELECT * FROM t WHERE id = 2",
        Duration::from_millis(5),
        true,
        true,
    );
}

// ── connect / close ────────────────────────────────────────────

#[test]
fn stats_collector_connect_count() {
    let c = make_collector();
    c.record_connect();
    c.record_connect();
    assert_eq!(c.connect_count(), 2);
}

#[test]
fn stats_collector_connect_error() {
    let c = make_collector();
    c.record_connect_error();
}

#[test]
fn stats_collector_close() {
    let c = make_collector();
    c.record_close();
}

// ── execute_batch ──────────────────────────────────────────────

#[test]
fn stats_collector_execute_batch() {
    let c = make_collector();
    c.record_execute_batch(10);
    c.record_execute_batch(5);
    assert_eq!(c.execute_batch_count(), 2);
    assert_eq!(c.execute_batch_size_total(), 15);
}

// ── max_sql_size ───────────────────────────────────────────────

#[test]
fn stats_collector_max_sql_size() {
    let c = make_collector();
    c.set_max_sql_size(1024);
    assert_eq!(c.max_sql_size(), 1024);
}

// ── clob / blob ────────────────────────────────────────────────

#[test]
fn stats_collector_clob_blob() {
    let c = make_collector();
    c.record_clob_open();
    c.record_blob_open();
}

// ── execute_result ─────────────────────────────────────────────

#[test]
fn stats_collector_execute_result() {
    let c = make_collector();
    c.record_execute_result(true);
    c.record_execute_result(false);
}

// ── transaction ────────────────────────────────────────────────

#[test]
fn stats_collector_transaction() {
    let c = make_collector();
    c.record_start_transaction();
    c.record_commit(Some(Duration::from_millis(100)));
    c.record_rollback(Some(Duration::from_millis(50)));
}

#[test]
fn stats_collector_commit_no_elapsed() {
    let c = make_collector();
    c.record_commit(None);
    c.record_rollback(None);
}

// ── connection_hold ────────────────────────────────────────────

#[test]
fn stats_collector_connection_hold() {
    let c = make_collector();
    c.record_connection_hold(Duration::from_secs(5));
}

// ── sub-stats ──────────────────────────────────────────────────

#[test]
fn stats_collector_result_set_stat() {
    let c = make_collector();
    let _ = c.result_set_stat();
}

#[test]
fn stats_collector_connection_stat() {
    let c = make_collector();
    let _ = c.connection_stat();
}

#[test]
fn stats_collector_statement_stat() {
    let c = make_collector();
    let _ = c.statement_stat();
}

// ── reset ──────────────────────────────────────────────────────

#[test]
fn stats_collector_reset() {
    let c = make_collector();
    c.record_connect();
    c.record_sql("SELECT 1", Duration::from_millis(10), true);
    c.reset();
    assert_eq!(c.connect_count(), 0);
}

#[test]
fn stats_collector_reset_stat_enable() {
    let c = make_collector();
    assert!(c.is_reset_stat_enable());
    c.set_reset_stat_enable(false);
    assert!(!c.is_reset_stat_enable());
}
