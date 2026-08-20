//! 对应 Java 类：com.alibaba.druid.pool.DruidAbstractDataSource（池配置）
//!
//! 池内部配置，从 `PoolConfig` 翻译而来。

use crate::core::{
    AutoLoad, ConfigFilter, EncodingConvertFilter, ExceptionSorter, FilterChain, FilterManager,
    LogFilter, MySQL8DateTimeSqlTypeFilter, ValidConnectionChecker,
};
use crate::dynamic::RandomDataSourceValidateFilter;
use crate::sql::{DbType, WallConfig, WallFilter, WallProvider};
use crate::stats::{MergeStatFilter, StatFilter, StatsCollector};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;

use super::{DataSourceStatSink, TracingDataSourceStatSink};

const STAT_FILTER_CLASS: &str = "com.alibaba.druid.filter.stat.StatFilter";
const MERGE_STAT_FILTER_CLASS: &str = "com.alibaba.druid.filter.stat.MergeStatFilter";
const WALL_FILTER_CLASS: &str = "com.alibaba.druid.wall.WallFilter";
const MYSQL8_DATETIME_FILTER_CLASS: &str =
    "com.alibaba.druid.filter.mysql8datetime.MySQL8DateTimeSqlTypeFilter";
const CONFIG_FILTER_CLASS: &str = "com.alibaba.druid.filter.config.ConfigFilter";
const ENCODING_FILTER_CLASS: &str = "com.alibaba.druid.filter.encoding.EncodingConvertFilter";
const HA_RANDOM_VALIDATE_FILTER_CLASS: &str =
    "com.alibaba.druid.pool.ha.selector.RandomDataSourceValidateFilter";
const LOG_FILTER_ID: &str = "druid::core::LogFilter";

/// 池内部配置（运行时使用）。
#[derive(Clone)]
pub struct PoolInnerConfig {
    pub db_type_name: Option<String>,
    /// 对外配置 URL。
    pub url: Option<String>,
    /// 底层驱动 URL。
    pub raw_url: Option<String>,
    /// 物理连接创建边界的逻辑驱动属性。
    ///
    /// 对应 Java `ConnectionProxyImpl#getProperties()` 保存的 `Properties info`。
    pub connection_properties: Arc<std::collections::HashMap<String, String>>,
    pub max_open: usize,
    pub initial_size: usize,
    /// Java `asyncInit`；使用受监管 creator 异步补足 initialSize。
    pub async_init: bool,
    /// Java `initExceptionThrow`；同步初始化首次建连失败时是否返回错误。
    pub init_exception_throw: bool,
    pub min_idle: usize,
    pub max_idle: usize,
    pub acquire_timeout: Duration,
    /// Java `notFullTimeoutRetryCount`，保留负数输入的“不重试”语义。
    pub not_full_timeout_retry_count: i32,
    /// Java `maxWaitThreadCount`；`None` 对应默认值 `-1`。
    pub max_wait_thread_count: Option<usize>,
    /// Java `connectionErrorRetryAttempts`。
    pub connection_error_retry_attempts: usize,
    /// Java `breakAfterAcquireFailure`。
    pub break_after_acquire_failure: bool,
    /// Java `timeBetweenConnectErrorMillis`。
    pub time_between_connect_error: Duration,
    /// Java `failFast`。
    pub fail_fast: bool,
    /// Java `onFatalErrorMaxActive`；非正数不启用借用门限，但仍参与
    /// fatal-error 计数和旧连接重新校验。
    pub on_fatal_error_max_active: i32,
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
    pub test_while_idle: bool,
    pub time_between_eviction_runs: Duration,
    /// 数据源区间统计快照的发布周期；零表示禁用周期任务。
    pub stat_publish_interval: Duration,
    /// Rust 原生统计输出端，不暴露 Java logger 类型。
    pub stat_sink: Arc<dyn DataSourceStatSink>,
    pub validation_query: Option<String>,
    pub validation_query_timeout: Duration,
    /// Java `queryTimeout`，单位秒；0 表示不限制。
    pub query_timeout: i32,
    /// Java `transactionQueryTimeout`，单位秒；非正数回退到 `queryTimeout`。
    pub transaction_query_timeout: i32,
    /// Java 全局 `DriverManager.loginTimeout` 在 Rust 中映射为数据源局部秒数。
    pub login_timeout: i32,
    pub valid_connection_checker: Option<Arc<dyn ValidConnectionChecker>>,
    pub remove_abandoned: bool,
    pub remove_abandoned_timeout: Duration,
    pub log_abandoned: bool,
    pub keep_alive: bool,
    pub keep_alive_between_time: Duration,
    pub keep_connection_underlying_transaction_isolation: bool,
    pub max_use_count: usize,
    pub default_auto_commit: bool,
    pub default_read_only: Option<bool>,
    pub default_transaction_isolation: Option<u8>,
    pub default_catalog: Option<String>,
    /// Java `connectionInitSqls`，按配置顺序在 raw connection 上执行。
    pub connection_init_sqls: Vec<String>,
    /// Java `initVariants`；仅 `MySQL` 协议族执行 `show variables`。
    pub init_variants: bool,
    /// Java `initGlobalVariants`；仅 `MySQL` 协议族执行 `show global variables`。
    pub init_global_variants: bool,
    pub pool_prepared_statements: bool,
    pub max_pool_prepared_statements_per_connection: usize,
    pub share_prepared_statements: bool,
    pub use_oracle_implicit_cache: bool,
}

impl Default for PoolInnerConfig {
    fn default() -> Self {
        Self {
            db_type_name: None,
            url: None,
            raw_url: None,
            connection_properties: Arc::new(std::collections::HashMap::new()),
            max_open: 8,
            initial_size: 0,
            async_init: false,
            init_exception_throw: true,
            min_idle: 0,
            max_idle: 8,
            acquire_timeout: Duration::MAX,
            not_full_timeout_retry_count: 0,
            max_wait_thread_count: None,
            connection_error_retry_attempts: 1,
            break_after_acquire_failure: false,
            time_between_connect_error: Duration::from_millis(500),
            fail_fast: false,
            on_fatal_error_max_active: 0,
            max_lifetime: Duration::MAX,
            idle_timeout: Duration::from_secs(1800),
            max_evictable_idle_time: Duration::from_secs(7 * 60 * 60),
            physical_connection_timeout: None,
            test_on_borrow: false,
            test_on_return: false,
            test_while_idle: false,
            time_between_eviction_runs: Duration::from_secs(60),
            stat_publish_interval: Duration::ZERO,
            stat_sink: Arc::new(TracingDataSourceStatSink::new()),
            validation_query: None,
            validation_query_timeout: Duration::ZERO,
            query_timeout: 0,
            transaction_query_timeout: 0,
            login_timeout: 0,
            valid_connection_checker: None,
            remove_abandoned: false,
            remove_abandoned_timeout: Duration::from_secs(300),
            log_abandoned: false,
            keep_alive: false,
            keep_alive_between_time: Duration::from_secs(120),
            keep_connection_underlying_transaction_isolation: false,
            max_use_count: 0,
            default_auto_commit: true,
            default_read_only: None,
            default_transaction_isolation: None,
            default_catalog: None,
            connection_init_sqls: Vec::new(),
            init_variants: false,
            init_global_variants: false,
            pool_prepared_statements: false,
            max_pool_prepared_statements_per_connection: 10,
            share_prepared_statements: false,
            use_oracle_implicit_cache: false,
        }
    }
}

/// `DruidPool` Builder。
pub struct DruidPoolBuilder {
    name: String,
    driver_name: String,
    db_type_name: Option<String>,
    url: Option<String>,
    raw_url: Option<String>,
    connection_properties: std::collections::HashMap<String, String>,
    factory: Option<Arc<dyn crate::core::PhysicalConnectionFactory>>,
    max_open: usize,
    initial_size: usize,
    async_init: bool,
    init_exception_throw: bool,
    min_idle: usize,
    max_idle: usize,
    acquire_timeout: Duration,
    not_full_timeout_retry_count: i32,
    max_wait_thread_count: Option<usize>,
    connection_error_retry_attempts: usize,
    break_after_acquire_failure: bool,
    time_between_connect_error: Duration,
    fail_fast: bool,
    on_fatal_error_max_active: i32,
    max_lifetime: Duration,
    idle_timeout: Duration,
    max_evictable_idle_time: Duration,
    physical_connection_timeout: Option<Duration>,
    test_on_borrow: bool,
    test_on_return: bool,
    test_while_idle: bool,
    time_between_eviction_runs: Duration,
    stat_publish_interval: Duration,
    stat_sink: Arc<dyn DataSourceStatSink>,
    validation_query: Option<String>,
    validation_query_timeout: Duration,
    query_timeout: i32,
    transaction_query_timeout: i32,
    login_timeout: i32,
    valid_connection_checker: Option<Arc<dyn ValidConnectionChecker>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    remove_abandoned: bool,
    remove_abandoned_timeout: Duration,
    log_abandoned: bool,
    keep_alive: bool,
    keep_alive_between_time: Duration,
    keep_connection_underlying_transaction_isolation: bool,
    max_use_count: usize,
    default_auto_commit: bool,
    default_read_only: Option<bool>,
    default_transaction_isolation: Option<u8>,
    default_catalog: Option<String>,
    connection_init_sqls: Vec<String>,
    init_variants: bool,
    init_global_variants: bool,
    pool_prepared_statements: bool,
    max_pool_prepared_statements_per_connection: usize,
    share_prepared_statements: bool,
    use_oracle_implicit_cache: bool,
    filter_chain: FilterChain,
    filter_chain_configured: bool,
    filter_manager: Arc<FilterManager>,
    filter_data_source_name: Arc<RwLock<String>>,
    clear_filters_enable: bool,
    load_spi_filter_skip: bool,
    stats_collector: Arc<StatsCollector>,
    wall_provider: Arc<WallProvider>,
    wall_config_explicit: bool,
}

impl DruidPoolBuilder {
    pub fn new() -> Self {
        let filter_data_source_name = Arc::new(RwLock::new(String::new()));
        let stats_collector = Arc::new(StatsCollector::new(String::new(), Duration::from_secs(2)));
        let wall_provider = Arc::new(WallProvider::default());
        let filter_manager = default_filter_manager(
            Arc::clone(&filter_data_source_name),
            Arc::clone(&stats_collector),
            Arc::clone(&wall_provider),
        );
        Self {
            name: String::new(),
            driver_name: String::new(),
            db_type_name: None,
            url: None,
            raw_url: None,
            connection_properties: std::collections::HashMap::new(),
            factory: None,
            max_open: 8,
            initial_size: 0,
            async_init: false,
            init_exception_throw: true,
            min_idle: 0,
            max_idle: 8,
            acquire_timeout: Duration::MAX,
            not_full_timeout_retry_count: 0,
            max_wait_thread_count: None,
            connection_error_retry_attempts: 1,
            break_after_acquire_failure: false,
            time_between_connect_error: Duration::from_millis(500),
            fail_fast: false,
            on_fatal_error_max_active: 0,
            max_lifetime: Duration::MAX,
            idle_timeout: Duration::from_secs(1800),
            max_evictable_idle_time: Duration::from_secs(7 * 60 * 60),
            physical_connection_timeout: None,
            test_on_borrow: false,
            test_on_return: false,
            test_while_idle: false,
            time_between_eviction_runs: Duration::from_secs(60),
            stat_publish_interval: Duration::ZERO,
            stat_sink: Arc::new(TracingDataSourceStatSink::new()),
            validation_query: None,
            validation_query_timeout: Duration::ZERO,
            query_timeout: 0,
            transaction_query_timeout: 0,
            login_timeout: 0,
            valid_connection_checker: None,
            exception_sorter: None,
            remove_abandoned: false,
            remove_abandoned_timeout: Duration::from_secs(300),
            log_abandoned: false,
            keep_alive: false,
            keep_alive_between_time: Duration::from_secs(120),
            keep_connection_underlying_transaction_isolation: false,
            max_use_count: 0,
            default_auto_commit: true,
            default_read_only: None,
            default_transaction_isolation: None,
            default_catalog: None,
            connection_init_sqls: Vec::new(),
            init_variants: false,
            init_global_variants: false,
            pool_prepared_statements: false,
            max_pool_prepared_statements_per_connection: 10,
            share_prepared_statements: false,
            use_oracle_implicit_cache: false,
            filter_chain: FilterChain::new(),
            filter_chain_configured: false,
            filter_manager,
            filter_data_source_name,
            clear_filters_enable: true,
            load_spi_filter_skip: false,
            stats_collector,
            wall_provider,
            wall_config_explicit: false,
        }
    }

    pub fn name(mut self, v: impl Into<String>) -> Self {
        self.name = v.into();
        self.filter_data_source_name.write().clone_from(&self.name);
        self.wall_provider.set_name(Some(self.name.clone()));
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
        let db_type_name = db_type_name.into();
        if let Some(db_type) = DbType::of(&db_type_name) {
            let config = self
                .wall_config_explicit
                .then(|| self.wall_provider.config().clone());
            if let Ok(provider) = WallFilter::create_provider(
                non_empty_string(&self.name),
                self.url.as_deref(),
                Some(db_type),
                config,
            ) {
                self.wall_provider = provider;
                self.filter_manager = default_filter_manager(
                    Arc::clone(&self.filter_data_source_name),
                    Arc::clone(&self.stats_collector),
                    Arc::clone(&self.wall_provider),
                );
            }
        }
        self.db_type_name = Some(db_type_name);
        self
    }

    /// 设置对外配置 URL。
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// 设置底层驱动 URL；未设置时回退到对外 URL。
    pub fn raw_url(mut self, raw_url: impl Into<String>) -> Self {
        self.raw_url = Some(raw_url.into());
        self
    }

    /// 替换 Wall 规则并重建默认 Filter 工厂注册。
    ///
    /// 对应 Java `WallFilter#configFromProperties` 在数据源初始化前应用规则。
    /// 必须在 `set_filters` 前调用；已显式配置的 `FilterChain` 不回溯替换。
    pub fn wall_config(mut self, wall_config: WallConfig) -> Self {
        self.wall_config_explicit = true;
        self.wall_provider = self
            .db_type_name
            .as_deref()
            .and_then(DbType::of)
            .and_then(|db_type| {
                WallFilter::create_provider(
                    non_empty_string(&self.name),
                    self.url.as_deref(),
                    Some(db_type),
                    Some(wall_config.clone()),
                )
                .ok()
            })
            .unwrap_or_else(|| Arc::new(WallProvider::new(wall_config)));
        self.filter_manager = default_filter_manager(
            Arc::clone(&self.filter_data_source_name),
            Arc::clone(&self.stats_collector),
            Arc::clone(&self.wall_provider),
        );
        self
    }
    pub fn factory(mut self, factory: Arc<dyn crate::core::PhysicalConnectionFactory>) -> Self {
        self.factory = Some(factory);
        self
    }

    /// 设置物理连接创建边界的逻辑驱动属性快照。
    ///
    /// 对应 Java `DruidAbstractDataSource#createPhysicalConnection()` 构造的
    /// `physicalConnectProperties`。属性在数据源构造完成后由所有连接租约共享
    /// 只读快照，避免把连接池配置项误当成驱动属性；具体 Adapter 负责把这些
    /// 属性编码为 URL 参数或驱动选项。
    pub fn connection_properties(
        mut self,
        connection_properties: std::collections::HashMap<String, String>,
    ) -> Self {
        self.connection_properties = connection_properties;
        self
    }

    /// 设置 Java `druid.stat.sql.MaxSize`。
    pub fn max_sql_size(self, max_sql_size: i32) -> Self {
        self.stats_collector.set_max_sql_size(max_sql_size);
        self
    }
    pub fn max_open(mut self, v: usize) -> Self {
        self.max_open = v;
        self
    }
    /// 设置 Java `initialSize`；初始化时预建但不借出这些连接。
    pub fn initial_size(mut self, initial_size: usize) -> Self {
        self.initial_size = initial_size;
        self
    }
    /// 设置是否由受监管 creator 异步创建 initialSize 个连接。
    ///
    /// 对应 Java：`DruidDataSource#setAsyncInit(boolean)`。
    pub fn async_init(mut self, async_init: bool) -> Self {
        self.async_init = async_init;
        self
    }
    /// 设置同步初始化首次建连失败时是否返回错误。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setInitExceptionThrow(boolean)`。
    pub fn init_exception_throw(mut self, init_exception_throw: bool) -> Self {
        self.init_exception_throw = init_exception_throw;
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

    /// 设置池未满时的完整 maxWait 超时重试次数。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setNotFullTimeoutRetryCount(int)`。
    pub fn not_full_timeout_retry_count(mut self, count: i32) -> Self {
        self.not_full_timeout_retry_count = count;
        self
    }

    /// 设置最多允许同时等待连接的任务数。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setMaxWaitThreadCount(int)`。
    /// `None` 表示 Java 默认值 `-1`，即不限制。
    pub fn max_wait_thread_count(mut self, max_wait_thread_count: Option<usize>) -> Self {
        self.max_wait_thread_count = max_wait_thread_count;
        self
    }

    /// 设置一次连接创建周期内的立即重试次数。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setConnectionErrorRetryAttempts(int)`。
    pub fn connection_error_retry_attempts(
        mut self,
        connection_error_retry_attempts: usize,
    ) -> Self {
        self.connection_error_retry_attempts = connection_error_retry_attempts;
        self
    }

    /// 设置达到连接创建重试阈值后是否停止本次获取。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setBreakAfterAcquireFailure(boolean)`。
    pub fn break_after_acquire_failure(mut self, break_after_acquire_failure: bool) -> Self {
        self.break_after_acquire_failure = break_after_acquire_failure;
        self
    }

    /// 设置连续创建失败后的退避时间。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setTimeBetweenConnectErrorMillis(long)`。
    pub fn time_between_connect_error(mut self, time_between_connect_error: Duration) -> Self {
        self.time_between_connect_error = time_between_connect_error;
        self
    }

    /// 设置连续创建失败时是否立即把错误返回等待者。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setFailFast(boolean)`。
    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// 设置 fatal-error 状态允许的最大活动连接数。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setOnFatalErrorMaxActive(int)`。保留 `i32`
    /// 原值；只有正数才启用借用门限。
    pub fn on_fatal_error_max_active(mut self, on_fatal_error_max_active: i32) -> Self {
        self.on_fatal_error_max_active = on_fatal_error_max_active;
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

    /// 设置借出长期空闲连接前是否执行有效性检查。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setTestWhileIdle(boolean)`。
    pub fn test_while_idle(mut self, test_while_idle: bool) -> Self {
        self.test_while_idle = test_while_idle;
        self
    }

    /// 设置后台驱逐周期，也是 `testWhileIdle` 的空闲检查阈值。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setTimeBetweenEvictionRunsMillis(long)`。
    pub fn time_between_eviction_runs(mut self, time_between_eviction_runs: Duration) -> Self {
        self.time_between_eviction_runs = time_between_eviction_runs;
        self
    }

    /// 设置数据源区间统计的发布周期；零表示禁用。
    ///
    /// 对应 Java `timeBetweenLogStatsMillis` 的调度语义，但命名不泄漏 Java
    /// logger 实现。启用后 init 立即发布第一份快照，之后按周期发布。
    pub fn stat_publish_interval(mut self, stat_publish_interval: Duration) -> Self {
        self.stat_publish_interval = stat_publish_interval;
        self
    }

    /// 设置 Rust 原生统计输出端。
    ///
    /// 对应 Java `setStatLogger` 的可替换发布能力，不接受 logger class/name。
    pub fn stat_sink(mut self, stat_sink: Arc<dyn DataSourceStatSink>) -> Self {
        self.stat_sink = stat_sink;
        self
    }

    /// 设置 Java `validationQuery`。
    pub fn validation_query(mut self, validation_query: impl Into<String>) -> Self {
        self.validation_query = Some(validation_query.into());
        self
    }

    /// 设置 Java 秒级 `validationQueryTimeout`。
    pub fn validation_query_timeout(mut self, validation_query_timeout: Duration) -> Self {
        self.validation_query_timeout = validation_query_timeout;
        self
    }

    /// 设置普通 Statement 查询超时秒数。
    pub fn query_timeout(mut self, query_timeout: i32) -> Self {
        self.query_timeout = query_timeout;
        self
    }

    /// 设置事务内 Statement 查询超时秒数。
    pub fn transaction_query_timeout(mut self, transaction_query_timeout: i32) -> Self {
        self.transaction_query_timeout = transaction_query_timeout;
        self
    }

    /// 设置数据源局部登录超时秒数。
    ///
    /// Java 使用进程级 `DriverManager`；Rust 不创建全局 RDBC 状态，而把同一
    /// 可观察配置收敛到数据源及其 `PhysicalConnectionFactory` 边界。
    pub fn login_timeout(mut self, login_timeout: i32) -> Self {
        self.login_timeout = login_timeout;
        self
    }

    /// 设置显式 `ValidConnectionChecker`。
    pub fn valid_connection_checker(
        mut self,
        valid_connection_checker: Arc<dyn ValidConnectionChecker>,
    ) -> Self {
        self.valid_connection_checker = Some(valid_connection_checker);
        self
    }

    /// 设置显式 `ExceptionSorter`，优先于按 `dbType`/driver name 自动推断。
    pub fn exception_sorter(mut self, exception_sorter: Arc<dyn ExceptionSorter>) -> Self {
        self.exception_sorter = Some(exception_sorter);
        self
    }

    /// 设置是否扫描并回收超过阈值且不在执行 SQL 的借出连接。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setRemoveAbandoned(boolean)`。
    pub fn remove_abandoned(mut self, remove_abandoned: bool) -> Self {
        self.remove_abandoned = remove_abandoned;
        self
    }

    /// 设置借出连接判定为 abandoned 的时间阈值。
    ///
    /// 对应 Java：`setRemoveAbandonedTimeoutMillis(long)`；秒级属性由
    /// `DruidDataSourceFactory` 转换后调用本方法。
    pub fn remove_abandoned_timeout(mut self, remove_abandoned_timeout: Duration) -> Self {
        self.remove_abandoned_timeout = remove_abandoned_timeout;
        self
    }

    /// 设置回收 abandoned 连接时是否输出告警。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setLogAbandoned(boolean)`。
    pub fn log_abandoned(mut self, log_abandoned: bool) -> Self {
        self.log_abandoned = log_abandoned;
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

    /// 设置物理连接初始化 SQL。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setConnectionInitSqls`。空白项被忽略，
    /// 非空 SQL 保持输入顺序。
    pub fn connection_init_sqls<I, S>(mut self, connection_init_sqls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.connection_init_sqls = connection_init_sqls
            .into_iter()
            .map(Into::into)
            .map(|sql: String| sql.trim().to_owned())
            .filter(|sql| !sql.is_empty())
            .collect();
        self
    }

    /// 设置是否采集 `MySQL` 会话变量。
    pub fn init_variants(mut self, init_variants: bool) -> Self {
        self.init_variants = init_variants;
        self
    }

    /// 设置是否采集 `MySQL` 全局变量。
    pub fn init_global_variants(mut self, init_global_variants: bool) -> Self {
        self.init_global_variants = init_global_variants;
        self
    }

    /// 启用单物理连接 `PreparedStatement` 缓存。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setPoolPreparedStatements(boolean)`。
    pub fn pool_prepared_statements(mut self, pool_prepared_statements: bool) -> Self {
        self.pool_prepared_statements = pool_prepared_statements;
        self
    }

    /// 设置每个物理连接的 `PreparedStatement` LRU 上限。
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
        self.filter_chain = fc.as_ref().clone();
        self.filter_chain_configured = true;
        self
    }

    /// 设置 Filter 别名与构造工厂管理器。
    ///
    /// 这是 Java ClassLoader/反射机制的 Rust 显式替代入口。默认管理器为
    /// Java `stat/default` 别名注册
    /// [`StatFilter`]；资源中尚未迁移的其他别名仍按 Java 缺失类语义记录后跳过。
    ///
    /// 应在 [`Self::set_filters`] / [`Self::add_filters`] 之前设置；已经构造并
    /// 加入链的 Filter 与 Java 一样不会因之后替换 ClassLoader/工厂而回溯重建。
    pub fn filter_manager(mut self, filter_manager: Arc<FilterManager>) -> Self {
        self.filter_manager = filter_manager;
        self
    }

    /// 设置是否允许 `clearFilters()` 清空当前 Filter。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setClearFiltersEnable(boolean)`。
    pub fn clear_filters_enable(mut self, clear_filters_enable: bool) -> Self {
        self.clear_filters_enable = clear_filters_enable;
        self
    }

    /// 设置是否跳过 inventory 自动 Filter。
    ///
    /// 对应 Java：`DruidDataSource#setLoadSpifilterSkip(boolean)`。
    pub fn load_spi_filter_skip(mut self, load_spi_filter_skip: bool) -> Self {
        self.load_spi_filter_skip = load_spi_filter_skip;
        self
    }

    /// 原位设置是否允许清空 Filter。
    ///
    /// 对应 Java：
    /// `DruidAbstractDataSource#setClearFiltersEnable(boolean)`；该入口便于与
    /// 原位 [`Self::set_filters`] 保持相同调用顺序。
    pub fn set_clear_filters_enable(&mut self, clear_filters_enable: bool) {
        self.clear_filters_enable = clear_filters_enable;
    }

    /// 向当前显式 Filter 应用连接属性与 system properties。
    ///
    /// 对应 Java `setConnectProperties` 后各 Filter 的配置回调，以及
    /// `StatFilter#init` 对 System properties 的第二阶段读取。应在
    /// [`Self::set_filters`] 之后、[`Self::build`] 之前调用。
    ///
    /// # Errors
    ///
    /// 任一 Filter 拒绝配置时停止后续回调并返回原错误。
    pub fn configure_filters(
        &mut self,
        properties: &std::collections::HashMap<String, String>,
        system_properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), crate::core::DruidError> {
        self.filter_chain
            .configure_filters(properties, system_properties)
    }

    /// 设置 Filter 配置。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setFilters(String)`：
    ///
    /// - `None` 和空字符串不改变当前配置；
    /// - 首字符为 `!` 时先移除该字符，再按 `clearFiltersEnable` 决定是否清空；
    /// - 其余内容交给 [`Self::add_filters`]。
    ///
    /// # 参数
    ///
    /// - `filters`：Java 参数 `filters`；`None` 对应 Java `null`。
    ///
    /// # Errors
    ///
    /// Filter 工厂构造失败时，在本方法调用点返回错误，对应 Java
    /// `setFilters` 直接抛出 `SQLException`，而不是推迟到数据源初始化。
    pub fn set_filters(&mut self, filters: Option<&str>) -> Result<(), crate::core::DruidError> {
        let Some(filters) = filters else {
            return Ok(());
        };
        let filters = if let Some(filters) = filters.strip_prefix('!') {
            if self.clear_filters_enable {
                self.filter_chain = FilterChain::new();
                self.filter_chain_configured = true;
            }
            filters
        } else {
            filters
        };
        self.add_filters_in_place(filters)?;
        Ok(())
    }

    /// 追加逗号分隔的 Filter 配置。
    ///
    /// 对应 Java：`DruidAbstractDataSource#addFilters(String)`。每项使用
    /// `String#trim()` 的 U+0000..U+0020 边界，而不是 Rust 会额外删除 Unicode
    /// 空白的 `str::trim()`；重复类名仍由 [`FilterManager`] 忽略大小写去重。
    ///
    /// # Errors
    ///
    /// 任一 Filter 工厂构造失败时立即返回带 Java 消息前缀的错误；此前已经添加的
    /// Filter 保持可见，对应 Java 循环的部分副作用。
    pub fn add_filters(&mut self, filters: Option<&str>) -> Result<(), crate::core::DruidError> {
        if let Some(filters) = filters {
            self.add_filters_in_place(filters)?;
        }
        Ok(())
    }

    /// 清空已配置 Filter。
    ///
    /// 对应 Java：`DruidAbstractDataSource#clearFilters()`。关闭
    /// `clearFiltersEnable` 时保持链不变。
    pub fn clear_filters(&mut self) {
        if self.clear_filters_enable {
            self.filter_chain = FilterChain::new();
            self.filter_chain_configured = true;
        }
    }

    fn add_filters_in_place(&mut self, filters: &str) -> Result<(), crate::core::DruidError> {
        if filters.is_empty() {
            return Ok(());
        }
        self.filter_chain_configured = true;
        for item in filters.split(',') {
            self.filter_manager
                .load_filter(&mut self.filter_chain, trim_rdbc_string(item))?;
        }
        Ok(())
    }

    pub async fn build(mut self) -> Result<super::DruidPool, crate::core::DruidError> {
        if self.max_open == 0 {
            return Err(crate::core::DruidError::InvalidArgument(
                "illegal maxActive 0".to_owned(),
            ));
        }
        if self.max_open < self.min_idle {
            return Err(crate::core::DruidError::InvalidArgument(format!(
                "illegal maxActive {}",
                self.max_open
            )));
        }
        if self.initial_size > self.max_open {
            return Err(crate::core::DruidError::InvalidArgument(format!(
                "illegal initialSize {}, maxActive {}",
                self.initial_size, self.max_open
            )));
        }
        if self.max_evictable_idle_time < self.idle_timeout {
            return Err(crate::core::DruidError::InvalidArgument(
                "maxEvictableIdleTimeMillis must be grater than minEvictableIdleTimeMillis"
                    .to_owned(),
            ));
        }
        if self.keep_alive && self.keep_alive_between_time <= self.time_between_eviction_runs {
            return Err(crate::core::DruidError::InvalidArgument(
                "keepAliveBetweenTimeMillis must be greater than timeBetweenEvictionRunsMillis"
                    .to_owned(),
            ));
        }

        let factory = self
            .factory
            .ok_or(crate::core::DruidError::Other("factory required".into()))?;
        let inner_config = PoolInnerConfig {
            db_type_name: self.db_type_name,
            raw_url: self.raw_url.or_else(|| self.url.clone()),
            url: self.url,
            connection_properties: Arc::new(self.connection_properties),
            max_open: self.max_open,
            initial_size: self.initial_size,
            async_init: self.async_init,
            init_exception_throw: self.init_exception_throw,
            min_idle: self.min_idle,
            max_idle: self.max_idle,
            acquire_timeout: self.acquire_timeout,
            not_full_timeout_retry_count: self.not_full_timeout_retry_count,
            max_wait_thread_count: self.max_wait_thread_count,
            connection_error_retry_attempts: self.connection_error_retry_attempts,
            break_after_acquire_failure: self.break_after_acquire_failure,
            time_between_connect_error: self.time_between_connect_error,
            fail_fast: self.fail_fast,
            on_fatal_error_max_active: self.on_fatal_error_max_active,
            max_lifetime: self.max_lifetime,
            idle_timeout: self.idle_timeout,
            max_evictable_idle_time: self.max_evictable_idle_time,
            physical_connection_timeout: self.physical_connection_timeout,
            test_on_borrow: self.test_on_borrow,
            test_on_return: self.test_on_return,
            test_while_idle: self.test_while_idle,
            time_between_eviction_runs: self.time_between_eviction_runs,
            stat_publish_interval: self.stat_publish_interval,
            stat_sink: self.stat_sink,
            validation_query: self.validation_query,
            validation_query_timeout: self.validation_query_timeout,
            query_timeout: self.query_timeout,
            transaction_query_timeout: self.transaction_query_timeout,
            login_timeout: self.login_timeout,
            valid_connection_checker: self.valid_connection_checker,
            remove_abandoned: self.remove_abandoned,
            remove_abandoned_timeout: self.remove_abandoned_timeout,
            log_abandoned: self.log_abandoned,
            keep_alive: self.keep_alive,
            keep_alive_between_time: self.keep_alive_between_time,
            keep_connection_underlying_transaction_isolation: self
                .keep_connection_underlying_transaction_isolation,
            max_use_count: self.max_use_count,
            default_auto_commit: self.default_auto_commit,
            default_read_only: self.default_read_only,
            default_transaction_isolation: self.default_transaction_isolation,
            default_catalog: self.default_catalog,
            connection_init_sqls: self.connection_init_sqls,
            init_variants: self.init_variants,
            init_global_variants: self.init_global_variants,
            pool_prepared_statements: self.pool_prepared_statements,
            max_pool_prepared_statements_per_connection: self
                .max_pool_prepared_statements_per_connection,
            share_prepared_statements: self.share_prepared_statements,
            use_oracle_implicit_cache: self.use_oracle_implicit_cache,
        };
        // Java 先初始化显式 Filter，再追加 ServiceLoader 自动 Filter；自动
        // provider 不会回溯执行 init，必须保留这一历史时序。
        if !self.filter_chain.is_empty() {
            self.filter_chain.init_filters().await?;
        }
        if !self.load_spi_filter_skip {
            AutoLoad::load_registered(&self.filter_manager, &mut self.filter_chain)?;
        }
        let filter_chain = if self.filter_chain.is_empty() && !self.filter_chain_configured {
            None
        } else {
            Some(Arc::new(self.filter_chain))
        };
        let pool = super::DruidPool::new_with_observability_and_exception_sorter(
            self.name,
            self.driver_name,
            factory,
            inner_config,
            filter_chain,
            self.stats_collector,
            self.wall_provider,
            self.exception_sorter,
        );
        if pool.filter_chain().is_some() {
            pool.mark_filters_initialized();
        }
        Ok(pool)
    }

    /// 构建 canonical `DruidDataSource` 门面。
    ///
    /// 与 [`Self::build`] 使用同一个 native pool 状态机，不增加第二层连接池。
    pub async fn build_data_source(
        self,
    ) -> Result<super::DruidDataSource, crate::core::DruidError> {
        self.build().await.map(super::DruidDataSource::from_pool)
    }
}

impl Default for DruidPoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn default_filter_manager(
    data_source_name: Arc<RwLock<String>>,
    stats_collector: Arc<StatsCollector>,
    wall_provider: Arc<WallProvider>,
) -> Arc<FilterManager> {
    let manager = Arc::new(FilterManager::new());
    let stat_collector = Arc::clone(&stats_collector);
    let merge_stats_collector = Arc::clone(&stats_collector);
    manager.register_filter(STAT_FILTER_CLASS, move || {
        let _data_source_name = data_source_name.read().clone();
        Ok(StatFilter::new(Arc::clone(&stat_collector)))
    });
    manager.register_filter(MERGE_STAT_FILTER_CLASS, move || {
        Ok(MergeStatFilter::new(Arc::clone(&merge_stats_collector)))
    });
    manager.register_filter(WALL_FILTER_CLASS, move || {
        Ok(WallFilter::new(Arc::clone(&wall_provider)))
    });
    manager.register_filter(MYSQL8_DATETIME_FILTER_CLASS, || {
        Ok(MySQL8DateTimeSqlTypeFilter::new())
    });
    manager.register_filter(CONFIG_FILTER_CLASS, || Ok(ConfigFilter::new()));
    manager.register_filter(ENCODING_FILTER_CLASS, || {
        EncodingConvertFilter::new(None, None)
    });
    manager.register_filter(HA_RANDOM_VALIDATE_FILTER_CLASS, || {
        Ok(RandomDataSourceValidateFilter)
    });
    // Rust 原生配置使用 `log`，其唯一实现是 tracing-backed LogFilter。
    // Java Log4j/SLF4J 等类名只出现在迁移说明中，不进入 Rust 运行时注册表。
    manager.register_filter(LOG_FILTER_ID, || Ok(LogFilter::new()));
    manager
}

fn trim_rdbc_string(value: &str) -> &str {
    value.trim_matches(|character| character <= '\u{20}')
}

fn non_empty_string(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}
