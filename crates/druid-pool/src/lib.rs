//! druid-pool — HikariCP-style async connection pool.

pub mod config;
pub mod pool_inner;

use druid_core::{
    Connection, ConnectionFactory, DruidError, ExecContext, ExecResult,
    FilterChain, PoolState, Value, Row,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

use config::{DruidPoolBuilder, PoolInnerConfig};
use pool_inner::PoolInner;

/// Druid 风格连接池。
pub struct DruidPool {
    name: String,
    driver_name: String,
    inner: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
}

impl DruidPool {
    pub fn new(
        name: String, driver_name: String,
        factory: Arc<dyn ConnectionFactory>, config: PoolInnerConfig,
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
            name: self.name.clone(),
            driver_name: self.driver_name.clone(),
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

/// 池化连接，Drop 时自动归还。
pub struct DruidPoolConnection {
    conn: Option<Box<dyn Connection>>,
    id: u64,
    pool: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
}

impl std::fmt::Debug for DruidPoolConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DruidPoolConnection")
            .field("id", &self.id)
            .field("has_conn", &self.conn.is_some())
            .finish()
    }
}

impl DruidPoolConnection {
    fn new(conn: Box<dyn Connection>, id: u64, pool: Arc<PoolInner>, filter_chain: Option<Arc<FilterChain>>) -> Self {
        Self { conn: Some(conn), id, pool, filter_chain }
    }

    pub fn id(&self) -> u64 { self.id }

    pub async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        let start = Instant::now();
        if let Some(ref fc) = self.filter_chain {
            let mut ctx = ExecContext { sql, params: &params, data_source: "", start, fingerprint: None };
            fc.before_execute(&mut ctx).await?;
        }
        let result = self.conn.as_mut().expect("taken").exec(sql, params).await;
        let elapsed = start.elapsed();
        if let Some(ref fc) = self.filter_chain {
            let ctx = ExecContext { sql, params: &[], data_source: "", start, fingerprint: None };
            fc.after_execute(&ctx, &result, elapsed).await;
        }
        result
    }

    pub async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.conn.as_mut().expect("taken").fetch(sql, params).await
    }

    pub async fn ping(&mut self) -> Result<(), DruidError> {
        self.conn.as_mut().expect("taken").ping().await
    }

    pub fn driver_name(&self) -> &str {
        self.conn.as_ref().map(|c: &Box<dyn Connection>| c.driver_name()).unwrap_or("")
    }
}

impl Drop for DruidPoolConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool.return_connection(conn, self.id);
        }
    }
}

// Implement druid_core::Pool trait so DruidPool can be used as Arc<dyn Pool>
#[async_trait::async_trait]
impl druid_core::Pool for DruidPool {
    async fn get(&self) -> Result<druid_core::PooledConnection, DruidError> {
        let timeout = self.inner.config.acquire_timeout;
        self.get_timeout(timeout).await.map(|c| c.into_core())
    }

    async fn get_timeout(&self, timeout: Duration) -> Result<druid_core::PooledConnection, DruidError> {
        self.get_timeout(timeout).await.map(|c| c.into_core())
    }

    fn state(&self) -> PoolState {
        self.state()
    }

    fn driver_name(&self) -> &str {
        &self.driver_name
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl DruidPoolConnection {
    /// Convert into core PooledConnection (consumes self, disables auto-return).
    pub fn into_core(mut self) -> druid_core::PooledConnection {
        let conn = self.conn.take().expect("connection taken");
        let id = self.id;
        druid_core::PooledConnection::new(
            conn,
            id,
            Box::new(|_conn, _id| { /* dropped without returning — caller owns it */ }),
        )
    }
}
