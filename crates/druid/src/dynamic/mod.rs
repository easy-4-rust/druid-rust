//! druid-rust 多数据源：ArcSwap 热切换、读写分离、负载均衡。
//!
//! 对应 Druid Java 的 `com.alibaba.druid.pool.ha` 包。

pub mod datasource_group;
pub mod dynamic_datasource;
pub mod load_balancer;
pub mod sql_hint;

pub use datasource_group::DataSourceGroup;
pub use dynamic_datasource::DynamicDataSource;
pub use load_balancer::{LoadBalancer, RandomBalancer, RoundRobinBalancer};
pub use sql_hint::SqlHint;
