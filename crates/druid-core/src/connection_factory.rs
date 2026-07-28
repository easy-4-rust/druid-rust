//! 对应 Java 类：DruidDataSource.init() 中的连接创建逻辑

use crate::error::DruidError;
use crate::physical_connection::PhysicalConnection;

/// 物理连接工厂。
///
/// 对应 Java: `DruidDataSource#createPhysicalConnection`。
#[async_trait::async_trait]
pub trait ConnectionFactory: Send + Sync {
    /// 创建一个未池化的物理连接。
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError>;

    /// 验证物理连接是否可继续使用。
    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError>;

    /// 关闭物理连接。
    async fn close(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.close().await
    }
}
