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
/// 以 Druid 物理连接 SPI 为核心的多数据库驱动目录与解析入口。
#[cfg(feature = "driver-catalog")]
pub mod driver;
/// DuckDB 原生未池化物理连接 Adapter。
#[cfg(feature = "duckdb-native")]
pub mod duckdb;
/// Druid 自有 JDBC Agent 协议、进程运行时与物理连接 Adapter。
#[cfg(feature = "jdbc-agent")]
pub mod jdbc_agent;
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
