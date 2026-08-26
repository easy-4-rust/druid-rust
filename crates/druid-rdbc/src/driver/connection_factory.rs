use crate::connection::Connection;
use crate::error::RdbcError;
use std::collections::HashMap;

/// Rust connection acquisition extension。
///
/// 与 JDBC [`crate::driver::Driver`] 的 URL negotiation 分离；每次返回 owned connection。
#[async_trait::async_trait]
pub trait ConnectionFactory: Send + Sync + 'static {
    /// 返回连接 URL；未知时保持 `None`。
    fn connection_url(&self) -> Option<&str> {
        None
    }

    /// 返回连接用户名；factory 未持有凭据时保持 `None`。
    fn user_name(&self) -> Option<&str> {
        None
    }

    /// 创建一个 owned standard connection。
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError>;

    /// 使用一次性连接属性创建 owned standard connection。
    async fn create_with_properties(
        &self,
        _properties: &HashMap<String, String>,
    ) -> Result<Box<dyn Connection>, RdbcError> {
        self.create().await
    }

    /// 验证连接是否可以继续交给调用方。
    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        if connection.is_valid(1).await? {
            Ok(())
        } else {
            Err(RdbcError::ConnectionDiscarded)
        }
    }

    /// 关闭 factory 创建的连接。
    async fn close(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.close().await
    }

    /// 返回 factory name。
    fn name(&self) -> &str {
        ""
    }
}
