//! 对应 Java 类：com.alibaba.druid.pool.DruidAbstractDataSource（配置字段）

use std::time::Duration;

/// 连接池配置。
#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub name: String,
    pub url: String,
    pub driver_name: String,
    pub username: String,
    pub password: String,
    pub max_open: usize,
    pub min_idle: usize,
    pub initial_size: usize,
    pub acquire_timeout: Duration,
    pub eviction_interval: Duration,
    pub min_evictable_idle: Duration,
    pub max_evictable_idle: Duration,
    pub test_on_borrow: bool,
    pub test_on_return: bool,
    pub test_while_idle: bool,
    pub validation_query: Option<String>,
    pub keep_alive: bool,
    pub leak_detection: bool,
    pub leak_threshold: Duration,
    pub slow_sql_threshold: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            name: String::new(), url: String::new(), driver_name: String::new(),
            username: String::new(), password: String::new(),
            max_open: 8, min_idle: 0, initial_size: 0,
            acquire_timeout: Duration::from_secs(30),
            eviction_interval: Duration::from_secs(60),
            min_evictable_idle: Duration::from_secs(1800),
            max_evictable_idle: Duration::from_secs(25200),
            test_on_borrow: false, test_on_return: false, test_while_idle: false,
            validation_query: None, keep_alive: false,
            leak_detection: false, leak_threshold: Duration::from_secs(300),
            slow_sql_threshold: Duration::from_secs(2),
        }
    }
}

pub struct PoolConfigBuilder(PoolConfig);

impl PoolConfig {
    pub fn builder() -> PoolConfigBuilder { PoolConfigBuilder(PoolConfig::default()) }
}

impl PoolConfigBuilder {
    pub fn name(mut self, v: impl Into<String>) -> Self { self.0.name = v.into(); self }
    pub fn url(mut self, v: impl Into<String>) -> Self { self.0.url = v.into(); self }
    pub fn driver_name(mut self, v: impl Into<String>) -> Self { self.0.driver_name = v.into(); self }
    pub fn username(mut self, v: impl Into<String>) -> Self { self.0.username = v.into(); self }
    pub fn password(mut self, v: impl Into<String>) -> Self { self.0.password = v.into(); self }
    pub fn max_open(mut self, v: usize) -> Self { self.0.max_open = v; self }
    pub fn min_idle(mut self, v: usize) -> Self { self.0.min_idle = v; self }
    pub fn initial_size(mut self, v: usize) -> Self { self.0.initial_size = v; self }
    pub fn acquire_timeout(mut self, v: Duration) -> Self { self.0.acquire_timeout = v; self }
    pub fn test_on_borrow(mut self, v: bool) -> Self { self.0.test_on_borrow = v; self }
    pub fn test_while_idle(mut self, v: bool) -> Self { self.0.test_while_idle = v; self }
    pub fn leak_detection(mut self, v: bool) -> Self { self.0.leak_detection = v; self }
    pub fn leak_threshold(mut self, v: Duration) -> Self { self.0.leak_threshold = v; self }
    pub fn slow_sql_threshold(mut self, v: Duration) -> Self { self.0.slow_sql_threshold = v; self }
    pub fn build(self) -> PoolConfig { self.0 }
}
