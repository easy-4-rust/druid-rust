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

mod builder;
mod builder_advanced;

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
    /// 物理关闭命令的硬队列容量；不包括 worker 当前处理的连接。
    pub close_queue_capacity: usize,
    /// 单条物理连接允许的最大关闭时间。
    pub close_timeout: Duration,
    /// 池关闭时等待受监管后台任务的最大时间。
    pub shutdown_timeout: Duration,
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
    pub driver_connection_timeout: Option<Duration>,
    /// `driver_connection_timeout` 的 Task 6 兼容字段。
    #[deprecated(
        since = "0.0.0-design",
        note = "use driver_connection_timeout; this alias is removed in the next breaking release"
    )]
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

impl PoolInnerConfig {
    /// Resolves the canonical timeout, accepting a directly populated compatibility field.
    #[allow(deprecated)]
    pub(crate) fn effective_driver_connection_timeout(&self) -> Option<Duration> {
        self.driver_connection_timeout
            .or(self.physical_connection_timeout)
    }
}

#[allow(deprecated)]
impl Default for PoolInnerConfig {
    fn default() -> Self {
        Self {
            db_type_name: None,
            url: None,
            raw_url: None,
            connection_properties: Arc::new(std::collections::HashMap::new()),
            max_open: 8,
            close_queue_capacity: 64,
            close_timeout: Duration::from_secs(5),
            shutdown_timeout: Duration::from_secs(15),
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
            driver_connection_timeout: None,
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
    factory: Option<Arc<dyn crate::core::ConnectionFactory>>,
    max_open: usize,
    close_queue_capacity: usize,
    close_timeout: Duration,
    shutdown_timeout: Duration,
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
    driver_connection_timeout: Option<Duration>,
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
