//! Java Druid `druid-wrapper` 模块的 Rust 语义迁移。
//!
//! 对应 Java 模块：`/druid-wrapper`。Java 模块包装 c3p0、DBCP、Proxool；
//! Rust 迁移按等价职责在本 crate 内部聚合 RBDC、SQLx、bb8 与 deadpool Adapter。

mod managed_wrapper_pool;
mod prepared_parameter_materializer;
mod prepared_parameter_state;
mod proxool_config_key;
mod wrapper_data_source_factory;
mod wrapper_pool_state;

/// c3p0 兼容对象。
pub mod c3p0;
/// Apache DBCP 1 兼容对象。
pub mod dbcp;
/// Apache DBCP 2 兼容对象。
pub mod dbcp2;
/// Proxool 兼容对象。
pub mod proxool;
/// RBDC 直连驱动 Adapter。
pub mod rbdc;
/// SQLx 直连驱动 Adapter，以及 bb8、deadpool 池适配器。
pub mod sqlx;

pub use managed_wrapper_pool::ManagedWrapperPool;
pub use proxool_config_key::ProxoolConfigKey;
pub use wrapper_data_source_factory::WrapperDataSourceFactory;
pub use wrapper_pool_state::WrapperPoolState;
