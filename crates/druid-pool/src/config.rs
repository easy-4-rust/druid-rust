//! 对应 Java 类：com.alibaba.druid.pool.DruidAbstractDataSource（池配置）
//!
//! 池内部配置，从 PoolConfig 翻译而来。

use druid_core::FilterChain;
use std::sync::Arc;
use std::time::Duration;

/// 池内部配置（运行时使用）。
#[derive(Clone)]
pub struct PoolInnerConfig {
    pub db_type_name: Option<String>,
    pub max_open: usize,
    pub min_idle: usize,
    pub max_idle: usize,
    pub acquire_timeout: Duration,
    /// Rust 扩展的显式最大生命周期；默认禁用。
    pub max_lifetime: Duration,
    /// Java `minEvictableIdleTimeMillis`。
    pub idle_timeout: Duration,
    /// Java `maxEvictableIdleTimeMillis`。
    pub max_evictable_idle_time: Duration,
    /// Java `phyTimeoutMillis`；`None` 对应默认值 `-1`。
    pub physical_connection_timeout: Option<Duration>,
    pub test_on_borrow: bool,
    pub test_on_return: bool,
    pub keep_alive: bool,
    pub keep_alive_between_time: Duration,
    pub keep_connection_underlying_transaction_isolation: bool,
    pub max_use_count: usize,
    pub default_auto_commit: bool,
    pub default_read_only: Option<bool>,
    pub default_transaction_isolation: Option<u8>,
    pub default_catalog: Option<String>,
    pub pool_prepared_statements: bool,
    pub max_pool_prepared_statements_per_connection: usize,
    pub share_prepared_statements: bool,
    pub use_oracle_implicit_cache: bool,
}

impl Default for PoolInnerConfig {
    fn default() -> Self {
        Self {
            db_type_name: None,
            max_open: 8,
            min_idle: 0,
            max_idle: 8,
            acquire_timeout: Duration::from_secs(30),
            max_lifetime: Duration::MAX,
            idle_timeout: Duration::from_secs(1800),
            max_evictable_idle_time: Duration::from_secs(7 * 60 * 60),
            physical_connection_timeout: None,
            test_on_borrow: false,
            test_on_return: false,
            keep_alive: false,
            keep_alive_between_time: Duration::from_secs(120),
            keep_connection_underlying_transaction_isolation: false,
            max_use_count: 0,
            default_auto_commit: true,
            default_read_only: None,
            default_transaction_isolation: None,
            default_catalog: None,
            pool_prepared_statements: false,
            max_pool_prepared_statements_per_connection: 10,
            share_prepared_statements: false,
            use_oracle_implicit_cache: false,
        }
    }
}

/// DruidPool Builder。
pub struct DruidPoolBuilder {
    name: String,
    driver_name: String,
    db_type_name: Option<String>,
    factory: Option<Arc<dyn druid_core::PhysicalConnectionFactory>>,
    max_open: usize,
    min_idle: usize,
    max_idle: usize,
    acquire_timeout: Duration,
    max_lifetime: Duration,
    idle_timeout: Duration,
    max_evictable_idle_time: Duration,
    physical_connection_timeout: Option<Duration>,
    test_on_borrow: bool,
    test_on_return: bool,
    keep_alive: bool,
    keep_alive_between_time: Duration,
    keep_connection_underlying_transaction_isolation: bool,
    max_use_count: usize,
    default_auto_commit: bool,
    default_read_only: Option<bool>,
    default_transaction_isolation: Option<u8>,
    default_catalog: Option<String>,
    pool_prepared_statements: bool,
    max_pool_prepared_statements_per_connection: usize,
    share_prepared_statements: bool,
    use_oracle_implicit_cache: bool,
    filter_chain: Option<Arc<FilterChain>>,
}

impl DruidPoolBuilder {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            driver_name: String::new(),
            db_type_name: None,
            factory: None,
            max_open: 8,
            min_idle: 0,
            max_idle: 8,
            acquire_timeout: Duration::from_secs(30),
            max_lifetime: Duration::MAX,
            idle_timeout: Duration::from_secs(1800),
            max_evictable_idle_time: Duration::from_secs(7 * 60 * 60),
            physical_connection_timeout: None,
            test_on_borrow: false,
            test_on_return: false,
            keep_alive: false,
            keep_alive_between_time: Duration::from_secs(120),
            keep_connection_underlying_transaction_isolation: false,
            max_use_count: 0,
            default_auto_commit: true,
            default_read_only: None,
            default_transaction_isolation: None,
            default_catalog: None,
            pool_prepared_statements: false,
            max_pool_prepared_statements_per_connection: 10,
            share_prepared_statements: false,
            use_oracle_implicit_cache: false,
            filter_chain: None,
        }
    }

    pub fn name(mut self, v: impl Into<String>) -> Self {
        self.name = v.into();
        self
    }
    pub fn driver_name(mut self, v: impl Into<String>) -> Self {
        self.driver_name = v.into();
        self
    }
    /// 设置数据库类型名称。
    ///
    /// 对应 Java：`DruidAbstractDataSource#dbTypeName`。`odps` 会跳过
    /// `defaultAutoCommit` 初始化，其余默认连接属性仍按 Java 顺序应用。
    pub fn db_type_name(mut self, db_type_name: impl Into<String>) -> Self {
        self.db_type_name = Some(db_type_name.into());
        self
    }
    pub fn factory(mut self, factory: Arc<dyn druid_core::PhysicalConnectionFactory>) -> Self {
        self.factory = Some(factory);
        self
    }
    pub fn max_open(mut self, v: usize) -> Self {
        self.max_open = v;
        self
    }
    pub fn min_idle(mut self, v: usize) -> Self {
        self.min_idle = v;
        self
    }
    pub fn max_idle(mut self, v: usize) -> Self {
        self.max_idle = v;
        self
    }
    pub fn acquire_timeout(mut self, v: Duration) -> Self {
        self.acquire_timeout = v;
        self
    }
    /// 设置 Rust 扩展的物理连接最大生命周期。
    ///
    /// Java Druid 默认没有这一额外限制，因此默认值为 `Duration::MAX`。
    /// Java `phyTimeoutMillis` 应使用 [`Self::physical_connection_timeout`]。
    pub fn max_lifetime(mut self, max_lifetime: Duration) -> Self {
        self.max_lifetime = max_lifetime;
        self
    }

    /// 设置最小可驱逐空闲时间。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setMinEvictableIdleTimeMillis(long)`。
    pub fn idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    /// 设置最大可驱逐空闲时间。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setMaxEvictableIdleTimeMillis(long)`。
    pub fn max_evictable_idle_time(mut self, max_evictable_idle_time: Duration) -> Self {
        self.max_evictable_idle_time = max_evictable_idle_time;
        self
    }

    /// 设置物理连接绝对寿命。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setPhyTimeoutMillis(long)`。
    /// Java 默认 `-1` 表示禁用，因此 Rust 使用 `Option<Duration>` 保存配置。
    pub fn physical_connection_timeout(mut self, physical_connection_timeout: Duration) -> Self {
        self.physical_connection_timeout = Some(physical_connection_timeout);
        self
    }

    /// `physical_connection_timeout` 的 Java 字段名兼容入口。
    pub fn phy_timeout(self, phy_timeout: Duration) -> Self {
        self.physical_connection_timeout(phy_timeout)
    }

    pub fn test_on_borrow(mut self, v: bool) -> Self {
        self.test_on_borrow = v;
        self
    }

    /// 设置归还连接时是否执行有效性检查。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setTestOnReturn(boolean)`。
    pub fn test_on_return(mut self, test_on_return: bool) -> Self {
        self.test_on_return = test_on_return;
        self
    }

    /// 设置空闲连接保活开关。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setKeepAlive(boolean)`。
    pub fn keep_alive(mut self, keep_alive: bool) -> Self {
        self.keep_alive = keep_alive;
        self
    }

    /// 设置空闲连接保活检查间隔。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setKeepAliveBetweenTimeMillis(long)`。
    pub fn keep_alive_between_time(mut self, keep_alive_between_time: Duration) -> Self {
        self.keep_alive_between_time = keep_alive_between_time;
        self
    }

    /// 设置回收时是否保留底层事务隔离级别。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setKeepConnectionUnderlyingTransactionIsolation(boolean)`。
    pub fn keep_connection_underlying_transaction_isolation(
        mut self,
        keep_connection_underlying_transaction_isolation: bool,
    ) -> Self {
        self.keep_connection_underlying_transaction_isolation =
            keep_connection_underlying_transaction_isolation;
        self
    }

    /// 设置单个物理连接最大借用次数；`0` 表示不限制。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setPhyMaxUseCount(int)`，Rust 使用
    /// `0` 表示 Java 的 `-1`/禁用状态，避免无符号类型伪造负值。
    pub fn max_use_count(mut self, max_use_count: usize) -> Self {
        self.max_use_count = max_use_count;
        self
    }

    /// 设置物理连接进入池前的默认自动提交状态。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setDefaultAutoCommit(boolean)`。
    pub fn default_auto_commit(mut self, default_auto_commit: bool) -> Self {
        self.default_auto_commit = default_auto_commit;
        self
    }

    /// 设置物理连接进入池前的默认只读状态。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setDefaultReadOnly(Boolean)`。
    /// 未调用时保留驱动创建连接后的只读状态。
    pub fn default_read_only(mut self, default_read_only: bool) -> Self {
        self.default_read_only = Some(default_read_only);
        self
    }

    /// 设置物理连接进入池前的默认事务隔离级别。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setDefaultTransactionIsolation(Integer)`。
    /// 未调用时保留驱动创建连接后的隔离级别。
    pub fn default_transaction_isolation(mut self, default_transaction_isolation: u8) -> Self {
        self.default_transaction_isolation = Some(default_transaction_isolation);
        self
    }

    /// 设置物理连接进入池前的默认 catalog。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setDefaultCatalog(String)`。空字符串
    /// 与 Java 一致，不触发 `setCatalog`。
    pub fn default_catalog(mut self, default_catalog: impl Into<String>) -> Self {
        self.default_catalog = Some(default_catalog.into());
        self
    }

    /// 启用单物理连接 PreparedStatement 缓存。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setPoolPreparedStatements(boolean)`。
    pub fn pool_prepared_statements(mut self, pool_prepared_statements: bool) -> Self {
        self.pool_prepared_statements = pool_prepared_statements;
        self
    }

    /// 设置每个物理连接的 PreparedStatement LRU 上限。
    ///
    /// 对应 Java：
    /// `setMaxPoolPreparedStatementPerConnectionSize(int)` /
    /// `setMaxOpenPreparedStatements(int)`。
    pub fn max_pool_prepared_statements_per_connection(
        mut self,
        max_pool_prepared_statements_per_connection: usize,
    ) -> Self {
        self.max_pool_prepared_statements_per_connection =
            max_pool_prepared_statements_per_connection;
        self
    }

    /// 设置正在使用的缓存语句是否可共享。
    ///
    /// 对应 Java：`setSharePreparedStatements(boolean)`。
    pub fn share_prepared_statements(mut self, share_prepared_statements: bool) -> Self {
        self.share_prepared_statements = share_prepared_statements;
        self
    }

    /// 设置 Oracle implicit statement cache 适配开关。
    ///
    /// 对应 Java：`setUseOracleImplicitCache(boolean)`。
    pub fn use_oracle_implicit_cache(mut self, use_oracle_implicit_cache: bool) -> Self {
        self.use_oracle_implicit_cache = use_oracle_implicit_cache;
        self
    }

    pub fn filter_chain(mut self, fc: Arc<FilterChain>) -> Self {
        self.filter_chain = Some(fc);
        self
    }

    pub async fn build(self) -> Result<crate::DruidPool, druid_core::DruidError> {
        let factory = self
            .factory
            .ok_or(druid_core::DruidError::Other("factory required".into()))?;
        let inner_config = PoolInnerConfig {
            db_type_name: self.db_type_name,
            max_open: self.max_open,
            min_idle: self.min_idle,
            max_idle: self.max_idle,
            acquire_timeout: self.acquire_timeout,
            max_lifetime: self.max_lifetime,
            idle_timeout: self.idle_timeout,
            max_evictable_idle_time: self.max_evictable_idle_time,
            physical_connection_timeout: self.physical_connection_timeout,
            test_on_borrow: self.test_on_borrow,
            test_on_return: self.test_on_return,
            keep_alive: self.keep_alive,
            keep_alive_between_time: self.keep_alive_between_time,
            keep_connection_underlying_transaction_isolation: self
                .keep_connection_underlying_transaction_isolation,
            max_use_count: self.max_use_count,
            default_auto_commit: self.default_auto_commit,
            default_read_only: self.default_read_only,
            default_transaction_isolation: self.default_transaction_isolation,
            default_catalog: self.default_catalog,
            pool_prepared_statements: self.pool_prepared_statements,
            max_pool_prepared_statements_per_connection: self
                .max_pool_prepared_statements_per_connection,
            share_prepared_statements: self.share_prepared_statements,
            use_oracle_implicit_cache: self.use_oracle_implicit_cache,
        };
        Ok(crate::DruidPool::new(
            self.name,
            self.driver_name,
            factory,
            inner_config,
            self.filter_chain,
        ))
    }
}

impl Default for DruidPoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}
