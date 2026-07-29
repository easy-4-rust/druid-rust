//! Druid 异步连接池实现。

mod active_connection_lease;
pub mod config;
pub mod druid_data_source;
pub mod druid_data_source_factory;
pub mod druid_pool;
pub mod managed_data_source;
pub mod pool_inner;
mod pool_validation_factory;

pub use crate::core::DruidPooledConnection;
pub use config::{DruidPoolBuilder, PoolInnerConfig};
pub use druid_data_source::DruidDataSource;
pub use druid_data_source_factory::DruidDataSourceFactory;
pub use druid_pool::DruidPool;
pub use managed_data_source::ManagedDataSource;
pub use pool_inner::PoolInner;
