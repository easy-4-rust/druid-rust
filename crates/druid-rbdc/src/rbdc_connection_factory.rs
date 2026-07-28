//! RBDC 物理连接工厂。

use crate::rbdc_connection_adapter::RbdcConnectionAdapter;
use druid_core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};
use std::sync::Arc;

/// RBDC 物理连接工厂。
///
/// 对应 Java: `DruidDataSource#createPhysicalConnection`。
/// Factory 持有可共享 Driver，但每次只创建未池化的 RBDC Connection。
pub struct RbdcConnectionFactory {
    driver: Arc<dyn rbdc::db::Driver>,
    url: String,
}

impl RbdcConnectionFactory {
    /// 创建 RBDC 物理连接工厂。
    ///
    /// 参数 `driver` 为具体 RBDC Driver，`url` 为数据库 URL。
    pub fn new(driver: Arc<dyn rbdc::db::Driver>, url: impl Into<String>) -> Self {
        Self {
            driver,
            url: url.into(),
        }
    }

    /// 返回连接 URL。
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 返回 RBDC 驱动名称。
    pub fn driver_name(&self) -> &str {
        self.driver.name()
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for RbdcConnectionFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let connection = self
            .driver
            .connect(&self.url)
            .await
            .map_err(|error| DruidError::DriverError(error.to_string()))?;
        Ok(Box::new(RbdcConnectionAdapter::new(
            connection,
            self.driver.name(),
        )))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}
