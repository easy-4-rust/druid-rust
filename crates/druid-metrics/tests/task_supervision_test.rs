//! 验证 MetricsSupervisor 对全部后台任务的所有权和失败传播。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use druid::stats::{
    DataSourceIdentity, DataSourceMonitorable, DruidTelemetrySnapshot, SnapshotUnavailable,
};
use druid_metrics::{DruidMetricsConfig, DruidMetricsRuntime, MetricsError};
use serde_json::Value;

struct SupervisionSource {
    identity: DataSourceIdentity,
    entered: Arc<AtomicBool>,
    panic_on_snapshot: bool,
}

impl DataSourceMonitorable for SupervisionSource {
    fn name(&self) -> &str {
        &self.identity.name
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        self.identity.clone()
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        self.entered.store(true, Ordering::Release);
        if self.panic_on_snapshot {
            panic!("injected sampler panic");
        }
        Err(SnapshotUnavailable::Busy)
    }
}

fn source(id: u64, panic_on_snapshot: bool) -> (Arc<SupervisionSource>, Arc<AtomicBool>) {
    let entered = Arc::new(AtomicBool::new(false));
    (
        Arc::new(SupervisionSource {
            identity: DataSourceIdentity {
                id,
                name: format!("supervision-{id}"),
                driver_name: Some("test".to_owned()),
            },
            entered: Arc::clone(&entered),
            panic_on_snapshot,
        }),
        entered,
    )
}

#[tokio::test]
async fn shutdown_joins_every_task_while_registration_guard_remains_live() {
    let runtime = DruidMetricsRuntime::start(
        DruidMetricsConfig::builder()
            .sample_interval(Duration::from_secs(60))
            .build()
            .unwrap(),
    )
    .await
    .unwrap();
    let metrics = runtime.self_metrics().clone();
    let (source, _) = source(91, false);
    let guard = runtime.register(source.clone());
    assert_eq!(metrics.active_tasks(), 4);

    runtime.shutdown(Duration::from_secs(1)).await.unwrap();
    assert_eq!(metrics.active_tasks(), 0);
    assert!(!metrics.unregister_listener_active());
    assert_eq!(metrics.registered_datasources(), 0);
    drop(guard);
}

#[tokio::test]
async fn sampler_panic_returns_join_error_and_supervisor_reaps_all_tasks() {
    let runtime = DruidMetricsRuntime::start(
        DruidMetricsConfig::builder()
            .sample_interval(Duration::from_millis(10))
            .build()
            .unwrap(),
    )
    .await
    .unwrap();
    let metrics = runtime.self_metrics().clone();
    let (source, entered) = source(92, true);
    let _guard = runtime.register(source.clone());
    tokio::time::timeout(Duration::from_secs(1), async {
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("sampler reaches injected panic");

    assert!(matches!(
        runtime.shutdown(Duration::from_secs(1)).await,
        Err(MetricsError::Join(_))
    ));
    assert_eq!(metrics.active_tasks(), 0);
    assert!(!metrics.unregister_listener_active());
}
