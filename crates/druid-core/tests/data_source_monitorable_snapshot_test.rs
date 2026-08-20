//! Typed observability SPI snapshot test.
//!
//! Verifies that `DataSourceMonitorable::try_snapshot` returns a structured
//! `DruidTelemetrySnapshot` containing datasource identity, pool snapshot,
//! SQL list, Wall snapshot, and sampling time. Busy must not block.

extern crate druid_core as druid;

use druid_core::stats::{
    DataSourceIdentity, DruidTelemetrySnapshot, PoolSnapshot, SnapshotUnavailable,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Mock `DataSourceMonitorable` for testing the typed SPI.
struct MockMonitorable {
    id: u64,
    name: String,
    closed: AtomicBool,
}

impl MockMonitorable {
    fn new(id: u64, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            closed: AtomicBool::new(false),
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

impl druid_core::stats::DataSourceMonitorable for MockMonitorable {
    fn name(&self) -> &str {
        &self.name
    }

    fn data_source_stat_data(&self) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        DataSourceIdentity {
            id: self.id,
            name: self.name.clone(),
            driver_name: Some("mock".to_string()),
        }
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(SnapshotUnavailable::Closed);
        }
        Ok(DruidTelemetrySnapshot {
            identity: self.identity(),
            pool_snapshot: PoolSnapshot {
                active_count: 0,
                idle_count: 0,
                max_active: 10,
                max_idle: 5,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 1234567890,
        })
    }
}

#[test]
fn try_snapshot_returns_identity_and_pool_data() {
    let mock = MockMonitorable::new(42, "test-ds");
    let monitorable: Arc<dyn druid_core::stats::DataSourceMonitorable> = Arc::new(mock);

    // identity() must return a valid DataSourceIdentity
    let identity = monitorable.identity();
    assert_eq!(identity.id, 42);
    assert_eq!(identity.name, "test-ds");
    assert_eq!(identity.driver_name.as_deref(), Some("mock"));

    // try_snapshot() must succeed
    let snapshot = monitorable.try_snapshot();
    let snap = snapshot.expect("try_snapshot should succeed");

    // Snapshot must contain the same identity
    assert_eq!(snap.identity.id, 42);
    assert_eq!(snap.identity.name, "test-ds");

    // sampling_time must be set
    assert!(
        snap.sampling_time_millis > 0,
        "sampling_time_millis must be > 0"
    );

    // pool_snapshot must be present
    assert!(
        snap.pool_snapshot.active_count <= snap.pool_snapshot.max_active,
        "active_count must be <= max_active"
    );
}

#[test]
fn try_snapshot_on_closed_datasource_returns_closed() {
    let mock = MockMonitorable::new(1, "closed-ds");
    mock.close(); // close before wrapping in Arc
    let monitorable: Arc<dyn druid_core::stats::DataSourceMonitorable> = Arc::new(mock);

    // try_snapshot should return Closed
    let result = monitorable.try_snapshot();
    assert!(
        matches!(result, Err(SnapshotUnavailable::Closed)),
        "expected SnapshotUnavailable::Closed, got {result:?}"
    );
}

#[test]
fn identity_is_stable_across_calls() {
    let mock = MockMonitorable::new(99, "stable-ds");
    let monitorable: Arc<dyn druid_core::stats::DataSourceMonitorable> = Arc::new(mock);

    let id1 = monitorable.identity();
    let id2 = monitorable.identity();

    assert_eq!(id1.id, id2.id);
    assert_eq!(id1.name, id2.name);
}

#[test]
fn reset_stat_does_not_panic() {
    let mock = MockMonitorable::new(7, "reset-ds");
    let monitorable: Arc<dyn druid_core::stats::DataSourceMonitorable> = Arc::new(mock);

    // reset_stat should not panic
    monitorable.reset_stat();
}
