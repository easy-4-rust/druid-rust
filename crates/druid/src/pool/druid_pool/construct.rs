use super::*;

impl DruidPool {
    /// 从 canonical standard factory 创建 Druid native pool。
    pub fn new_standard(
        name: String,
        driver_name: String,
        factory: Arc<dyn druid_rdbc::driver::ConnectionFactory>,
        config: super::super::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Self {
        Self::new(name, driver_name, factory, config, filter_chain)
    }

    pub fn new(
        name: String,
        driver_name: String,
        factory: Arc<dyn ConnectionFactory>,
        config: super::super::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Self {
        Self::new_with_observability(
            name,
            driver_name,
            factory,
            config,
            filter_chain,
            Arc::new(StatsCollector::default()),
            Arc::new(WallProvider::default()),
        )
    }

    /// 创建具有共享 Stat/Wall 管理对象的池。
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_observability(
        name: String,
        driver_name: String,
        factory: Arc<dyn ConnectionFactory>,
        config: super::super::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Arc<StatsCollector>,
        wall_provider: Arc<WallProvider>,
    ) -> Self {
        Self::new_with_observability_and_exception_sorter(
            name,
            driver_name,
            factory,
            config,
            filter_chain,
            stats_collector,
            wall_provider,
            None,
        )
    }

    /// 创建具有共享 Stat/Wall 对象及显式异常分类器的池。
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_observability_and_exception_sorter(
        name: String,
        driver_name: String,
        factory: Arc<dyn ConnectionFactory>,
        mut config: super::super::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Arc<StatsCollector>,
        wall_provider: Arc<WallProvider>,
        exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    ) -> Self {
        let exception_sorter = exception_sorter.or_else(|| {
            Self::select_exception_sorter(config.db_type_name.as_deref(), &driver_name)
        });
        if config.valid_connection_checker.is_none() {
            config.valid_connection_checker =
                Self::select_valid_connection_checker(config.db_type_name.as_deref(), &driver_name);
        }
        let close_factory = Arc::clone(&factory);
        let inner = Arc::new(PoolInner::new_with_stats(
            factory,
            config,
            Arc::clone(&stats_collector),
        ));
        inner.install_filter_chain(filter_chain.clone());
        let (close_sender, close_receiver) =
            tokio::sync::mpsc::channel(inner.config.close_queue_capacity.max(1));
        inner.install_close_sender(close_sender);
        let (create_sender, create_receiver) = tokio::sync::watch::channel(Some(0));
        inner.install_create_sender(create_sender);
        let stat_snapshot_context = Arc::new(DataSourceStatSnapshotContext {
            name: name.clone(),
            driver_name: driver_name.clone(),
            inner: Arc::clone(&inner),
            filter_chain: filter_chain.clone(),
            exception_sorter: exception_sorter.clone(),
            stats_collector: Arc::clone(&stats_collector),
        });
        Self {
            name,
            driver_name,
            inner: Arc::clone(&inner),
            filter_chain: filter_chain.clone(),
            exception_sorter,
            filters_initialized: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            reset_stat_enable: AtomicBool::new(true),
            reset_count: AtomicU64::new(0),
            lifecycle_lock: AsyncMutex::new(()),
            active_leases: Arc::new(DashMap::new()),
            remove_abandoned_count: Arc::new(AtomicU64::new(0)),
            maintenance_shutdown: Arc::new(Notify::new()),
            maintenance_task: AsyncMutex::new(None),
            stat_publish_shutdown: Arc::new(Notify::new()),
            stat_publish_task: AsyncMutex::new(None),
            create_worker: parking_lot::Mutex::new(Some(ConnectionCreateWorker::new(
                Arc::clone(&inner),
                create_receiver,
            ))),
            create_worker_task: AsyncMutex::new(None),
            close_worker: parking_lot::Mutex::new(Some(ConnectionCloseWorker::new(
                close_factory,
                filter_chain.clone(),
                close_receiver,
                inner.config.close_timeout,
                Arc::clone(&inner.close_timeout_total),
                Arc::clone(&inner.close_worker_terminated),
            ))),
            close_worker_task: AsyncMutex::new(None),
            filter_destroy_task: AsyncMutex::new(None),
            stats_collector,
            wall_provider,
            statement_id_seed: Arc::new(AtomicU64::new(20_000)),
            result_set_id_seed: Arc::new(AtomicU64::new(50_000)),
            metadata_id_seed: Arc::new(AtomicU64::new(80_000)),
            transaction_id_seed: Arc::new(AtomicU64::new(60_000)),
            stat_snapshot_context,
        }
    }

    pub(crate) fn select_valid_connection_checker(
        db_type_name: Option<&str>,
        driver_name: &str,
    ) -> Option<Arc<dyn ValidConnectionChecker>> {
        let identity = db_type_name.unwrap_or(driver_name).to_ascii_lowercase();
        if identity.contains("oceanbase") {
            if identity.contains("mysql") {
                Some(Arc::new(OceanBaseValidConnectionChecker::mysql_mode()))
            } else {
                Some(Arc::new(OceanBaseValidConnectionChecker::new()))
            }
        } else if identity.contains("oracle") {
            Some(Arc::new(OracleValidConnectionChecker::new()))
        } else if identity.contains("mysql") || identity.contains("mariadb") {
            Some(Arc::new(MySqlValidConnectionChecker::new()))
        } else if identity.contains("postgres") || identity == "pg" {
            Some(Arc::new(PgValidConnectionChecker))
        } else if identity.contains("sqlserver") || identity.contains("mssql") {
            Some(Arc::new(MsSqlValidConnectionChecker))
        } else {
            None
        }
    }

    pub(crate) fn select_exception_sorter(
        db_type_name: Option<&str>,
        driver_name: &str,
    ) -> Option<Arc<dyn ExceptionSorter>> {
        // Java Druid 按实际驱动类选择 Mock sorter，即使 dbTypeName 被设为 mysql。
        // Rust 的 driver_name 是这里可用的驱动身份；先处理 Mock 再回退到数据库类型。
        if driver_name.eq_ignore_ascii_case("mock")
            || driver_name.eq_ignore_ascii_case("com.alibaba.druid.mock.MockDriver")
        {
            return Some(Arc::new(MockExceptionSorter));
        }
        let identity = db_type_name.unwrap_or(driver_name).to_ascii_lowercase();
        if identity.contains("oceanbase") && identity.contains("oracle") {
            Some(Arc::new(OceanBaseOracleExceptionSorter::new()))
        } else if identity.contains("oracle") {
            Some(Arc::new(OracleExceptionSorter::new()))
        } else if identity.contains("mysql") || identity.contains("mariadb") {
            Some(Arc::new(MySqlExceptionSorter))
        } else if identity.contains("postgres") || identity == "pg" {
            Some(Arc::new(PgExceptionSorter))
        } else if identity.contains("phoenix") {
            Some(Arc::new(PhoenixExceptionSorter))
        } else if identity.contains("informix") {
            Some(Arc::new(InformixExceptionSorter))
        } else if identity.contains("sybase") {
            Some(Arc::new(SybaseExceptionSorter))
        } else if identity.contains("db2") {
            Some(Arc::new(Db2ExceptionSorter))
        } else if identity.contains("mock") {
            Some(Arc::new(MockExceptionSorter))
        } else {
            None
        }
    }

    pub fn builder() -> DruidPoolBuilder {
        DruidPoolBuilder::new()
    }

    pub async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection().await
    }

    /// 获取池化连接。
    ///
    /// 对应 Java：`DruidDataSource#getConnection()`。保留 `get()` 作为 Rust
    /// `Pool` 习惯入口，但 canonical Java 迁移名称不能缺失。
    pub async fn get_connection(&self) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection_with_max_wait(self.inner.config.acquire_timeout)
            .await
    }

    /// 获取标准 pooled owner；owner 关闭时才将其 holder 归还 native pool。
    pub async fn get_pooled_connection_owner(
        &self,
    ) -> Result<DruidPooledConnectionOwner, DruidError> {
        self.get_connection()
            .await
            .map(DruidPooledConnectionOwner::new)
    }

    /// 使用本次 maxWait 获取池化连接。
    ///
    /// 对应 Java：`DruidDataSource#getConnection(long)`。
    pub async fn get_connection_with_max_wait(
        &self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        match &self.filter_chain {
            Some(filter_chain) if !filter_chain.is_empty() => {
                filter_chain
                    .data_source_get_connection(self, max_wait)
                    .await
            }
            _ => self.get_connection_direct(max_wait).await,
        }
    }

    /// 绕过数据源获取 Filter，直接进入 native pool 状态机。
    ///
    /// 对应 Java：`DruidDataSource#getConnectionDirect(long)`。物理驱动建连
    /// Filter 仍在 `PoolInner` 内执行；绕过的只是 `dataSource_getConnection`
    /// 这一外层 hook。
    pub async fn get_connection_direct(
        &self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.get_with_timeout(max_wait).await
    }

}
