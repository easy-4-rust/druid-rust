use super::*;

impl DruidPoolBuilder {
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

    pub(crate) fn add_filters_in_place(&mut self, filters: &str) -> Result<(), crate::core::DruidError> {
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

    #[allow(deprecated)]
    pub async fn build(mut self) -> Result<super::super::DruidPool, crate::core::DruidError> {
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
            close_queue_capacity: self.close_queue_capacity,
            close_timeout: self.close_timeout,
            shutdown_timeout: self.shutdown_timeout,
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
            driver_connection_timeout: self.driver_connection_timeout,
            physical_connection_timeout: self.driver_connection_timeout,
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
        let pool = super::super::DruidPool::new_with_observability_and_exception_sorter(
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
    ) -> Result<super::super::DruidDataSource, crate::core::DruidError> {
        self.build().await.map(super::super::DruidDataSource::from_pool)
    }
}
