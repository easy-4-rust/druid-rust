//! 对应 Java 接口：com.alibaba.druid.pool.ValidConnectionChecker

use std::collections::HashMap;
use std::time::Duration;

use super::physical_connection::PhysicalConnection;
use super::DruidError;

/// 物理连接验证 SPI。
///
/// 对应 Java: `com.alibaba.druid.pool.ValidConnectionChecker`。
#[async_trait::async_trait]
pub trait ValidConnectionChecker: Send + Sync {
    /// 使用验证 SQL 或驱动原生校验检查物理连接。
    ///
    /// `query` 对应 Java `validationQuery`，`validation_query_timeout` 对应秒级
    /// `validationQueryTimeout`。驱动异常通过 `Result` 原样传播。
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
        query: Option<&str>,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError>;

    /// 从数据源属性配置校验器。
    fn config_from_properties(&mut self, _properties: &HashMap<String, String>) {}

    /// 早期 Rust API 的兼容入口；使用驱动原生校验且不设置超时。
    async fn is_valid(&self, connection: &mut Box<dyn PhysicalConnection>) -> bool {
        self.is_valid_connection(connection, None, Duration::ZERO)
            .await
            .unwrap_or(false)
    }
}
