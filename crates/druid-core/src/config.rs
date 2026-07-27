//! 对应 Java 类：com.alibaba.druid.pool.DruidAbstractDataSource
//! 来源文件：core/src/main/java/com/alibaba/druid/pool/DruidAbstractDataSource.java
//!
//! 连接池配置，替代 DruidAbstractDataSource 的 100+ setter。
//! 每个字段对应 Druid Java 的一个 setter，保留语义一致性。

use std::time::Duration;

/// 连接池配置。
///
/// 对应 Druid Java 的 `DruidAbstractDataSource` 配置字段，
/// 使用 Builder 模式替代 Java 的 setter 链。
#[derive(Debug, Clone)]
pub struct PoolConfig {
    // ── 数据源标识 ──────────────────────────────────────────────
    /// 数据源名称（对应 name）
    pub name: String,
    /// JDBC 风格 URL（对应 jdbcUrl）
    pub url: String,
    /// 驱动名称（对应 driverClassName）
    pub driver_name: String,
    /// 用户名（对应 username）
    pub username: String,
    /// 密码（对应 password）
    pub password: String,
    /// JDBC 连接属性（对应 connectProperties）
    pub connect_properties: std::collections::HashMap<String, String>,

    // ── 连接池大小 ──────────────────────────────────────────────
    /// 最大活跃连接数（对应 maxActive，默认 8）
    pub max_open: usize,
    /// 最小空闲连接数（对应 minIdle，默认 0）
    pub min_idle: usize,
    /// 初始连接数（对应 initialSize，默认 0）
    pub initial_size: usize,

    // ── 超时 ───────────────────────────────────────────────────
    /// 获取连接超时（对应 maxWait，默认 30s）
    pub acquire_timeout: Duration,
    /// 连接最大生命周期（对应 maxEvictableIdleTimeMillis，默认 7h）
    pub max_lifetime: Duration,
    /// 空闲驱逐间隔（对应 timeBetweenEvictionRunsMillis，默认 60s）
    pub eviction_interval: Duration,
    /// 最小可驱逐空闲时间（对应 minEvictableIdleTimeMillis，默认 30min）
    pub min_evictable_idle: Duration,

    // ── 验证 ───────────────────────────────────────────────────
    /// 借出时验证（对应 testOnBorrow，默认 false）
    pub test_on_borrow: bool,
    /// 归还时验证（对应 testOnReturn，默认 false）
    pub test_on_return: bool,
    /// 空闲时验证（对应 testWhileIdle，默认 false）
    pub test_while_idle: bool,
    /// 验证 SQL（对应 validationQuery）
    pub validation_query: Option<String>,
    /// 验证查询超时（对应 validationQueryTimeout，默认 0）
    pub validation_query_timeout: Duration,

    // ── 保活 ───────────────────────────────────────────────────
    /// 保活开关（对应 keepAlive，默认 false）
    pub keep_alive: bool,
    /// 保活间隔（对应 keepAliveBetweenTimeMillis，默认 2min）
    pub keep_alive_interval: Duration,

    // ── 泄漏检测 ───────────────────────────────────────────────
    /// 泄漏检测开关（对应 removeAbandoned，默认 false）
    pub leak_detection: bool,
    /// 泄漏检测阈值（对应 removeAbandonedTimeout，默认 5min）
    pub leak_threshold: Duration,
    /// 泄漏检测是否记录栈追踪（对应 removeAbandonedMonitor）
    pub leak_stack_trace: bool,

    // ── 事务 ───────────────────────────────────────────────────
    /// 默认自动提交（对应 defaultAutoCommit）
    pub default_auto_commit: Option<bool>,
    /// 默认只读（对应 defaultReadOnly）
    pub default_read_only: Option<bool>,
    /// 默认事务隔离级别（对应 defaultTransactionIsolation）
    pub default_transaction_isolation: Option<u8>,

    // ── 预编译语句 ─────────────────────────────────────────────
    /// 启用预编译语句池（对应 poolPreparedStatements，默认 false）
    pub pool_prepared_statements: bool,
    /// 每连接最大预编译语句数（对应 maxPoolPreparedStatementPerConnectionSize，默认 10）
    pub max_pool_prepared_statements_per_connection: usize,

    // ── 慢 SQL ─────────────────────────────────────────────────
    /// 慢 SQL 阈值（对应 slowSqlMillis，默认 2s）
    pub slow_sql_threshold: Duration,
    /// SQL 合并统计开关（对应 mergeSql，默认 true）
    pub merge_sql: bool,
    /// 连接栈追踪开关（对应 connectionStackTrace，默认 false）
    pub connection_stack_trace: bool,

    // ── 锁与调度 ───────────────────────────────────────────────
    /// 公平锁（对应 useUnfairLock，默认 true）
    pub use_unfair_lock: bool,
    /// 获取失败后是否断开（对应 breakAfterAcquireFailure，默认 false）
    pub break_after_acquire_failure: bool,
    /// 连接错误重试次数（对应 connectionErrorRetryAttempts，默认 1）
    pub connection_error_retry_attempts: usize,
    /// 异步关闭连接（对应 asyncCloseConnectionEnable，默认 false）
    pub async_close_connection: bool,

    // ── 连接检测器 ─────────────────────────────────────────────
    /// 验证查询超时（对应 validConnectionCheckerClassName）
    pub valid_connection_check_class: Option<String>,

    // ── 其他 ───────────────────────────────────────────────────
    /// 丢弃连接时记录日志（对应 dupCloseLogEnable，默认 true）
    pub dup_close_log_enable: bool,
    /// 嵌套池（对应 statLoggerClassName）
    pub stat_logger_class: Option<String>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            url: String::new(),
            driver_name: String::new(),
            username: String::new(),
            password: String::new(),
            connect_properties: std::collections::HashMap::new(),
            max_open: 8,
            min_idle: 0,
            initial_size: 0,
            acquire_timeout: Duration::from_secs(30),
            max_lifetime: Duration::from_secs(25200),
            eviction_interval: Duration::from_secs(60),
            min_evictable_idle: Duration::from_secs(1800),
            test_on_borrow: false,
            test_on_return: false,
            test_while_idle: false,
            validation_query: None,
            validation_query_timeout: Duration::ZERO,
            keep_alive: false,
            keep_alive_interval: Duration::from_secs(120),
            leak_detection: false,
            leak_threshold: Duration::from_secs(300),
            leak_stack_trace: false,
            default_auto_commit: None,
            default_read_only: None,
            default_transaction_isolation: None,
            pool_prepared_statements: false,
            max_pool_prepared_statements_per_connection: 10,
            slow_sql_threshold: Duration::from_secs(2),
            merge_sql: true,
            connection_stack_trace: false,
            use_unfair_lock: true,
            break_after_acquire_failure: false,
            connection_error_retry_attempts: 1,
            async_close_connection: false,
            valid_connection_check_class: None,
            dup_close_log_enable: true,
            stat_logger_class: None,
        }
    }
}

/// PoolConfig Builder。
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
    pub fn max_lifetime(mut self, v: Duration) -> Self { self.0.max_lifetime = v; self }
    pub fn eviction_interval(mut self, v: Duration) -> Self { self.0.eviction_interval = v; self }
    pub fn min_evictable_idle(mut self, v: Duration) -> Self { self.0.min_evictable_idle = v; self }
    pub fn test_on_borrow(mut self, v: bool) -> Self { self.0.test_on_borrow = v; self }
    pub fn test_on_return(mut self, v: bool) -> Self { self.0.test_on_return = v; self }
    pub fn test_while_idle(mut self, v: bool) -> Self { self.0.test_while_idle = v; self }
    pub fn validation_query(mut self, v: impl Into<String>) -> Self { self.0.validation_query = Some(v.into()); self }
    pub fn keep_alive(mut self, v: bool) -> Self { self.0.keep_alive = v; self }
    pub fn leak_detection(mut self, v: bool) -> Self { self.0.leak_detection = v; self }
    pub fn leak_threshold(mut self, v: Duration) -> Self { self.0.leak_threshold = v; self }
    pub fn slow_sql_threshold(mut self, v: Duration) -> Self { self.0.slow_sql_threshold = v; self }
    pub fn pool_prepared_statements(mut self, v: bool) -> Self { self.0.pool_prepared_statements = v; self }
    pub fn default_auto_commit(mut self, v: bool) -> Self { self.0.default_auto_commit = Some(v); self }
    pub fn break_after_acquire_failure(mut self, v: bool) -> Self { self.0.break_after_acquire_failure = v; self }
    pub fn connection_error_retry_attempts(mut self, v: usize) -> Self { self.0.connection_error_retry_attempts = v; self }
    pub fn async_close_connection(mut self, v: bool) -> Self { self.0.async_close_connection = v; self }
    pub fn build(self) -> PoolConfig { self.0 }
}
