use super::DataSourceMonitorable;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

/// Druid 数据源统计注册表。
///
/// 对应 Java：`com.alibaba.druid.stat.DruidDataSourceStatManager`。Rust 不依赖
/// JMX/弱引用反射；数据源以显式 `Arc<dyn DataSourceMonitorable>` 注册。
pub struct DruidDataSourceStatManager {
    next_id: AtomicU64,
    instances: DashMap<u64, Arc<dyn DataSourceMonitorable>>,
}

impl DruidDataSourceStatManager {
    /// 返回进程级注册表。
    #[must_use]
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<DruidDataSourceStatManager> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            next_id: AtomicU64::new(1),
            instances: DashMap::new(),
        })
    }

    /// 注册数据源并返回稳定管理 ID。
    pub fn register(&self, data_source: Arc<dyn DataSourceMonitorable>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.instances.insert(id, data_source);
        id
    }

    /// 注销指定管理 ID。
    pub fn unregister(&self, id: u64) -> Option<Arc<dyn DataSourceMonitorable>> {
        self.instances
            .remove(&id)
            .map(|(_, data_source)| data_source)
    }

    /// 返回指定数据源。
    #[must_use]
    pub fn get(&self, id: u64) -> Option<Arc<dyn DataSourceMonitorable>> {
        self.instances
            .get(&id)
            .map(|entry| Arc::clone(entry.value()))
    }

    /// 返回 `(id, datasource)` 快照。
    #[must_use]
    pub fn instances(&self) -> Vec<(u64, Arc<dyn DataSourceMonitorable>)> {
        self.instances
            .iter()
            .map(|entry| (*entry.key(), Arc::clone(entry.value())))
            .collect()
    }

    /// 重置全部已注册数据源。
    pub fn reset(&self) {
        for entry in &self.instances {
            entry.value().reset_stat();
        }
    }
}
