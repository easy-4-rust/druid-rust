//! 对应 Java 类：com.alibaba.druid.pool.ValidConnectionChecker + JDBC4ValidConnectionChecker

use crate::connection::Connection;

/// 连接验证 trait。
#[async_trait::async_trait]
pub trait ValidConnectionChecker: Send + Sync {
    async fn is_valid(&self, conn: &mut Box<dyn Connection>) -> bool;
}

/// 基于 ping 的验证器（默认）。
pub struct PingConnectionChecker;

#[async_trait::async_trait]
impl ValidConnectionChecker for PingConnectionChecker {
    async fn is_valid(&self, conn: &mut Box<dyn Connection>) -> bool {
        conn.ping().await.is_ok()
    }
}
