//! druid-stats S4 验收测试

use druid::core::*;
use druid::stats::*;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_parameterize_numbers() {
    let p = parameterize("SELECT * FROM users WHERE id = 42");
    assert_eq!(p.template, "SELECT * FROM users WHERE id = ?");
    assert!(p.fingerprint > 0);
}

#[test]
fn test_parameterize_strings() {
    let p = parameterize("SELECT * FROM users WHERE name = 'alice'");
    assert_eq!(p.template, "SELECT * FROM users WHERE name = ?");
}

#[test]
fn test_parameterize_same_fingerprint() {
    let p1 = parameterize("SELECT * FROM users WHERE id = 1");
    let p2 = parameterize("SELECT * FROM users WHERE id = 999");
    assert_eq!(
        p1.fingerprint, p2.fingerprint,
        "same template should have same fingerprint"
    );
}

#[test]
fn test_parameterize_different_fingerprint() {
    let p1 = parameterize("SELECT * FROM users WHERE id = 1");
    let p2 = parameterize("SELECT * FROM orders WHERE id = 1");
    assert_ne!(p1.fingerprint, p2.fingerprint);
}

#[test]
fn test_fingerprint_stability() {
    let fp1 = fingerprint("SELECT * FROM t WHERE id = ?");
    let fp2 = fingerprint("SELECT * FROM t WHERE id = ?");
    assert_eq!(fp1, fp2);
}

#[test]
fn test_merged_sql_stat_record() {
    let stat = MergedSqlStat::new("SELECT 1".into(), 12345);
    stat.record(Duration::from_millis(10), true);
    stat.record(Duration::from_millis(20), true);
    stat.record(Duration::from_millis(5), false);

    assert_eq!(stat.execute_count(), 3);
    assert_eq!(stat.error_count(), 1);
    assert!(stat.total_time_ms() > 0.0);
    assert!(stat.max_time_ms() >= 20.0 - 0.1, "max should be ~20ms");
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
fn test_sql_merger_record_and_get() {
    let merger = SqlMerger::new();
    merger.record(
        "SELECT * FROM users WHERE id = 1",
        Duration::from_millis(10),
        true,
    );
    merger.record(
        "SELECT * FROM users WHERE id = 2",
        Duration::from_millis(15),
        true,
    );

    // Same template -> same stat
    assert_eq!(merger.len(), 1);

    let stats = merger.all_stats();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].execute_count(), 2);
}

#[test]
fn test_sql_merger_different_templates() {
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
fn test_stats_collector_record_sql() {
    let collector = StatsCollector::new("test", Duration::from_millis(100));
    collector.record_sql("SELECT 1", Duration::from_millis(10), true);
    collector.record_sql("SELECT 1", Duration::from_millis(200), false); // slow

    assert_eq!(collector.connect_count(), 0);
    assert!(collector.slow_sql_count() >= 1, "should detect slow SQL");
    assert_eq!(collector.sql_merger.len(), 1);
}

#[test]
fn test_stats_collector_connect() {
    let collector = StatsCollector::default();
    collector.record_connect();
    collector.record_connect();
    collector.record_connect_error();
    assert_eq!(collector.connect_count(), 2);
    collector.record_execute_batch(3);
    collector.record_execute_batch(2);
    assert_eq!(collector.execute_batch_count(), 2);
    assert_eq!(collector.execute_batch_size_total(), 5);
}

#[tokio::test]
async fn test_stat_filter_as_after_filter() {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_millis(100)));
    let filter = StatFilter::new(collector.clone());

    assert_eq!(AfterFilter::name(&filter), "stat");

    let params = vec![];
    let ctx = ExecContext {
        connection_id: 7,
        statement_id: Some(20_001),
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: std::time::Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: druid::core::ExecOperation::Update,
    };

    // ok execution
    filter
        .after(
            &ctx,
            &Ok(ExecResult {
                rows_affected: 1,
                last_insert_id: None,
                row_count: None,
            }),
            Duration::from_millis(5),
        )
        .await
        .unwrap();

    // error execution
    filter
        .after(
            &ctx,
            &Err(DruidError::Other("fail".into())),
            Duration::from_millis(10),
        )
        .await
        .unwrap();

    assert_eq!(collector.sql_merger.len(), 1);
    let stats = collector.sql_merger.all_stats();
    assert_eq!(stats[0].execute_count(), 2);
    assert_eq!(stats[0].error_count(), 1);
}
