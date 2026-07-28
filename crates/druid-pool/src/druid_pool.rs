//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource
//!
//! Druid 风格连接池。

use crate::config::DruidPoolBuilder;
use crate::pool_inner::PoolInner;
use druid_core::{
    ConnectionFactory, DruidError, DruidPooledConnection, FilterChain, PoolState,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Druid 风格连接池。
///
/// 对应 Druid Java 的 `DruidDataSource`，实现 max_open / min_idle /
/// acquire_timeout / FilterChain 装配 / PooledConnection::drop 归还。
pub struct DruidPool {
    name: String,
    driver_name: String,
    inner: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
}

impl DruidPool {
    pub fn new(
        name: String, driver_name: String,
        factory: Arc<dyn ConnectionFactory>, config: crate::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Self {
        Self { name, driver_name, inner: Arc::new(PoolInner::new(factory, config)), filter_chain }
    }

    pub fn builder() -> DruidPoolBuilder { DruidPoolBuilder::new() }

    pub async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.get_timeout(self.inner.config.acquire_timeout).await
    }

    pub async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        if self.inner.closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(DruidError::PoolClosed);
        }
        let deadline = Instant::now() + timeout;
        self.inner.connect_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        loop {
            let idle_connection = {
                let mut idle = self.inner.idle.lock();
                idle.pop_front()
            };
            if let Some(mut item) = idle_connection {
                if self.inner.config.test_on_borrow
                    && self.inner.factory.validate(&mut item.conn).await.is_err()
                {
                    self.inner.destroy_connection(item.conn);
                    continue;
                }
                self.inner.active_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(self.wrap_connection(item.conn, item.id));
            }
            match self.inner.create_connection().await {
                Ok(conn) => {
                    let id = self.inner.next_id();
                    self.inner.active_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Ok(self.wrap_connection(conn, id));
                }
                Err(DruidError::PoolExhausted) => {}
                Err(_) if !self.inner.idle.lock().is_empty() => continue,
                Err(e) => return Err(e),
            }
            let notify = self.inner.notify.notified();
            tokio::pin!(notify);
            match tokio::time::timeout_at(deadline.into(), notify).await {
                Ok(_) => continue,
                Err(_) => return Err(DruidError::AcquireTimeout),
            }
        }
    }

    pub fn state(&self) -> PoolState {
        PoolState {
            name: self.name.clone(), driver_name: self.driver_name.clone(),
            max_open: self.inner.config.max_open,
            active_count: self.inner.active_count.load(std::sync::atomic::Ordering::Relaxed),
            idle_count: self.inner.idle.lock().len(),
            create_count: self.inner.create_count.load(std::sync::atomic::Ordering::Relaxed),
            close_count: self.inner.close_count.load(std::sync::atomic::Ordering::Relaxed),
            connect_count: self.inner.connect_count.load(std::sync::atomic::Ordering::Relaxed),
            connect_error_count: self.inner.connect_error_count.load(std::sync::atomic::Ordering::Relaxed),
            recycle_count: self.inner.recycle_count.load(std::sync::atomic::Ordering::Relaxed),
            closed: self.inner.closed.load(std::sync::atomic::Ordering::Relaxed),
            ..Default::default()
        }
    }

    pub async fn close(&self) { self.inner.close().await; }
    pub fn filter_chain(&self) -> Option<&Arc<FilterChain>> { self.filter_chain.as_ref() }
    pub fn driver_name(&self) -> &str { &self.driver_name }
    pub fn name(&self) -> &str { &self.name }

    fn wrap_connection(
        &self,
        connection: Box<dyn druid_core::PhysicalConnection>,
        id: u64,
    ) -> DruidPooledConnection {
        let pool = self.inner.clone();
        DruidPooledConnection::with_context(
            connection,
            id,
            self.name.clone(),
            self.filter_chain.clone(),
            Box::new(move |connection, connection_id| {
                pool.return_connection(connection, connection_id);
            }),
        )
    }
}

#[async_trait::async_trait]
impl druid_core::Pool for DruidPool {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        DruidPool::get(self).await
    }

    async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        DruidPool::get_timeout(self, timeout).await
    }

    fn state(&self) -> PoolState {
        DruidPool::state(self)
    }

    fn driver_name(&self) -> &str {
        DruidPool::driver_name(self)
    }

    fn name(&self) -> &str {
        DruidPool::name(self)
    }
}
