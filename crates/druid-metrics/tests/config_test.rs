use druid_metrics::{DruidMetricsConfig, MetricsConfigError, SqlTextPolicy};
use std::time::Duration;

#[test]
fn default_config_has_expected_values() {
    let cfg = DruidMetricsConfig::default();

    assert_eq!(cfg.sample_interval, Duration::from_secs(15));
    assert_eq!(cfg.queue_capacity, 1024);
    assert_eq!(cfg.batch_size, 64);
    assert_eq!(cfg.coalesce_window, Duration::from_millis(500));
    assert_eq!(cfg.max_sql_statements, 256);
    assert!(matches!(cfg.sql_text_policy, SqlTextPolicy::FingerprintOnly));
    assert_eq!(cfg.shutdown_timeout, Duration::from_secs(5));
}

#[test]
fn zero_queue_capacity_is_rejected() {
    let result = DruidMetricsConfig::builder()
        .queue_capacity(0)
        .build();
    assert!(matches!(result, Err(MetricsConfigError::InvalidQueueCapacity)));
}

#[test]
fn zero_batch_size_is_rejected() {
    let result = DruidMetricsConfig::builder()
        .batch_size(0)
        .build();
    assert!(matches!(result, Err(MetricsConfigError::InvalidBatchSize)));
}

#[test]
fn zero_coalesce_window_is_rejected() {
    let result = DruidMetricsConfig::builder()
        .coalesce_window(Duration::ZERO)
        .build();
    assert!(matches!(result, Err(MetricsConfigError::InvalidCoalesceWindow)));
}

#[test]
fn zero_sample_interval_is_rejected() {
    let result = DruidMetricsConfig::builder()
        .sample_interval(Duration::ZERO)
        .build();
    assert!(matches!(result, Err(MetricsConfigError::InvalidSampleInterval)));
}

#[test]
fn zero_flush_interval_is_rejected() {
    // flush_interval is not directly configurable in V1, but shutdown_timeout must be non-zero
    let result = DruidMetricsConfig::builder()
        .shutdown_timeout(Duration::ZERO)
        .build();
    assert!(matches!(result, Err(MetricsConfigError::InvalidShutdownTimeout)));
}

#[test]
fn builder_produces_correct_values() {
    let cfg = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_secs(30))
        .queue_capacity(2048)
        .batch_size(128)
        .coalesce_window(Duration::from_secs(1))
        .max_sql_statements(512)
        .sql_text_policy(SqlTextPolicy::Disabled)
        .shutdown_timeout(Duration::from_secs(10))
        .build()
        .expect("valid config");

    assert_eq!(cfg.sample_interval, Duration::from_secs(30));
    assert_eq!(cfg.queue_capacity, 2048);
    assert_eq!(cfg.batch_size, 128);
    assert_eq!(cfg.coalesce_window, Duration::from_secs(1));
    assert_eq!(cfg.max_sql_statements, 512);
    assert!(matches!(cfg.sql_text_policy, SqlTextPolicy::Disabled));
    assert_eq!(cfg.shutdown_timeout, Duration::from_secs(10));
}
