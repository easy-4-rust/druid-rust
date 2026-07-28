//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource
//!
//! Druid 风格连接池。

use crate::config::DruidPoolBuilder;
use crate::pool_inner::PoolInner;
use druid_core::{
    DruidConnectionHolder, DruidError, DruidPooledConnection, FilterChain,
    PhysicalConnectionFactory, PoolState,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Druid 风格连接池。
///
/// 对应 Druid Java 的 `DruidDataSource`，实现 max_open / min_idle /
/// acquire_timeout / FilterChain 装配 / DruidPooledConnection::drop 归还。
pub struct DruidPool {
    name: String,
    driver_name: String,
    inner: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
}

impl DruidPool {
    pub fn new(
        name: String,
        driver_name: String,
        factory: Arc<dyn PhysicalConnectionFactory>,
        config: crate::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Self {
        Self {
            name,
            driver_name,
            inner: Arc::new(PoolInner::new(factory, config)),
            filter_chain,
        }
    }

    pub fn builder() -> DruidPoolBuilder {
        DruidPoolBuilder::new()
    }

    pub async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.get_timeout(self.inner.config.acquire_timeout).await
    }

    pub async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        if self.inner.closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(DruidError::PoolClosed);
        }
        let deadline = Instant::now() + timeout;
        self.inner
            .connect_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        loop {
            let (idle_connection, remaining_idle) = {
                let mut idle = self.inner.idle.lock();
                let connection = idle.pop_front();
                (connection, idle.len())
            };
            if let Some(mut holder) = idle_connection {
                let lifetime_expired = holder.physical_age() >= self.inner.config.max_lifetime;
                let idle_expired = remaining_idle >= self.inner.config.min_idle
                    && holder.idle_duration() >= self.inner.config.idle_timeout;
                if lifetime_expired || idle_expired {
                    self.inner.destroy_holder(holder);
                    continue;
                }
                if self.inner.config.test_on_borrow {
                    let validation_failed = match holder.physical_connection_box_mut() {
                        Some(connection) => self.inner.factory.validate(connection).await.is_err(),
                        None => true,
                    };
                    if validation_failed {
                        self.inner.destroy_holder(holder);
                        continue;
                    }
                    holder.record_valid();
                }
                if !holder.mark_active() {
                    self.inner.destroy_holder(holder);
                    continue;
                }
                self.inner
                    .active_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(self.wrap_connection(holder));
            }
            match self.inner.create_connection().await {
                Ok(holder) => {
                    if !holder.mark_active() {
                        self.inner.destroy_holder(holder);
                        continue;
                    }
                    self.inner
                        .active_count
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Ok(self.wrap_connection(holder));
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
            name: self.name.clone(),
            driver_name: self.driver_name.clone(),
            max_open: self.inner.config.max_open,
            active_count: self
                .inner
                .active_count
                .load(std::sync::atomic::Ordering::Relaxed),
            idle_count: self.inner.idle.lock().len(),
            create_count: self
                .inner
                .create_count
                .load(std::sync::atomic::Ordering::Relaxed),
            close_count: self
                .inner
                .close_count
                .load(std::sync::atomic::Ordering::Relaxed),
            destroy_count: self
                .inner
                .destroy_count
                .load(std::sync::atomic::Ordering::Relaxed),
            connect_count: self
                .inner
                .connect_count
                .load(std::sync::atomic::Ordering::Relaxed),
            connect_error_count: self
                .inner
                .connect_error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            recycle_count: self
                .inner
                .recycle_count
                .load(std::sync::atomic::Ordering::Relaxed),
            recycle_error_count: self
                .inner
                .recycle_error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            discard_count: self
                .inner
                .discard_count
                .load(std::sync::atomic::Ordering::Relaxed),
            keep_alive_check_count: self
                .inner
                .keep_alive_check_count
                .load(std::sync::atomic::Ordering::Relaxed),
            keep_alive_check_error_count: self
                .inner
                .keep_alive_check_error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            prepared_statement_count: self
                .inner
                .prepared_statement_stats
                .prepared_statement_count(),
            closed_prepared_statement_count: self
                .inner
                .prepared_statement_stats
                .closed_prepared_statement_count(),
            cached_prepared_statement_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_count(),
            cached_prepared_statement_delete_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_delete_count(),
            cached_prepared_statement_hit_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_hit_count(),
            cached_prepared_statement_miss_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_miss_count(),
            cached_prepared_statement_access_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_access_count(),
            closed: self.inner.closed.load(std::sync::atomic::Ordering::Relaxed),
            ..Default::default()
        }
    }

    /// 将超过 `min_idle` 的空闲连接收缩掉。
    ///
    /// 对应 Java：`DruidDataSource#shrink()`，即
    /// `shrink(false, false)`。
    pub async fn shrink(&self) {
        self.inner.shrink(false, false).await;
    }

    /// 按时间执行空闲连接收缩。
    ///
    /// 对应 Java：`DruidDataSource#shrink(boolean)`；保活参数取数据源配置。
    ///
    /// # 参数
    /// - `check_time`：是否应用空闲与物理寿命阈值。
    pub async fn shrink_check_time(&self, check_time: bool) {
        self.inner
            .shrink(check_time, self.inner.config.keep_alive)
            .await;
    }

    /// 按显式时间与保活选项执行空闲连接收缩。
    ///
    /// 对应 Java：`DruidDataSource#shrink(boolean, boolean)`。
    ///
    /// # 参数
    /// - `check_time`：是否应用空闲与物理寿命阈值。
    /// - `keep_alive`：是否验证到期的空闲连接。
    pub async fn shrink_with_options(&self, check_time: bool, keep_alive: bool) {
        self.inner.shrink(check_time, keep_alive).await;
    }

    pub async fn close(&self) {
        self.inner.close().await;
    }
    pub fn filter_chain(&self) -> Option<&Arc<FilterChain>> {
        self.filter_chain.as_ref()
    }
    pub fn driver_name(&self) -> &str {
        &self.driver_name
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    fn wrap_connection(&self, holder: DruidConnectionHolder) -> DruidPooledConnection {
        let pool = self.inner.clone();
        let recycle_validator = self
            .inner
            .config
            .test_on_return
            .then(|| self.inner.factory.clone());
        DruidPooledConnection::with_holder(
            holder,
            self.name.clone(),
            self.filter_chain.clone(),
            self.inner
                .config
                .keep_connection_underlying_transaction_isolation,
            recycle_validator,
            Box::new(move |holder, disposition| {
                pool.return_connection(holder, disposition);
            }),
        )
    }
}

#[async_trait::async_trait]
impl druid_core::Pool for DruidPool {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        DruidPool::get(self).await
    }

    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
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
