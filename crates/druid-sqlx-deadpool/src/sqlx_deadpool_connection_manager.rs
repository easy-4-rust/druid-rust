//! SQLx 物理连接的 deadpool 管理器。

use deadpool::managed::{Manager, Metrics, RecycleResult};
use druid_core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};
use druid_sqlx::SqlxConnectionFactory;
use std::sync::atomic::{AtomicU64, Ordering};

/// SQLx 物理连接的 deadpool 管理器。
///
/// 对应 Java: `DruidDataSource#createPhysicalConnection` 与连接有效性检查。
/// 每次 `create` 只创建一个未池化的 `SqlxConnectionAdapter`。
#[derive(Debug, Clone)]
pub struct SqlxDeadpoolConnectionManager {
    factory: SqlxConnectionFactory,
    create_count: std::sync::Arc<AtomicU64>,
}

impl SqlxDeadpoolConnectionManager {
    /// 创建 deadpool 连接管理器。
    ///
    /// 参数 `url` 为 SQLx 数据库连接 URL。
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
    type Type = Box<dyn PhysicalConnection>;
    type Error = DruidError;

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
        if connection.is_discarded() {
            return Err(deadpool::managed::RecycleError::Backend(
                DruidError::ConnectionDiscarded,
            ));
        }
        self.factory
            .validate(connection)
            .await
            .map_err(deadpool::managed::RecycleError::Backend)
    }
}
