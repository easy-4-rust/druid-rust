extern crate druid_core as druid;
use druid::stats::{DataSourceMonitorable, DruidDataSourceStatManager};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct MockMonitorable {
    name: &'static str,
    reset_calls: Arc<AtomicUsize>,
    log_calls: Arc<AtomicUsize>,
}

impl MockMonitorable {
    fn arc(name: &'static str) -> (Arc<Self>, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let reset_calls = Arc::new(AtomicUsize::new(0));
        let log_calls = Arc::new(AtomicUsize::new(0));
        let m = Arc::new(Self {
            name,
            reset_calls: Arc::clone(&reset_calls),
            log_calls: Arc::clone(&log_calls),
        });
        (m, reset_calls, log_calls)
    }
}

impl DataSourceMonitorable for MockMonitorable {
    fn name(&self) -> &str {
        self.name
    }
    fn reset_stat(&self) {
        self.reset_calls.fetch_add(1, Ordering::Relaxed);
    }
    fn log_stats(&self) -> Result<(), druid::core::DruidError> {
        self.log_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn data_source_stat_data(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    fn identity(&self) -> druid::stats::DataSourceIdentity {
        druid::stats::DataSourceIdentity {
            id: 0,
            name: self.name.to_string(),
            driver_name: None,
        }
    }
    fn try_snapshot(
        &self,
    ) -> Result<druid::stats::DruidTelemetrySnapshot, druid::stats::SnapshotUnavailable> {
        Err(druid::stats::SnapshotUnavailable::Busy)
    }
}

#[test]
fn stat_manager_global_singleton() {
    let m1 = DruidDataSourceStatManager::global();
    let m2 = DruidDataSourceStatManager::global();
    assert!(std::ptr::eq(m1, m2));
}

#[test]
fn stat_manager_register_and_get() {
    let manager = DruidDataSourceStatManager::global();
    let (ds, _, _) = MockMonitorable::arc("test");
    let id = manager.register(ds.clone());
    assert!(manager.get(id).is_some());
    assert_eq!(manager.get(id).unwrap().name(), "test");
    let _ = manager.unregister(id);
}

#[test]
fn stat_manager_unregister() {
    let manager = DruidDataSourceStatManager::global();
    let (ds, _, _) = MockMonitorable::arc("test");
    let id = manager.register(ds.clone());
    let removed = manager.unregister(id);
    assert!(removed.is_some());
    assert!(manager.get(id).is_none());
    assert!(manager.unregister(999_999).is_none());
}

#[test]
fn stat_manager_instances() {
    let manager = DruidDataSourceStatManager::global();
    let (ds, _, _) = MockMonitorable::arc("test");
    let id = manager.register(ds.clone());
    let all = manager.instances();
    assert!(all.iter().any(|(i, _)| *i == id));
    let _ = manager.unregister(id);
}

#[test]
fn stat_manager_reset() {
    let manager = DruidDataSourceStatManager::global();
    let (ds, reset_calls, _) = MockMonitorable::arc("test");
    let id = manager.register(ds.clone());
    let count_before = manager.reset_count();
    manager.reset();
    assert!(reset_calls.load(Ordering::Relaxed) >= 1);
    assert!(manager.reset_count() > count_before);
    let _ = manager.unregister(id);
}

#[test]
fn stat_manager_log_and_reset() {
    let manager = DruidDataSourceStatManager::global();
    let (ds, _reset_calls, log_calls) = MockMonitorable::arc("test");
    let id = manager.register(ds.clone());
    let count_before = manager.reset_count();
    manager.log_and_reset_data_source();
    assert!(log_calls.load(Ordering::Relaxed) >= 1);
    assert!(manager.reset_count() > count_before);
    let _ = manager.unregister(id);
}
