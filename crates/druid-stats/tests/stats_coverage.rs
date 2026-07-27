//! Comprehensive coverage tests for druid-stats crate.
//!
//! Targets: merge.rs (75 uncovered), collector.rs (22 uncovered),
//! stat_filter.rs (8 uncovered).

use druid_core::*;
use druid_stats::*;
use std::sync::Arc;
use std::time::Duration;

// ══════════════════════════════════════════════════════════════════
// 1. merge.rs: fingerprint, parameterize, MergedSqlStat, SqlMerger
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_fingerprint_basic() {
    let fp1 = fingerprint("SELECT 1");
    let fp2 = fingerprint("SELECT 1");
    assert_eq!(fp1, fp2);
    let fp3 = fingerprint("SELECT 2");
    assert_ne!(fp1, fp3);
}

#[test]
fn test_parameterize_numbers() {
    let p = parameterize("SELECT * FROM t WHERE id = 42");
    assert_eq!(p.template, "SELECT * FROM t WHERE id = ?");
    assert!(p.fingerprint > 0);
}

#[test]
fn test_parameterize_strings() {
    let p = parameterize("SELECT * FROM t WHERE name = 'alice'");
    assert_eq!(p.template, "SELECT * FROM t WHERE name = ?");
}

#[test]
fn test_parameterize_same_fingerprint() {
    let p1 = parameterize("SELECT * FROM users WHERE id = 1");
    let p2 = parameterize("SELECT * FROM users WHERE id = 999");
    assert_eq!(p1.fingerprint, p2.fingerprint);
}

#[test]
fn test_parameterize_different_fingerprint() {
    let p1 = parameterize("SELECT * FROM users WHERE id = 1");
    let p2 = parameterize("SELECT * FROM orders WHERE id = 1");
    assert_ne!(p1.fingerprint, p2.fingerprint);
}

#[test]
fn test_parameterize_empty_string() {
    let p = parameterize("");
    assert_eq!(p.template, "");
}

#[test]
fn test_parameterize_multiple_numbers() {
    let p = parameterize("INSERT INTO t VALUES (1, 2, 3)");
    assert_eq!(p.template, "INSERT INTO t VALUES (?, ?, ?)");
}

#[test]
fn test_parameterize_mixed_types() {
    let p = parameterize("SELECT * FROM t WHERE id = 1 AND name = 'test' AND val = 3.14");
    assert!(p.template.contains('?'));
}

#[test]
fn test_parameterize_string_with_numbers() {
    let p = parameterize("SELECT * FROM t WHERE name = 'item123'");
    assert_eq!(p.template, "SELECT * FROM t WHERE name = ?");
}

#[test]
fn test_parameterize_unterminated_string() {
    let p = parameterize("SELECT * FROM t WHERE name = 'unterminated");
    // Should handle gracefully
    assert!(!p.template.is_empty());
}

#[test]
fn test_parameterize_number_at_end() {
    let p = parameterize("SELECT * FROM t WHERE id = 42");
    assert!(p.template.ends_with('?'));
}

// ══════════════════════════════════════════════════════════════════
// 2. MergedSqlStat: all methods
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_merged_sql_stat_new() {
    let stat = MergedSqlStat::new("SELECT 1".into(), 12345);
    assert_eq!(stat.sql, "SELECT 1");
    assert_eq!(stat.fingerprint, 12345);
    assert_eq!(stat.execute_count(), 0);
    assert_eq!(stat.total_time_ms(), 0.0);
    assert_eq!(stat.max_time_ms(), 0.0);
    assert_eq!(stat.error_count(), 0);
}

#[test]
fn test_merged_sql_stat_record_ok() {
    let stat = MergedSqlStat::new("SELECT 1".into(), 12345);
    stat.record(Duration::from_millis(10), true);
    stat.record(Duration::from_millis(20), true);
    assert_eq!(stat.execute_count(), 2);
    assert!(stat.total_time_ms() > 0.0);
    assert!(stat.max_time_ms() >= 20.0 - 0.1);
    assert_eq!(stat.error_count(), 0);
}

#[test]
fn test_merged_sql_stat_record_error() {
    let stat = MergedSqlStat::new("SELECT 1".into(), 12345);
    stat.record(Duration::from_millis(5), false);
    assert_eq!(stat.execute_count(), 1);
    assert_eq!(stat.error_count(), 1);
}

#[test]
fn test_merged_sql_stat_cas_max() {
    let stat = MergedSqlStat::new("SELECT 1".into(), 12345);
    stat.record(Duration::from_millis(5), true);
    stat.record(Duration::from_millis(100), true);
    stat.record(Duration::from_millis(10), true);
    assert!(stat.max_time_ms() >= 100.0 - 0.1);
}

#[test]
fn test_merged_sql_stat_concurrent_max() {
    let stat = Arc::new(MergedSqlStat::new("SELECT 1".into(), 12345));
    let mut handles = vec![];
    for i in 0..100 {
        let stat = stat.clone();
        handles.push(std::thread::spawn(move || {
            stat.record(Duration::from_millis(i), true);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(stat.execute_count(), 100);
    assert!(stat.max_time_ms() >= 99.0);
}

// ══════════════════════════════════════════════════════════════════
// 3. SqlMerger: all methods
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_sql_merger_new() {
    let merger = SqlMerger::new();
    assert!(merger.is_empty());
    assert_eq!(merger.len(), 0);
}

#[test]
fn test_sql_merger_default() {
    let merger = SqlMerger::default();
    assert!(merger.is_empty());
}

#[test]
fn test_sql_merger_record_same_template() {
    let merger = SqlMerger::new();
    merger.record("SELECT * FROM users WHERE id = 1", Duration::from_millis(10), true);
    merger.record("SELECT * FROM users WHERE id = 2", Duration::from_millis(15), true);
    assert_eq!(merger.len(), 1);
    let stats = merger.all_stats();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].execute_count(), 2);
}

#[test]
fn test_sql_merger_record_different_templates() {
    let merger = SqlMerger::new();
    merger.record("SELECT * FROM users", Duration::from_millis(5), true);
    merger.record("SELECT * FROM orders", Duration::from_millis(5), true);
    assert_eq!(merger.len(), 2);
}

#[test]
fn test_sql_merger_get_stat_by_fingerprint() {
    let merger = SqlMerger::new();
    merger.record("SELECT 1", Duration::from_millis(1), true);
    let fp = fingerprint("SELECT ?");
    let stat = merger.get_stat(fp);
    assert!(stat.is_some());
    assert_eq!(stat.unwrap().execute_count(), 1);
}

#[test]
fn test_sql_merger_get_stat_nonexistent() {
    let merger = SqlMerger::new();
    let stat = merger.get_stat(999999);
    assert!(stat.is_none());
}

#[test]
fn test_sql_merger_all_stats_empty() {
    let merger = SqlMerger::new();
    assert!(merger.all_stats().is_empty());
}

#[test]
fn test_sql_merger_concurrent() {
    let merger = Arc::new(SqlMerger::new());
    let mut handles = vec![];
    for i in 0..50 {
        let merger = merger.clone();
        let table = match i % 5 {
            0 => "users",
            1 => "orders",
            2 => "products",
            3 => "sessions",
            _ => "logs",
        };
        handles.push(std::thread::spawn(move || {
            merger.record(
                &format!("SELECT * FROM {table} WHERE id = {i}"),
                Duration::from_millis(i as u64),
                true,
            );
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    // 5 unique templates (users, orders, products, sessions, logs)
    assert_eq!(merger.len(), 5);
}

// ══════════════════════════════════════════════════════════════════
// 4. StatsCollector: all methods
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_stats_collector_new() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    assert_eq!(collector.name, "test");
    assert_eq!(collector.connect_count(), 0);
    assert_eq!(collector.slow_sql_count(), 0);
}

#[test]
fn test_stats_collector_default() {
    let collector = StatsCollector::default();
    assert_eq!(collector.name, "default");
    assert_eq!(collector.slow_sql_threshold, Duration::from_secs(2));
}

#[test]
fn test_stats_collector_record_sql_fast() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    collector.record_sql("SELECT 1", Duration::from_millis(10), true);
    assert_eq!(collector.slow_sql_count(), 0);
    assert_eq!(collector.sql_merger.len(), 1);
}

#[test]
fn test_stats_collector_record_sql_slow() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    collector.record_sql("SELECT 1", Duration::from_millis(200), true);
    assert_eq!(collector.slow_sql_count(), 1);
}

#[test]
fn test_stats_collector_record_sql_error() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    collector.record_sql("SELECT 1", Duration::from_millis(10), false);
    assert_eq!(collector.sql_merger.len(), 1);
    let stats = collector.sql_merger.all_stats();
    assert_eq!(stats[0].error_count(), 1);
}

#[test]
fn test_stats_collector_connect() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    collector.record_connect();
    collector.record_connect();
    collector.record_connect();
    assert_eq!(collector.connect_count(), 3);
}

#[test]
fn test_stats_collector_connect_error() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    collector.record_connect_error();
    assert_eq!(collector.connect_error_count.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
fn test_stats_collector_close() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    collector.record_close();
    assert_eq!(collector.close_count.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
fn test_stats_collector_multiple_sql_templates() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    collector.record_sql("SELECT * FROM users", Duration::from_millis(10), true);
    collector.record_sql("SELECT * FROM orders", Duration::from_millis(20), true);
    collector.record_sql("SELECT * FROM users", Duration::from_millis(15), true);
    assert_eq!(collector.sql_merger.len(), 2);
}

// ══════════════════════════════════════════════════════════════════
// 5. StatFilter: all methods
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_stat_filter_name() {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_millis(100)));
    let filter = StatFilter::new(collector);
    assert_eq!(filter.name(), "stat");
}

#[tokio::test]
async fn test_stat_filter_after_ok() {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_millis(100)));
    let filter = StatFilter::new(collector.clone());
    let params = vec![];
    let ctx = ExecContext {
        sql: "SELECT 1",
        params: &params,
        data_source: "test",
        start: std::time::Instant::now(),
        fingerprint: None,
    };
    filter.after(&ctx, &Ok(ExecResult { rows_affected: 1, last_insert_id: None, row_count: None }), Duration::from_millis(5)).await;
    assert_eq!(collector.sql_merger.len(), 1);
}

#[tokio::test]
async fn test_stat_filter_after_error() {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_millis(100)));
    let filter = StatFilter::new(collector.clone());
    let params = vec![];
    let ctx = ExecContext {
        sql: "SELECT 1",
        params: &params,
        data_source: "test",
        start: std::time::Instant::now(),
        fingerprint: None,
    };
    filter.after(&ctx, &Err(DruidError::Other("fail".into())), Duration::from_millis(10)).await;
    let stats = collector.sql_merger.all_stats();
    assert_eq!(stats[0].error_count(), 1);
}

#[tokio::test]
async fn test_stat_filter_after_slow_sql() {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_millis(100)));
    let filter = StatFilter::new(collector.clone());
    let params = vec![];
    let ctx = ExecContext {
        sql: "SELECT 1",
        params: &params,
        data_source: "test",
        start: std::time::Instant::now(),
        fingerprint: None,
    };
    filter.after(&ctx, &Ok(ExecResult::default()), Duration::from_millis(200)).await;
    assert_eq!(collector.slow_sql_count(), 1);
}

#[tokio::test]
async fn test_stat_filter_after_connection_close() {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_millis(100)));
    let filter = StatFilter::new(collector);
    // after_connection_close is a no-op default, just call it
    filter.after_connection_close().await;
}

// ══════════════════════════════════════════════════════════════════
// MergedSqlStat: CAS retry branch (L97: Err(actual) => current = actual)
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_merged_sql_stat_cas_retry_concurrent() {
    // Heavy concurrent writes to force CAS retry in the max update loop
    let stat = Arc::new(MergedSqlStat::new("SELECT 1".into(), 12345));
    let mut handles = vec![];
    // Use more threads and higher values to increase chance of CAS retry
    for i in 0..500 {
        let stat = stat.clone();
        handles.push(std::thread::spawn(move || {
            stat.record(Duration::from_millis(i * 10), true);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(stat.execute_count(), 500);
    assert!(stat.max_time_ms() >= 4990.0);
}

#[test]
fn test_merged_sql_stat_cas_retry_direct() {
    // Force CAS retry by pre-setting max_time_ns to a value,
    // then having multiple threads try to update it simultaneously
    use std::sync::atomic::AtomicU64;
    let stat = Arc::new(MergedSqlStat::new("SELECT 1".into(), 12345));
    // Pre-set max_time_ns to a non-zero value
    stat.max_time_ns.store(1000, std::sync::atomic::Ordering::Relaxed);
    let mut handles = vec![];
    // Many threads all trying to set a larger value - forces CAS contention
    for i in 0..1000 {
        let stat = stat.clone();
        handles.push(std::thread::spawn(move || {
            stat.record(Duration::from_millis((i + 1) * 100), true);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(stat.execute_count(), 1000);
    assert!(stat.max_time_ms() >= 100_000.0);
}
