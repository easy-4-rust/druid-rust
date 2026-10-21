use super::*;

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
    pub fn factory(mut self, factory: Arc<dyn crate::core::ConnectionFactory>) -> Self {
        self.factory = Some(factory);
        self
    }

    /// 设置物理连接创建边界的逻辑驱动属性快照。
    ///
    /// 对应 Java `DruidAbstractDataSource#createConnection()` 构造的
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

    /// 设置物理关闭队列容量；零容量按最小容量一处理。
    #[must_use]
    pub fn close_queue_capacity(mut self, capacity: usize) -> Self {
        self.close_queue_capacity = capacity.max(1);
        self
    }

    /// 设置单条物理连接的关闭截止时间。
    #[must_use]
    pub fn close_timeout(mut self, timeout: Duration) -> Self {
        self.close_timeout = timeout;
        self
    }

    /// 设置池关闭时等待后台任务的截止时间。
    #[must_use]
    pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
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
    /// 设置 Rust 扩展的 driver connection 最大生命周期。
    ///
    /// Java Druid 默认没有这一额外限制，因此默认值为 `Duration::MAX`。
    /// Java `phyTimeoutMillis` 应使用 [`Self::driver_connection_timeout`]。
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

    /// 设置 driver connection 绝对寿命。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setPhyTimeoutMillis(long)`。
    /// Java 默认 `-1` 表示禁用，因此 Rust 使用 `Option<Duration>` 保存配置。
    pub fn driver_connection_timeout(mut self, driver_connection_timeout: Duration) -> Self {
        self.driver_connection_timeout = Some(driver_connection_timeout);
        self
    }

    /// Task 6 兼容入口；新代码应使用 [`Self::driver_connection_timeout`]。
    #[deprecated(
        since = "0.0.0-design",
        note = "use driver_connection_timeout; this alias is removed in the next breaking release"
    )]
    pub fn physical_connection_timeout(self, physical_connection_timeout: Duration) -> Self {
        self.driver_connection_timeout(physical_connection_timeout)
    }

    /// Java `phyTimeoutMillis` 的 Task 6 兼容入口。
    #[deprecated(
        since = "0.0.0-design",
        note = "use driver_connection_timeout; this alias is removed in the next breaking release"
    )]
    pub fn phy_timeout(self, phy_timeout: Duration) -> Self {
        self.driver_connection_timeout(phy_timeout)
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
    /// 可观察配置收敛到数据源及其 `ConnectionFactory` 边界。
    pub fn login_timeout(mut self, login_timeout: i32) -> Self {
        self.login_timeout = login_timeout;
        self
    }
}
