//! 对应 Java 类：com.alibaba.druid.pool.ha.HighAvailableDataSource
//!
//! 动态数据源，支持 ArcSwap 热切换。

use crate::datasource_group::DataSourceGroup;
use crate::sql_hint::SqlHint;
use arc_swap::ArcSwap;
use druid_core::Pool;
use std::sync::Arc;

/// 动态数据源。
///
/// 对应 Druid Java 的 `HighAvailableDataSource`，使用 ArcSwap 实现 lock-free 热切换。
pub struct DynamicDataSource {
    current: ArcSwap<DataSourceGroup>,
}

impl DynamicDataSource {
    /// 创建动态数据源。
    pub fn new(initial: DataSourceGroup) -> Self {
        Self {
            current: ArcSwap::from(Arc::new(initial)),
        }
    }

    /// 按 SqlHint 路由到对应池。
    pub fn route(&self, hint: SqlHint) -> Arc<dyn Pool> {
        let group = self.current.load();
        match hint {
            SqlHint::Write => group.master.clone(),
            SqlHint::Read => group
                .load_balancer
                .pick(&group.slaves)
                .unwrap_or(&group.master)
                .clone(),
            SqlHint::Auto => group.master.clone(), // 默认走主库
        }
    }

    /// 热切换数据源（lock-free）。
    ///
    /// 对应 Druid Java 的 HighAvailableDataSource 节点切换。
    pub fn switch(&self, new_group: DataSourceGroup) {
        self.current.store(Arc::new(new_group));
    }

    /// 返回当前数据源组快照。
    pub fn current(&self) -> Arc<DataSourceGroup> {
        self.current.load_full()
    }

    /// 返回当前主库名称。
    pub fn current_name(&self) -> String {
        self.current.load().name.clone()
    }
}
