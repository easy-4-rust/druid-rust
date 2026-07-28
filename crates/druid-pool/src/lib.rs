//! druid-pool — HikariCP-style async connection pool.

pub mod config;
pub mod druid_pool;
pub mod pool_inner;

pub use config::{DruidPoolBuilder, PoolInnerConfig};
pub use druid_core::DruidPooledConnection;
pub use druid_pool::DruidPool;
pub use pool_inner::PoolInner;
