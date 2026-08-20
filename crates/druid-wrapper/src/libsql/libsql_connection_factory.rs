use super::LibSqlConnectionAdapter;
use druid_core::core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};
use std::collections::HashMap;

/// Turso/libSQL 原生远程未池化物理连接工厂。
///
/// 每次 `create` 都构造独立的 libSQL `Database` 与 `Connection`，DruidPool
/// 是唯一池化边界，不复用 libSQL 客户端池。
#[derive(Clone)]
pub struct LibSqlConnectionFactory {
    url: String,
    properties: HashMap<String, String>,
}

impl std::fmt::Debug for LibSqlConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibSqlConnectionFactory")
            .field("url", &self.url)
            .field(
                "property_names",
                &self.properties.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl LibSqlConnectionFactory {
    /// 创建 Turso/libSQL 工厂。
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            properties: HashMap::new(),
        }
    }

    /// 添加连接属性；令牌使用 `token` 或 `auth_token`。
    #[must_use]
    pub fn property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for LibSqlConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let token = self
            .properties
            .get("token")
            .or_else(|| self.properties.get("auth_token"))
            .cloned()
            .unwrap_or_default();
        let mut connection = LibSqlConnectionAdapter::connect(&self.url, token).await?;
        connection.ping().await?;
        Ok(Box::new(connection))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}
