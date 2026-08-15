use druid::stats::RdbcStatementStat;
use std::time::Duration;

#[test]
fn statement_stat_new() {
    let stat = RdbcStatementStat::new();
    assert_eq!(stat.create_count(), 0);
    assert_eq!(stat.prepare_count(), 0);
    assert_eq!(stat.prepare_call_count(), 0);
    assert_eq!(stat.close_count(), 0);
    assert_eq!(stat.running_count(), 0);
    assert_eq!(stat.concurrent_max(), 0);
    assert_eq!(stat.execute_count(), 0);
    assert_eq!(stat.error_count(), 0);
    assert_eq!(stat.nano_total(), 0);
    assert!(stat.last_error().is_none());
    assert!(stat.last_error_time_millis().is_none());
}

#[test]
fn statement_stat_before_after_execute() {
    let stat = RdbcStatementStat::new();
    stat.before_execute();
    assert_eq!(stat.running_count(), 1);
    assert_eq!(stat.execute_count(), 1);
    assert_eq!(stat.concurrent_max(), 1);

    stat.before_execute();
    assert_eq!(stat.running_count(), 2);
    assert_eq!(stat.concurrent_max(), 2);

    stat.after_execute(Duration::from_millis(5));
    assert_eq!(stat.running_count(), 1);
    assert!(stat.nano_total() > 0);
}

#[test]
fn statement_stat_concurrent_max_tracks_peak() {
    let stat = RdbcStatementStat::new();
    stat.before_execute();
    stat.before_execute();
    stat.before_execute();
    assert_eq!(stat.concurrent_max(), 3);
    stat.after_execute(Duration::from_millis(1));
    stat.after_execute(Duration::from_millis(1));
    assert_eq!(stat.concurrent_max(), 3);
}

#[test]
fn statement_stat_error() {
    let stat = RdbcStatementStat::new();
    stat.error("connection lost");
    assert_eq!(stat.error_count(), 1);
    assert_eq!(stat.last_error(), Some("connection lost".to_owned()));
    assert!(stat.last_error_time_millis().is_some());
}

#[test]
fn statement_stat_error_overwrites_last() {
    let stat = RdbcStatementStat::new();
    stat.error("first");
    stat.error("second");
    assert_eq!(stat.error_count(), 2);
    assert_eq!(stat.last_error(), Some("second".to_owned()));
}

#[test]
fn statement_stat_increment_create_counter() {
    let stat = RdbcStatementStat::new();
    stat.increment_create_counter();
    stat.increment_create_counter();
    assert_eq!(stat.create_count(), 2);
}

#[test]
fn statement_stat_increment_prepare_counter() {
    let stat = RdbcStatementStat::new();
    stat.increment_prepare_counter();
    assert_eq!(stat.prepare_count(), 1);
}

#[test]
fn statement_stat_increment_prepare_call_count() {
    let stat = RdbcStatementStat::new();
    stat.increment_prepare_call_count();
    assert_eq!(stat.prepare_call_count(), 1);
}

#[test]
fn statement_stat_increment_statement_close_counter() {
    let stat = RdbcStatementStat::new();
    stat.increment_statement_close_counter();
    stat.increment_statement_close_counter();
    assert_eq!(stat.close_count(), 2);
}

#[test]
fn statement_stat_execute_success_count() {
    let stat = RdbcStatementStat::new();
    stat.before_execute();
    stat.after_execute(Duration::from_millis(1));
    stat.before_execute();
    stat.after_execute(Duration::from_millis(1));
    stat.error("fail");
    assert_eq!(stat.execute_success_count(), 1);
}

#[test]
fn statement_stat_millis_total() {
    let stat = RdbcStatementStat::new();
    stat.after_execute(Duration::from_millis(10));
    assert!(stat.millis_total() >= 10);
}

#[test]
fn statement_stat_histogram_buckets() {
    let stat = RdbcStatementStat::new();
    // < 10ms
    stat.after_execute(Duration::from_millis(5));
    // 10-100ms
    stat.after_execute(Duration::from_millis(50));
    // 100-1000ms
    stat.after_execute(Duration::from_millis(500));
    // 1000-10000ms
    stat.after_execute(Duration::from_millis(5000));
    // > 10000ms
    stat.after_execute(Duration::from_millis(15000));
    assert_eq!(stat.nano_total(), stat.nano_total());
}

#[test]
fn statement_stat_execute_last_time_millis_none() {
    let stat = RdbcStatementStat::new();
    assert!(stat.execute_last_time_millis().is_none());
}

#[test]
fn statement_stat_execute_last_time_millis_some() {
    let stat = RdbcStatementStat::new();
    stat.before_execute();
    assert!(stat.execute_last_time_millis().is_some());
}

#[test]
fn statement_stat_histogram_ranges() {
    let ranges = RdbcStatementStat::histogram_ranges();
    assert_eq!(ranges, [10, 100, 1_000, 10_000]);
}

#[test]
fn statement_stat_histogram_values_default() {
    let stat = RdbcStatementStat::new();
    let values = stat.histogram_values();
    assert_eq!(values.len(), 5);
    assert!(values.iter().all(|&v| v == 0));
}

#[test]
fn statement_stat_histogram_values_after_execute() {
    let stat = RdbcStatementStat::new();
    stat.after_execute(Duration::from_millis(5));
    stat.after_execute(Duration::from_millis(50));
    let values = stat.histogram_values();
    assert!(values.iter().any(|&v| v > 0));
}

#[test]
fn statement_stat_reset() {
    let stat = RdbcStatementStat::new();
    stat.increment_create_counter();
    stat.increment_prepare_counter();
    stat.increment_prepare_call_count();
    stat.increment_statement_close_counter();
    stat.before_execute();
    stat.after_execute(Duration::from_millis(10));
    stat.error("test error");

    stat.reset();
    assert_eq!(stat.create_count(), 0);
    assert_eq!(stat.prepare_count(), 0);
    assert_eq!(stat.prepare_call_count(), 0);
    assert_eq!(stat.close_count(), 0);
    assert_eq!(stat.running_count(), 0);
    assert_eq!(stat.concurrent_max(), 0);
    assert_eq!(stat.execute_count(), 0);
    assert_eq!(stat.error_count(), 0);
    assert_eq!(stat.nano_total(), 0);
    assert!(stat.last_error().is_none());
    assert!(stat.last_error_time_millis().is_none());
    assert!(stat.execute_last_time_millis().is_none());
}

#[test]
fn statement_stat_default() {
    let stat = RdbcStatementStat::default();
    assert_eq!(stat.execute_count(), 0);
}
