//! Java Druid `druid-wrapper` 模块的 Rust 语义迁移。
//!
//! 对应 Java 模块：`/druid-wrapper`。Java 模块包装 c3p0、DBBC、Proxool；
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
/// 显式 JDBC 驱动安装、内容校验和运行时诊断。
#[cfg(feature = "managed-driver-install")]
pub mod driver_admin;
/// DuckDB 原生未池化物理连接 Adapter。
#[cfg(feature = "duckdb-native")]
pub mod duckdb;
/// RQLite、Cloudflare D1 等产品的未池化 HTTP SQL 物理连接 Adapter。
#[cfg(feature = "http-sql")]
pub mod http_sql;
/// Druid 自有 JDBC Agent 协议、进程运行时与物理连接 Adapter。
#[cfg(feature = "jdbc-agent")]
pub mod jdbc_agent;
/// Turso/libSQL 远程原生未池化物理连接 Adapter。
#[cfg(feature = "libsql-native")]
pub mod libsql;
/// Proxool 兼容对象。
pub mod proxool;
/// RBDC 直连驱动 Adapter。
pub mod rbdc;
/// 基于 DruidPool 的 RDBC 标准 DataSource 与动态数据源实现。
#[cfg(feature = "driver-catalog")]
pub mod rdbc;
/// SQLx 直连驱动 Adapter，以及 bb8、deadpool 池适配器。
pub mod sqlx;
/// 内置 Toasty 标准数据源实现。
pub mod toasty;

/// Ensure driver extension `inventory::submit!` registrations are linked.
///
/// Test binaries and downstream consumers should call this once at startup
/// (or reference it) to prevent the linker from stripping the statics.
#[cfg(feature = "driver-catalog")]
pub fn init_driver_extensions() {
    // Touch the module so the linker retains its statics.
    driver::extensions::init();
}

pub use managed_wrapper_pool::ManagedWrapperPool;
pub use proxool_config_key::ProxoolConfigKey;
pub use wrapper_data_source_factory::WrapperDataSourceFactory;
pub use wrapper_pool_state::WrapperPoolState;
