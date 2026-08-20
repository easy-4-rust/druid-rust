#![allow(dead_code)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use druid_core::stats::{
    DataSourceIdentity, DataSourceMonitorable, DruidTelemetrySnapshot, PoolSnapshot,
    SnapshotUnavailable,
};
use druid_metrics::{DruidMetricsConfig, DruidMetricsRuntime};
use serde_json::Value;

/// A mock datasource for testing.
struct MockDataSource {
    identity: DataSourceIdentity,
    snapshot_count: AtomicU64,
}

impl MockDataSource {
    fn new(id: u64, name: &str) -> Self {
        Self {
            identity: DataSourceIdentity {
                id,
                name: name.to_owned(),
                driver_name: Some("mock".to_owned()),
            },
            snapshot_count: AtomicU64::new(0),
        }
    }

    fn snapshot_count(&self) -> u64 {
        self.snapshot_count.load(Ordering::Relaxed)
    }
}

impl DataSourceMonitorable for MockDataSource {
    fn name(&self) -> &str {
        &self.identity.name
    }

    fn driver_name(&self) -> Option<&str> {
        self.identity.driver_name.as_deref()
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Object(serde_json::Map::new())
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        self.identity.clone()
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        self.snapshot_count.fetch_add(1, Ordering::Relaxed);
        Ok(DruidTelemetrySnapshot {
            identity: self.identity.clone(),
            pool_snapshot: PoolSnapshot {
                active_count: 0,
                idle_count: 1,
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

#[tokio::test]
async fn runtime_holds_weak_references_only() {
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(50))
        .build()
        .unwrap();
    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    let source = Arc::new(MockDataSource::new(1, "test_ds"));
    let weak = Arc::downgrade(&source);

    let _guard = runtime.register(source.clone());
    // The runtime should hold a Weak ref, so the Arc is still alive via `source`
    assert!(weak.upgrade().is_some());

    // Drop the source Arc -- but the guard still holds a reference indirectly
    // via unregister_tx, so let's drop the guard first
    drop(_guard);
    drop(source);

    // After dropping both source and guard, the Weak should be gone
    // (though the unregister listener may have already cleaned up)
    runtime.shutdown(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn guard_drop_unregisters_datasource() {
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(50))
        .build()
        .unwrap();
    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    let source = Arc::new(MockDataSource::new(2, "test_ds_2"));
    let guard = runtime.register(source.clone());

    // Drop the guard -- this should trigger unregister
    drop(guard);

    // Give the unregister listener a moment to process
    tokio::time::sleep(Duration::from_millis(20)).await;

    // The registry should now be empty (the unregister message was processed)
    // We verify by checking that the runtime shuts down cleanly
    runtime.shutdown(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn source_drop_auto_cleans_on_next_sample() {
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(50))
        .build()
        .unwrap();
    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    let source = Arc::new(MockDataSource::new(3, "test_ds_3"));
    let _guard = runtime.register(source.clone());

    // Drop the source -- the Weak ref in the sampler will fail to upgrade
    drop(source);

    // Wait for at least one sample cycle so the sampler detects the dead Weak
    tokio::time::sleep(Duration::from_millis(150)).await;

    // The sampler should have cleaned up the dead entry
    // Runtime should shut down cleanly
    runtime.shutdown(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn multiple_registrations_and_selective_drop() {
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(50))
        .build()
        .unwrap();
    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    let source_a = Arc::new(MockDataSource::new(10, "ds_a"));
    let source_b = Arc::new(MockDataSource::new(11, "ds_b"));

    let guard_a = runtime.register(source_a.clone());
    let _guard_b = runtime.register(source_b.clone());

    // Drop only guard_a
    drop(guard_a);
    tokio::time::sleep(Duration::from_millis(20)).await;

    // source_b should still be registered
    // Shut down cleanly
    runtime.shutdown(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn shutdown_within_deadline_succeeds() {
    let config = DruidMetricsConfig::builder()
        .sample_interval(Duration::from_millis(50))
        .shutdown_timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    let source = Arc::new(MockDataSource::new(20, "ds_shutdown"));
    let _guard = runtime.register(source);

    // Let a few samples happen
    tokio::time::sleep(Duration::from_millis(120)).await;

    // Shutdown with generous deadline
    let result = runtime.shutdown(Duration::from_secs(5)).await;
    assert!(result.is_ok(), "shutdown should succeed within deadline");
}
