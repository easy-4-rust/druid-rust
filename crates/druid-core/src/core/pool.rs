//! 对应 Java 类：javax.sql.DataSource + com.alibaba.druid.pool.DruidDataSource

use super::druid_pooled_connection::DruidPooledConnection;
use super::error::DruidError;
use super::pool_state::PoolState;
use std::time::Duration;

/// 连接池 trait，替代 DataSource。
#[async_trait::async_trait]
pub trait Pool: Send + Sync {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError>;
    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError>;

    /// 为 HA 节点摘除执行资源收尾。
    ///
    /// 返回 `false` 表示仍有活动连接，本轮必须延迟摘除；默认外部 Pool 没有
    /// Druid 生命周期协议，按 Java 对非 `DruidDataSource` 的行为直接允许摘除。
    async fn close_for_removal_if_idle(&self) -> Result<bool, DruidError> {
        Ok(true)
    }

    /// 关闭 Pool 自身持有的资源。
    ///
    /// Java HA 只主动关闭 `DruidDataSource` 子节点，因此未知外部 Pool 默认无
    /// 操作；Druid 原生实现覆盖该方法。
    async fn close_pool(&self) {}

    fn state(&self) -> PoolState;
    fn driver_name(&self) -> &str;
    fn name(&self) -> &str;
}
