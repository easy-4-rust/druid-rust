//! 对应 Java 类：com.alibaba.druid.pool.ha.HighAvailableDataSource
//!
//! 动态数据源，支持 ArcSwap 热切换。

use super::datasource_group::DataSourceGroup;
use super::sql_hint::SqlHint;
use crate::core::{DruidError, DruidPooledConnection, Pool};
use arc_swap::ArcSwap;
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

    /// 按 SQL 意图从当前数据源组取得池化连接。
    pub async fn get_connection_for(
        &self,
        hint: SqlHint,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.route(hint).get().await
    }

    /// 从当前默认写节点取得连接。对应 Java: `DataSource#getConnection()`。
    pub async fn get_connection(&self) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection_for(SqlHint::Auto).await
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
