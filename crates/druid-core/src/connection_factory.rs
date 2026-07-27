//! 对应 Java 类：DruidDataSource.init() 中的连接创建逻辑

use crate::connection::Connection;
use crate::error::DruidError;

/// 连接工厂 trait。
#[async_trait::async_trait]
pub trait ConnectionFactory: Send + Sync {
    async fn create(&self) -> Result<Box<dyn Connection>, DruidError>;
    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError>;
    async fn close(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
        conn.close().await
    }
}
