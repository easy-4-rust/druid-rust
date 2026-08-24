use crate::connection::Connection;
use crate::error::RdbcError;

/// `javax.sql.PooledConnection` 的 logical-connection source 标准。
#[async_trait::async_trait]
pub trait PooledConnection: Send {
    /// 创建一个 owned logical connection handle。
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError>;

    /// 关闭该 pooled connection owner。
    async fn close(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("pooled_connection_close"))
    }
}
