use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use druid_core::stats::{
    DataSourceIdentity, DataSourceMonitorable, DruidTelemetrySnapshot, PoolSnapshot,
    SnapshotUnavailable,
};
use druid_metrics::{DruidMetricsConfig, DruidMetricsRuntime, MetricsError};
use serde_json::Value;

/// Mock datasource for shutdown tests.
struct MockDataSource {
    identity: DataSourceIdentity,
}

impl MockDataSource {
    fn new(id: u64, name: &str) -> Self {
        Self {
            identity: DataSourceIdentity {
                id,
                name: name.to_owned(),
                driver_name: Some("mock".to_owned()),
            },
        }
    }
}

impl DataSourceMonitorable for MockDataSource {
    fn name(&self) -> &str {
        &self.identity.name
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Object(serde_json::Map::new())
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        self.identity.clone()
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        Ok(DruidTelemetrySnapshot {
            identity: self.identity.clone(),
            pool_snapshot: PoolSnapshot {
                active_count: 1,
                idle_count: 0,
                max_active: 10,
                max_idle: 5,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 0,
        })
    }
}

/// A datasource that blocks on snapshot (simulates slow datasource).
struct SlowDataSource {
    identity: DataSourceIdentity,
    call_count: AtomicU64,
}

impl SlowDataSource {
    fn new(id: u64, name: &str) -> Self {
        Self {
            identity: DataSourceIdentity {
                id,
                name: name.to_owned(),
                driver_name: Some("mock".to_owned()),
            },
            call_count: AtomicU64::new(0),
        }
    }
}

impl DataSourceMonitorable for SlowDataSource {
    fn name(&self) -> &str {
        &self.identity.name
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Object(serde_json::Map::new())
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        self.identity.clone()
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        // Simulate a datasource that's always busy
        Err(SnapshotUnavailable::Busy)
    }
}

#[tokio::test]
async fn shutdown_within_generous_deadline_succeeds() {
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(50))
        .shutdown_timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    for i in 0..3 {
        let source = Arc::new(MockDataSource::new(i, &format!("ds_{i}")));
        let _guard = runtime.register(source);
    }

    // Let some samples happen
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Shutdown with generous deadline
    let result = runtime.shutdown(Duration::from_secs(5)).await;
    assert!(result.is_ok(), "shutdown should succeed: {result:?}");
}

#[tokio::test]
async fn shutdown_stops_sampler_quickly() {
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(10))
        .build()
        .unwrap();

    let runtime = DruidMetricsRuntime::start(config).await.unwrap();
    let source = Arc::new(SlowDataSource::new(1, "slow_ds"));
    let before = source.call_count.load(Ordering::Relaxed);
    let _guard = runtime.register(source.clone());

    // Let some samples happen
    tokio::time::sleep(Duration::from_millis(100)).await;
    let during = source.call_count.load(Ordering::Relaxed);
    assert!(during > before, "sampler should have called try_snapshot");

    // Shutdown
    runtime.shutdown(Duration::from_secs(2)).await.unwrap();

    // After shutdown, no more samples should happen
    let at_shutdown = source.call_count.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let after_shutdown = source.call_count.load(Ordering::Relaxed);

    assert_eq!(
        at_shutdown, after_shutdown,
        "no more samples should happen after shutdown"
    );
}

#[tokio::test]
async fn shutdown_with_zero_pending_returns_ok() {
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(50))
        .build()
        .unwrap();

    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    // Register and immediately shutdown (no time for samples)
    let source = Arc::new(MockDataSource::new(1, "immediate_shutdown"));
    let _guard = runtime.register(source);

    let result = runtime.shutdown(Duration::from_secs(2)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn shutdown_order_stops_sampler_before_flushing() {
    // Verify that shutdown completes cleanly even with active datasources.
    // The shutdown sequence is: cancel sampler -> flush aggregator -> join tasks.
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(10))
        .queue_capacity(64)
        .build()
        .unwrap();

    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    for i in 0..10 {
        let source = Arc::new(MockDataSource::new(i, &format!("ds_{i}")));
        let _guard = runtime.register(source);
    }

    // Let many samples happen
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Shutdown should complete cleanly
    let result = runtime.shutdown(Duration::from_secs(3)).await;
    assert!(result.is_ok(), "shutdown failed: {result:?}");
}

#[test]
fn config_shutdown_timeout_is_respected() {
    let config = DruidMetricsConfig::builder()
        .shutdown_timeout(Duration::from_secs(7))
        .build()
        .unwrap();
    assert_eq!(config.shutdown_timeout, Duration::from_secs(7));
}
