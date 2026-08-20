//! druid-rust 多数据源：ArcSwap 热切换、读写分离、负载均衡。
//!
//! 对应 Druid Java 的 `com.alibaba.druid.pool.ha` 包。

pub mod data_source_creator;
pub mod datasource_group;
pub mod dynamic_datasource;
pub mod high_available_data_source;
pub mod load_balancer;
pub mod node;
pub mod properties_utils;
pub mod selector;
pub mod sql_hint;

pub use data_source_creator::DataSourceCreator;
pub use datasource_group::DataSourceGroup;
pub use dynamic_datasource::DynamicDataSource;
pub use high_available_data_source::HighAvailableDataSource;
pub use load_balancer::{LoadBalancer, RandomBalancer, RoundRobinBalancer};
pub use node::{
    FileNodeListener, NodeEvent, NodeEventTypeEnum, NodeListener, PoolUpdater, ZookeeperNodeInfo,
    ZookeeperNodeListener, ZookeeperNodeRegister,
};
pub use properties_utils::PropertiesUtils;
pub use selector::{
    DataSourceSelector, DataSourceSelectorEnum, DataSourceSelectorFactory, NamedDataSourceSelector,
    RandomDataSourceRecoverTask, RandomDataSourceSelector, RandomDataSourceValidateFilter,
    RandomDataSourceValidateTask, StickyDataSourceHolder, StickyRandomDataSourceSelector,
};
pub use sql_hint::SqlHint;

pub(crate) fn epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
