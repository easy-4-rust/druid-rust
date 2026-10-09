use super::lifetime_test_connection::LifetimeTestConnection;
use crate::core::{Connection, ConnectionFactory};
use druid_rdbc::error::RdbcError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 记录真实创建和关闭次数的测试工厂。对应 Java：DruidDataSource 物理建连测试替身。
#[derive(Default)]
pub(super) struct LifetimeTestFactory {
    pub(super) creates: AtomicU64,
    pub(super) closes: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl ConnectionFactory for LifetimeTestFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(LifetimeTestConnection {
            closed: false,
            closes: Arc::clone(&self.closes),
        }))
    }
    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}
