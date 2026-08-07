use super::{HttpSqlConnectionAdapter, HttpSqlProvider};
use druid::core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};
use std::collections::HashMap;

/// 为每个 Druid holder 创建独立 HTTP SQL 逻辑会话的未池化工厂。
#[derive(Clone)]
pub struct HttpSqlConnectionFactory {
    provider: HttpSqlProvider,
    endpoint: String,
    properties: HashMap<String, String>,
}

impl HttpSqlConnectionFactory {
    /// 创建 HTTP SQL 工厂。核心建池路径不会发起请求，首次 `create` 才验证服务。
    #[must_use]
    pub fn new(provider: HttpSqlProvider, endpoint: impl Into<String>) -> Self {
        Self {
            provider,
            endpoint: endpoint.into(),
            properties: HashMap::new(),
        }
    }

    /// 设置鉴权或产品连接属性；Debug 输出只展示属性名。
    #[must_use]
    pub fn property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }
}

impl std::fmt::Debug for HttpSqlConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpSqlConnectionFactory")
            .field("provider", &self.provider)
            .field("endpoint", &self.endpoint)
            .field("property_names", &self.properties.keys())
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for HttpSqlConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.endpoint)
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let mut connection = HttpSqlConnectionAdapter::new(
            self.provider,
            self.endpoint.clone(),
            self.properties.clone(),
            reqwest::Client::new(),
        )?;
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
