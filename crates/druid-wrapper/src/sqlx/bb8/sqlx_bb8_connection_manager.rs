//! `SQLx` canonical 连接的 bb8 管理器。

use crate::sqlx::SqlxConnectionFactory;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;

/// `SQLx` canonical 连接的 bb8 管理器。
/// 管理器创建的是单个 `SqlxConnectionAdapter`，不创建 `DruidPool`。
#[derive(Debug, Clone)]
pub struct SqlxBb8ConnectionManager {
    factory: SqlxConnectionFactory,
}

impl SqlxBb8ConnectionManager {
    /// 创建 bb8 连接管理器。
    ///
    /// 参数 `url` 为 `SQLx` 数据库连接 URL。
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            factory: SqlxConnectionFactory::new(url),
        }
    }

    /// 返回数据库连接 URL。
    pub fn url(&self) -> &str {
        self.factory.url()
    }
}

#[async_trait::async_trait]
impl bb8::ManageConnection for SqlxBb8ConnectionManager {
    type Connection = Box<dyn Connection>;
    type Error = RdbcError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.factory.create().await
    }

    async fn is_valid(&self, connection: &mut Self::Connection) -> Result<(), Self::Error> {
        self.factory.validate(connection).await
    }

    fn has_broken(&self, connection: &mut Self::Connection) -> bool {
        connection.is_closed().unwrap_or(true)
    }
}
