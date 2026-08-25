//! RBDC 物理连接工厂。

use super::rbdc_connection_adapter::RbdcConnectionAdapter;
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use druid_rdbc::spi::{PhysicalConnection, PhysicalConnectionFactory};
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

    async fn create_adapter_core(&self) -> Result<RbdcConnectionAdapter, RdbcError> {
        let connection = self.driver.connect(&self.url).await.map_err(|error| {
            RdbcError::SqlException(Box::new(
                SqlException::driver(0, error.to_string()).with_class_name("rbdc::Error"),
            ))
        })?;
        Ok(RbdcConnectionAdapter::new(connection, self.driver.name()))
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for RbdcConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core().await?))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for RbdcConnectionFactory {
    async fn create(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core().await?))
    }

    fn name(&self) -> &str {
        self.driver.name()
    }
}
