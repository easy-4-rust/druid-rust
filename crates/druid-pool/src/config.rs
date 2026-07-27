//! 对应 Java 类：com.alibaba.druid.pool.DruidAbstractDataSource（池配置）
//!
//! 池内部配置，从 PoolConfig 翻译而来。

use druid_core::FilterChain;
use std::sync::Arc;
use std::time::Duration;

/// 池内部配置（运行时使用）。
#[derive(Clone)]
pub struct PoolInnerConfig {
    pub max_open: usize,
    pub min_idle: usize,
    pub max_idle: usize,
    pub acquire_timeout: Duration,
    pub max_lifetime: Duration,
    pub idle_timeout: Duration,
    pub test_on_borrow: bool,
}

impl Default for PoolInnerConfig {
    fn default() -> Self {
        Self {
            max_open: 8,
            min_idle: 0,
            max_idle: 8,
            acquire_timeout: Duration::from_secs(30),
            max_lifetime: Duration::from_secs(1800),
            idle_timeout: Duration::from_secs(600),
            test_on_borrow: false,
        }
    }
}

/// DruidPool Builder。
pub struct DruidPoolBuilder {
    name: String,
    driver_name: String,
    factory: Option<Arc<dyn druid_core::ConnectionFactory>>,
    max_open: usize,
    min_idle: usize,
    max_idle: usize,
    acquire_timeout: Duration,
    max_lifetime: Duration,
    idle_timeout: Duration,
    test_on_borrow: bool,
    filter_chain: Option<Arc<FilterChain>>,
}

impl DruidPoolBuilder {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            driver_name: String::new(),
            factory: None,
            max_open: 8,
            min_idle: 0,
            max_idle: 8,
            acquire_timeout: Duration::from_secs(30),
            max_lifetime: Duration::from_secs(1800),
            idle_timeout: Duration::from_secs(600),
            test_on_borrow: false,
            filter_chain: None,
        }
    }

    pub fn name(mut self, v: impl Into<String>) -> Self { self.name = v.into(); self }
    pub fn driver_name(mut self, v: impl Into<String>) -> Self { self.driver_name = v.into(); self }
    pub fn factory(mut self, f: Arc<dyn druid_core::ConnectionFactory>) -> Self { self.factory = Some(f); self }
    pub fn max_open(mut self, v: usize) -> Self { self.max_open = v; self }
    pub fn min_idle(mut self, v: usize) -> Self { self.min_idle = v; self }
    pub fn max_idle(mut self, v: usize) -> Self { self.max_idle = v; self }
    pub fn acquire_timeout(mut self, v: Duration) -> Self { self.acquire_timeout = v; self }
    pub fn test_on_borrow(mut self, v: bool) -> Self { self.test_on_borrow = v; self }
    pub fn filter_chain(mut self, fc: Arc<FilterChain>) -> Self { self.filter_chain = Some(fc); self }

    pub async fn build(self) -> Result<crate::DruidPool, druid_core::DruidError> {
        let factory = self.factory.ok_or(druid_core::DruidError::Other("factory required".into()))?;
        let inner_config = PoolInnerConfig {
            max_open: self.max_open,
            min_idle: self.min_idle,
            max_idle: self.max_idle,
            acquire_timeout: self.acquire_timeout,
            max_lifetime: self.max_lifetime,
            idle_timeout: self.idle_timeout,
            test_on_borrow: self.test_on_borrow,
        };
        Ok(crate::DruidPool::new(self.name, self.driver_name, factory, inner_config, self.filter_chain))
    }
}

impl Default for DruidPoolBuilder {
    fn default() -> Self { Self::new() }
}
