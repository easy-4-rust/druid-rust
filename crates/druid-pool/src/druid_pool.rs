//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource
//!
//! Druid 风格连接池。

use crate::config::DruidPoolBuilder;
use crate::pool_inner::PoolInner;
use druid_core::{
    ConnectionFactory, DruidError, ExecContext, ExecResult,
    FilterChain, PoolState, Value, Row,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::pooled_connection::DruidPoolConnection;

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

    pub async fn get(&self) -> Result<DruidPoolConnection, DruidError> {
        self.get_timeout(self.inner.config.acquire_timeout).await
    }

    pub async fn get_timeout(&self, timeout: Duration) -> Result<DruidPoolConnection, DruidError> {
        if self.inner.closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(DruidError::PoolClosed);
        }
        let deadline = Instant::now() + timeout;
        self.inner.connect_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        loop {
            if let Some(item) = self.inner.idle.lock().pop_front() {
                self.inner.active_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(DruidPoolConnection::new(item.conn, item.id, self.inner.clone(), self.filter_chain.clone()));
            }
            if self.inner.can_grow() {
                match self.inner.create_connection().await {
                    Ok(conn) => {
                        let id = self.inner.next_id();
                        self.inner.active_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return Ok(DruidPoolConnection::new(conn, id, self.inner.clone(), self.filter_chain.clone()));
                    }
                    Err(_) if !self.inner.idle.lock().is_empty() => continue,
                    Err(e) => return Err(e),
                }
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
}
