//! 未池化物理连接工厂。

use super::{DruidError, PhysicalConnection, PhysicalConnectionInfo};
use std::collections::HashMap;
use std::time::Instant;

/// 未池化物理连接工厂。
///
/// 对应 Java: `DruidDataSource#createPhysicalConnection`。该工厂只服务
/// Druid native pool，每次创建一个 raw `PhysicalConnection`，不得从
/// bb8、deadpool 或其他外部连接池获取租约。
#[async_trait::async_trait]
pub trait PhysicalConnectionFactory: Send + Sync {
    /// 返回驱动连接 URL；未知时保持 `None`，不得伪造空 URL。
    fn connection_url(&self) -> Option<&str> {
        None
    }

    /// 返回连接用户名；Adapter 未持有凭据时保持 `None`。
    fn user_name(&self) -> Option<&str> {
        None
    }

    /// 返回驱动/Adapter 名称；未知时保持 `None`。
    fn driver_name(&self) -> Option<&str> {
        None
    }

    /// 创建一个未池化的物理连接。
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError>;

    /// 创建一个带阶段时序的未池化物理连接。
    ///
    /// 对应 Java：`DruidAbstractDataSource#createPhysicalConnection()` 返回
    /// `PhysicalConnectionInfo`。默认实现包裹现有 `create()`，因此外部驱动
    /// Adapter 无需为了新增元数据立即改写；需要提供会话变量的 Adapter 可以
    /// 覆盖本方法。
    async fn create_info(&self) -> Result<PhysicalConnectionInfo, DruidError> {
        let connect_started_at = Instant::now();
        let connection = self.create().await?;
        Ok(PhysicalConnectionInfo::connected(
            connection,
            connect_started_at,
        ))
    }

    /// 使用单次连接属性创建物理连接。
    ///
    /// 对应 Java `Driver#connect(url, Properties)`。默认实现适配已经在构造时
    /// 固化凭据和选项的 Rust driver factory；需要观察 Filter 改写属性的
    /// Adapter 应覆盖本方法，并把最终属性传给底层驱动。
    async fn create_info_with_properties(
        &self,
        _properties: &HashMap<String, String>,
    ) -> Result<PhysicalConnectionInfo, DruidError> {
        self.create_info().await
    }

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
