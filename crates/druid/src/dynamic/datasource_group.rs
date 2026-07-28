//! 对应 Java 类：com.alibaba.druid.pool.ha.HighAvailableDataSource（节点组）
//!
//! 数据源组：主库 + 从库列表 + 负载均衡器。

use super::load_balancer::LoadBalancer;
use crate::core::Pool;
use std::sync::Arc;

/// 数据源组。
///
/// 对应 Druid Java 的 HA 节点组，包含主库和从库。
#[derive(Clone)]
pub struct DataSourceGroup {
    pub name: String,
    pub master: Arc<dyn Pool>,
    pub slaves: Vec<Arc<dyn Pool>>,
    pub load_balancer: Arc<dyn LoadBalancer>,
}

impl DataSourceGroup {
    pub fn new(
        name: impl Into<String>,
        master: Arc<dyn Pool>,
        slaves: Vec<Arc<dyn Pool>>,
        load_balancer: Arc<dyn LoadBalancer>,
    ) -> Self {
        Self {
            name: name.into(),
            master,
            slaves,
            load_balancer,
        }
    }
}
