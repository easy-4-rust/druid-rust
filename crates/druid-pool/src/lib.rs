//! druid-pool — HikariCP-style async connection pool.

pub mod config;
pub mod pool_inner;
pub mod druid_pool;
pub mod pooled_connection;

pub use config::{DruidPoolBuilder, PoolInnerConfig};
pub use pool_inner::PoolInner;
pub use druid_pool::DruidPool;
pub use pooled_connection::DruidPoolConnection;

use druid_core::{DruidError, PoolState};

// Implement druid_core::Pool trait for DruidPool
#[async_trait::async_trait]
impl druid_core::Pool for DruidPool {
    async fn get(&self) -> Result<druid_core::PooledConnection, DruidError> {
        self.get().await.map(|c| c.into_core())
    }
    async fn get_timeout(&self, timeout: std::time::Duration) -> Result<druid_core::PooledConnection, DruidError> {
        self.get_timeout(timeout).await.map(|c| c.into_core())
    }
    fn state(&self) -> PoolState { self.state() }
    fn driver_name(&self) -> &str { self.driver_name() }
    fn name(&self) -> &str { self.name() }
}
