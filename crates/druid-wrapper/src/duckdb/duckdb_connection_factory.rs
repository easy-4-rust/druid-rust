//! DuckDB 原生物理连接工厂。

use super::DuckDbConnectionAdapter;
use druid::core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};

/// DuckDB 原生未池化物理连接工厂。
///
/// 对应 Java: `DruidDataSource#createPhysicalConnection`。每次 `create` 都通过
/// duckdb-rs 打开一个独立物理连接，DruidPool 是唯一池化边界。
#[derive(Debug, Clone)]
pub struct DuckDbConnectionFactory {
    url: String,
}

impl DuckDbConnectionFactory {
    /// 创建 DuckDB 连接工厂。
    ///
    /// 参数 `url` 必须使用 `duckdb:` scheme；内存库使用
    /// `duckdb::memory:`，文件库使用 `duckdb:/absolute/path` 或显式相对路径。
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// 返回配置的 DuckDB URL。
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for DuckDbConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Ok(Box::new(DuckDbConnectionAdapter::connect(&self.url).await?))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}
