//! `SQLx` canonical 连接的 deadpool 管理器。

use crate::sqlx::external_pool_connection_control::take_poisoned;
use crate::sqlx::SqlxConnectionFactory;
use deadpool::managed::{Manager, Metrics, RecycleResult};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use std::sync::atomic::{AtomicU64, Ordering};

/// `SQLx` canonical 连接的 deadpool 管理器。
/// 每次 `create` 只创建一个未池化的 `SqlxConnectionAdapter`。
#[derive(Debug, Clone)]
pub struct SqlxDeadpoolConnectionManager {
    factory: SqlxConnectionFactory,
    create_count: std::sync::Arc<AtomicU64>,
}

impl SqlxDeadpoolConnectionManager {
    /// 创建 deadpool 连接管理器。
    ///
    /// 参数 `url` 为 `SQLx` 数据库连接 URL。
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            factory: SqlxConnectionFactory::new(url),
            create_count: std::sync::Arc::new(AtomicU64::new(0)),
        }
    }

    /// 返回数据库连接 URL。
    pub fn url(&self) -> &str {
        self.factory.url()
    }

    /// 返回成功创建的物理连接总数。
    pub fn create_count(&self) -> u64 {
        self.create_count.load(Ordering::Relaxed)
    }
}

impl Manager for SqlxDeadpoolConnectionManager {
    type Type = Box<dyn Connection>;
    type Error = RdbcError;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let connection = self.factory.create().await?;
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(connection)
    }

    async fn recycle(
        &self,
        connection: &mut Self::Type,
        _metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        if take_poisoned(connection.as_ref()) || connection.is_closed().unwrap_or(true) {
            return Err(deadpool::managed::RecycleError::Backend(
                RdbcError::ConnectionDiscarded,
            ));
        }
        self.factory
            .validate(connection)
            .await
            .map_err(deadpool::managed::RecycleError::Backend)
    }
}
