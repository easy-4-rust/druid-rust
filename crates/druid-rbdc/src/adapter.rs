//! 对应 Java 类：DruidDataSource（rbdc adapter）
//! 来源文件：core/src/main/java/com/alibaba/druid/pool/DruidDataSource.java
//!
//! rbdc 连接适配器，实现 ConnectionFactory 将 rbdc::Connection 桥接到 druid-core Connection trait。

use druid_core::{Connection, ConnectionFactory, DruidError};

/// rbdc 连接适配器。
///
/// 对应 DruidJava `DruidDataSource` 中的连接创建逻辑，
/// 将 `rbdc::db::Connection` 桥接到 `druid_core::Connection`。
pub struct RbdcAdapter {
    url: String,
}

impl RbdcAdapter {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for RbdcAdapter {
    async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
        // rbdc connection creation will be implemented when rbdc dependency is added.
        // See ADR-001 in druid-rust-Architecture.zh_CN.md
        Err(DruidError::DriverError("rbdc adapter: connection creation not yet implemented".into()))
    }

    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
        conn.ping().await
    }
}
