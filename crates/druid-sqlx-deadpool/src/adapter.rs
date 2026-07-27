//! 对应 Java 类：DruidDataSource（sqlx-deadpool adapter）
//! 来源文件：core/src/main/java/com/alibaba/druid/pool/DruidDataSource.java
//!
//! sqlx + deadpool 连接适配器。

use druid_core::{Connection, ConnectionFactory, DruidError};

/// sqlx-deadpool 连接适配器。
pub struct SqlxDeadpoolAdapter {
    url: String,
}

impl SqlxDeadpoolAdapter {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for SqlxDeadpoolAdapter {
    async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
        Err(DruidError::DriverError("sqlx-deadpool adapter: connection creation not yet implemented".into()))
    }

    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
        conn.ping().await
    }
}
