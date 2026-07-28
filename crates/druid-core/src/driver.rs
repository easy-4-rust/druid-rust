//! 对应 Java 类：java.sql.Driver

use crate::error::DruidError;
use crate::physical_connection::PhysicalConnection;

/// 驱动 trait，替代 JDBC java.sql.Driver。
#[async_trait::async_trait]
pub trait Driver: Send + Sync {
    fn name(&self) -> &str;
    async fn connect(&self, url: &str) -> Result<Box<dyn PhysicalConnection>, DruidError>;
    async fn connect_with_auth(
        &self,
        url: &str,
        username: &str,
        password: &str,
    ) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let _ = (username, password);
        self.connect(url).await
    }
}
