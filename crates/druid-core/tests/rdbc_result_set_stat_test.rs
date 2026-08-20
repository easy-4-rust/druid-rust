extern crate druid_core as druid;
use druid_core::core::DruidError;
use druid_core::stats::RdbcResultSetStat;
use std::sync::Arc;
use std::thread;

#[test]
fn rdbc_result_set_stat_preserves_java_counters_times_and_reset_scope() {
    let stat = RdbcResultSetStat::new();
    assert_eq!(stat.opening_count(), 0);
    assert_eq!(stat.opening_max(), 0);
    assert_eq!(stat.last_open_time_millis(), None);
    assert_eq!(stat.last_error(), None);
    assert_eq!(stat.last_error_time_millis(), None);

    stat.before_open();
    stat.before_open();
    stat.after_close(2_500_000);
    stat.add_fetch_row_count(7);
    stat.increment_close_counter();
    stat.error(DruidError::DriverError("read failed".to_string()));

    assert_eq!(stat.opening_count(), 1);
    assert_eq!(stat.opening_max(), 2);
    assert_eq!(stat.open_count(), 2);
    assert!(stat.last_open_time_millis().is_some());
    assert_eq!(stat.alive_nano_total(), 2_500_000);
    assert_eq!(stat.alive_millis_total(), 2);
    assert_eq!(stat.hold_millis_total(), 2);
    assert_eq!(stat.alive_millis_max(), 2);
    assert_eq!(stat.alive_millis_min(), 0);
    assert_eq!(stat.fetch_row_count(), 7);
    assert_eq!(stat.close_count(), 1);
    assert_eq!(stat.error_count(), 1);
    assert_eq!(
        stat.last_error(),
        Some(DruidError::DriverError("read failed".to_string()))
    );
    assert!(stat.last_error_time_millis().is_some());

    stat.reset();
    assert_eq!(stat.opening_count(), 1);
    assert_eq!(stat.opening_max(), 0);
    assert_eq!(stat.open_count(), 0);
    assert_eq!(stat.alive_nano_total(), 0);
    assert_eq!(stat.alive_millis_min(), 0);
    assert_eq!(stat.alive_millis_max(), 0);
    assert_eq!(stat.fetch_row_count(), 0);
    assert_eq!(stat.close_count(), 0);
    assert_eq!(stat.error_count(), 0);
    assert_eq!(stat.last_open_time_millis(), None);
    assert_eq!(stat.last_error(), None);
    assert_eq!(stat.last_error_time_millis(), None);
}

#[test]
fn rdbc_result_set_stat_updates_opening_peak_atomically() {
    let stat = Arc::new(RdbcResultSetStat::default());
    let mut workers = Vec::new();
    for _ in 0..16 {
        let stat = Arc::clone(&stat);
        workers.push(thread::spawn(move || stat.before_open()));
    }
    for worker in workers {
        worker.join().expect("worker must finish");
    }

    assert_eq!(stat.opening_count(), 16);
    assert_eq!(stat.opening_max(), 16);
    assert_eq!(stat.open_count(), 16);

    let mut workers = Vec::new();
    for nanos in 1..=16 {
        let stat = Arc::clone(&stat);
        workers.push(thread::spawn(move || stat.after_close(nanos)));
    }
    for worker in workers {
        worker.join().expect("worker must finish");
    }

    assert_eq!(stat.opening_count(), 0);
    assert_eq!(stat.alive_nano_total(), 136);
    assert_eq!(stat.alive_millis_max(), 0);
    assert_eq!(stat.alive_millis_min(), 0);
}

#[test]
fn result_set_stat_default_trait() {
    let stat = RdbcResultSetStat::default();
    assert_eq!(stat.open_count(), 0);
}

#[test]
fn result_set_stat_alive_millis_min_max() {
    let stat = RdbcResultSetStat::new();
    stat.before_open();
    stat.after_close(10_000_000); // 10ms
    stat.before_open();
    stat.after_close(50_000_000); // 50ms
    assert!(stat.alive_millis_min() <= stat.alive_millis_max());
    assert!(stat.alive_millis_max() >= 50);
}

#[test]
fn result_set_stat_multiple_errors() {
    let stat = RdbcResultSetStat::new();
    stat.error(DruidError::Other("err1".to_owned()));
    stat.error(DruidError::Other("err2".to_owned()));
    stat.error(DruidError::Other("err3".to_owned()));
    assert_eq!(stat.error_count(), 3);
}
