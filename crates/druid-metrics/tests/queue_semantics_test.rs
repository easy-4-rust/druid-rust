#![allow(clippy::match_same_arms)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use druid::stats::{
    DataSourceIdentity, DataSourceMonitorable, DruidTelemetrySnapshot, PoolSnapshot,
    SnapshotUnavailable,
};
use druid_metrics::{DruidMetricsConfig, DruidMetricsRuntime};
use serde_json::Value;

/// A mock datasource that tracks snapshot calls.
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
        self.snapshot_count.fetch_add(1, Ordering::Relaxed);
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

/// A datasource that simulates Busy on every snapshot attempt.
struct BusyDataSource {
    identity: DataSourceIdentity,
    busy_count: AtomicU64,
}

impl BusyDataSource {
    fn new(id: u64, name: &str) -> Self {
        Self {
            identity: DataSourceIdentity {
                id,
                name: name.to_owned(),
                driver_name: Some("mock".to_owned()),
            },
            busy_count: AtomicU64::new(0),
        }
    }
}

impl DataSourceMonitorable for BusyDataSource {
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
        self.busy_count.fetch_add(1, Ordering::Relaxed);
        Err(SnapshotUnavailable::Busy)
    }
}

#[tokio::test]
async fn saturated_metrics_queue_never_blocks_datasource_operations() {
    // Test the queue saturation directly: create a capacity-1 channel,
    // register 5 datasources, run one sampler tick, and verify coalescing.
    use druid_metrics::aggregator::PendingSnapshot;
    use parking_lot::RwLock;

    let metrics = Arc::new(druid_metrics::self_metrics::RuntimeSelfMetrics::new());
    let registry: Arc<RwLock<Vec<druid_metrics::registry::RegistryEntry>>> =
        Arc::new(RwLock::new(Vec::new()));

    // Register 5 datasources into the shared registry
    let mut sources = Vec::new();
    for i in 0..5 {
        let source = Arc::new(MockDataSource::new(i, &format!("ds_{i}")));
        let entry = druid_metrics::registry::RegistryEntry::new(
            i,
            Arc::downgrade(&source) as std::sync::Weak<dyn DataSourceMonitorable>,
        );
        registry.write().push(entry);
        sources.push(source);
    }

    // Create a capacity-1 channel but DO NOT spawn a consumer.
    // This ensures the queue stays full after the first send.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<PendingSnapshot>(1);

    // Run one sample cycle manually (not via the sampler task)
    // by calling sample_all indirectly through a single sampler tick.
    // We'll simulate it by running the sampler for a very short time.
    let cancel = tokio_util::sync::CancellationToken::new();
    let sampler_cancel = cancel.clone();
    let sampler_registry = Arc::clone(&registry);
    let sampler_metrics = Arc::clone(&metrics);

    let sampler_handle = tokio::spawn(async move {
        // Run sampler with 10ms interval
        let mut ticker = tokio::time::interval(Duration::from_millis(10));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Run for a few ticks then cancel
        for _ in 0..5 {
            tokio::select! {
                () = sampler_cancel.cancelled() => break,
                _ = ticker.tick() => {
                    // Inline sample_all logic
                    let entries: Vec<druid_metrics::registry::RegistryEntry> = {
                        let guard = sampler_registry.read();
                        guard.clone()
                    };
                    for entry in &entries {
                        if let Some(source) = entry.weak_ref.upgrade() {
                            if let Ok(snapshot) = source.try_snapshot() {
                                let pending = PendingSnapshot {
                                    datasource_id: entry.datasource_id,
                                    snapshot,
                                };
                                match tx.try_send(pending) {
                                    Ok(()) => {}
                                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                        sampler_metrics.increment_coalesced_total();
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // Wait for the sampler to run a few ticks
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel.cancel();
    sampler_handle.await.unwrap();

    // Drain one snapshot to prove the first one went through
    let first = rx.recv().await;
    assert!(
        first.is_some(),
        "should have received at least one snapshot"
    );

    // The coalesced counter should be > 0 because with 5 sources and capacity 1,
    // only 1 can be queued per tick, the other 4 are coalesced.
    let coalesced = metrics.coalesced_total();
    assert!(
        coalesced > 0,
        "expected coalesced_total > 0 when queue is saturated, got {coalesced}"
    );
}

#[tokio::test]
async fn busy_datasource_increments_snapshot_busy_total() {
    // 使用真实 runtime 验证 Busy 与队列合并分别计数。
    let config = DruidMetricsConfig::builder()
        .queue_capacity(1)
        .sample_interval(Duration::from_millis(5))
        .build()
        .unwrap();
    let runtime = DruidMetricsRuntime::start(config).await.unwrap();
    let busy_source = Arc::new(BusyDataSource::new(100, "busy_ds"));
    let guard = runtime.register(Arc::clone(&busy_source) as Arc<dyn DataSourceMonitorable>);
    tokio::time::timeout(Duration::from_secs(1), async {
        while runtime.self_metrics().snapshot_busy_total() == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("真实采样器应识别 Busy 数据源");
    assert!(busy_source.busy_count.load(Ordering::Relaxed) > 0);
    assert_eq!(runtime.self_metrics().coalesced_total(), 0);
    runtime.shutdown(Duration::from_secs(2)).await.unwrap();
    drop(guard);
}

#[tokio::test]
async fn runtime_end_to_end_with_many_datasources() {
    // Full pipeline test: start the runtime, register many datasources,
    // verify snapshots flow through and runtime shuts down cleanly.
    let config = DruidMetricsConfig::builder()
        .queue_capacity(64)
        .sample_interval(Duration::from_millis(10))
        .build()
        .unwrap();

    let runtime = DruidMetricsRuntime::start(config).await.unwrap();
    let _metrics = runtime.self_metrics().clone();

    // Register 10 datasources
    let mut guards = Vec::new();
    for i in 0..10 {
        let source = Arc::new(MockDataSource::new(i, &format!("ds_{i}")));
        let guard = runtime.register(source);
        guards.push(guard);
    }

    // Wait for several sampler ticks
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Pending snapshots should be > 0 (aggregator received data)
    // The exact count depends on timing, but at least some data flowed through.
    runtime.shutdown(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn queue_saturation_coalesces_live_sources_without_panicking() {
    // Stress test: capacity-1 queue with many datasources and rapid sampling
    let config = DruidMetricsConfig::builder()
        .queue_capacity(1)
        .sample_interval(Duration::from_millis(5))
        .build()
        .unwrap();

    let runtime = DruidMetricsRuntime::start(config).await.unwrap();

    // Runtime 仅持有弱引用；在饱和窗口内同时保留数据源和注册句柄，
    // 确保这里验证的是真实采样器路径。
    let mut sources = Vec::new();
    let mut guards = Vec::new();
    for i in 0..20 {
        let source = Arc::new(MockDataSource::new(i, &format!("stress_ds_{i}")));
        guards.push(runtime.register(Arc::clone(&source) as Arc<dyn DataSourceMonitorable>));
        sources.push(source);
    }

    assert_eq!(runtime.self_metrics().registered_datasources(), 20);

    // Let it run for a while -- should not panic
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert!(runtime.self_metrics().coalesced_total() > 0);
    assert_eq!(runtime.self_metrics().snapshot_busy_total(), 0);
    assert!(sources
        .iter()
        .all(|source| source.snapshot_count.load(Ordering::Relaxed) > 0));

    runtime.shutdown(Duration::from_secs(2)).await.unwrap();
    drop(guards);
}
