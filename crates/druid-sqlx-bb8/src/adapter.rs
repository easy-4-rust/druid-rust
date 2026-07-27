//! 对应 Java 类：DruidDataSource（sqlx-bb8 adapter）
//! 来源文件：core/src/main/java/com/alibaba/druid/pool/DruidDataSource.java
//!
//! sqlx + bb8 连接适配器。

use druid_core::{Connection, ConnectionFactory, DruidError};

/// sqlx-bb8 连接适配器。
pub struct SqlxBb8Adapter {
    url: String,
}

impl SqlxBb8Adapter {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for SqlxBb8Adapter {
    async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
        Err(DruidError::DriverError("sqlx-bb8 adapter: connection creation not yet implemented".into()))
    }

    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
        conn.ping().await
    }
}
