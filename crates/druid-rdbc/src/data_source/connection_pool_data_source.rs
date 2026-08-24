use crate::data_source::{CommonDataSource, PooledConnection};
use crate::error::RdbcError;

/// `javax.sql.ConnectionPoolDataSource` 的 provider 标准。
#[async_trait::async_trait]
pub trait ConnectionPoolDataSource: CommonDataSource {
    /// 获取一个 owned [`PooledConnection`]。
    async fn pooled_connection(&self) -> Result<Box<dyn PooledConnection>, RdbcError>;

    /// 使用 caller credentials 获取 owned pooled connection。
    async fn pooled_connection_with_credentials(
        &self,
        _user: &str,
        _password: &str,
    ) -> Result<Box<dyn PooledConnection>, RdbcError> {
        self.pooled_connection().await
    }
}
