//! 未池化物理连接工厂。

use super::{DruidError, PhysicalConnection};

/// 未池化物理连接工厂。
///
/// 对应 Java: `DruidDataSource#createPhysicalConnection`。该工厂只服务
/// Druid native pool，每次创建一个 raw `PhysicalConnection`，不得从
/// bb8、deadpool 或其他外部连接池获取租约。
#[async_trait::async_trait]
pub trait PhysicalConnectionFactory: Send + Sync {
    /// 创建一个未池化的物理连接。
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError>;

    /// 验证物理连接是否可继续使用。
    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError>;

    /// 关闭物理连接。
    async fn close(&self, connection: &mut Box<dyn PhysicalConnection>) -> Result<(), DruidError> {
        connection.close().await
    }
}
