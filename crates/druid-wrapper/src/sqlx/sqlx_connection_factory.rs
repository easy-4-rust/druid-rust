//! SQLx 物理连接工厂。

use super::sqlx_connection_adapter::SqlxConnectionAdapter;
use druid_core::core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};

/// SQLx 物理连接工厂。
///
/// 对应 Java: `DruidDataSource#createPhysicalConnection`。
/// 每次 `create` 都打开一个未池化的 SQLx 连接，由 DruidPool 独占池化职责。
#[derive(Clone)]
pub struct SqlxConnectionFactory {
    url: String,
}

impl SqlxConnectionFactory {
    /// 创建 SQLx 物理连接工厂。
    ///
    /// 参数 `url` 为 SQLx 支持的数据库连接 URL。
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// 返回数据库连接 URL。
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl std::fmt::Debug for SqlxConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_url = url::Url::parse(&self.url).map_or_else(
            |_| "<redacted>".to_owned(),
            |mut url| {
                let _ = url.set_username("");
                let _ = url.set_password(None);
                url.set_query(None);
                url.to_string()
            },
        );
        formatter
            .debug_struct("SqlxConnectionFactory")
            .field("url", &display_url)
            .finish()
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for SqlxConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let connection = SqlxConnectionAdapter::connect(&self.url).await?;
        Ok(Box::new(connection))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}
