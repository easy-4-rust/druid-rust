//! 对应 Java 类：javax.sql.DataSource + com.alibaba.druid.pool.DruidDataSource

use crate::druid_pooled_connection::DruidPooledConnection;
use crate::error::DruidError;
use crate::pool_state::PoolState;
use std::time::Duration;

/// 连接池 trait，替代 DataSource。
#[async_trait::async_trait]
pub trait Pool: Send + Sync {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError>;
    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError>;
    fn state(&self) -> PoolState;
    fn driver_name(&self) -> &str;
    fn name(&self) -> &str;
}
