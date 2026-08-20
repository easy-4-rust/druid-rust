//! 对应 Java 类：`com.alibaba.druid.pool.ha.HighAvailableDataSource`。

use super::data_source_creator::DataSourceCreator;
use super::node::{FileNodeListener, NodeListener, PoolUpdater};
use super::selector::{DataSourceSelector, DataSourceSelectorFactory};
use crate::core::{DruidError, DruidPooledConnection, Pool, PoolState};
use dashmap::{DashMap, DashSet};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct HighAvailableDataSourceConfig {
    pub(crate) driver_class_name: Option<String>,
    pub(crate) connect_properties: HashMap<String, String>,
    pub(crate) connection_properties: Option<String>,
    pub(crate) initial_size: usize,
    pub(crate) max_active: usize,
    pub(crate) min_idle: usize,
    pub(crate) max_wait: Duration,
    pub(crate) validation_query: Option<String>,
    pub(crate) validation_query_timeout: Duration,
    pub(crate) test_on_borrow: bool,
    pub(crate) test_on_return: bool,
    pub(crate) test_while_idle: bool,
    pub(crate) pool_prepared_statements: bool,
    pub(crate) share_prepared_statements: bool,
    pub(crate) max_pool_prepared_statement_per_connection_size: usize,
    pub(crate) query_timeout: i32,
    pub(crate) transaction_query_timeout: i32,
    pub(crate) time_between_eviction_runs: Duration,
    pub(crate) min_evictable_idle_time: Duration,
    pub(crate) max_evictable_idle_time: Duration,
    pub(crate) physical_timeout: Option<Duration>,
    pub(crate) time_between_connect_error: Duration,
    pub(crate) remove_abandoned: bool,
    pub(crate) remove_abandoned_timeout: Duration,
    pub(crate) log_abandoned: bool,
    pub(crate) filters: Option<String>,
    pub(crate) data_source_file: PathBuf,
    pub(crate) property_prefix: String,
    pub(crate) pool_purge_interval_seconds: i32,
    pub(crate) allow_empty_pool_when_update: bool,
}

impl Default for HighAvailableDataSourceConfig {
    fn default() -> Self {
        Self {
            driver_class_name: None,
            connect_properties: HashMap::new(),
            connection_properties: None,
            initial_size: 0,
            max_active: 8,
            min_idle: 0,
            max_wait: Duration::MAX,
            validation_query: None,
            validation_query_timeout: Duration::ZERO,
            test_on_borrow: false,
            test_on_return: false,
            test_while_idle: true,
            pool_prepared_statements: false,
            share_prepared_statements: false,
            max_pool_prepared_statement_per_connection_size: 10,
            query_timeout: 0,
            transaction_query_timeout: 0,
            time_between_eviction_runs: Duration::from_secs(60),
            min_evictable_idle_time: Duration::from_secs(30 * 60),
            max_evictable_idle_time: Duration::from_secs(7 * 60 * 60),
            physical_timeout: None,
            time_between_connect_error: Duration::from_millis(500),
            remove_abandoned: false,
            remove_abandoned_timeout: Duration::from_secs(300),
            log_abandoned: false,
            filters: None,
            data_source_file: PathBuf::from("ha-datasource.properties"),
            property_prefix: String::new(),
            pool_purge_interval_seconds: PoolUpdater::DEFAULT_INTERVAL,
            allow_empty_pool_when_update: false,
        }
    }
}

/// HA 数据源共享状态。
pub(crate) struct HighAvailableDataSourceInner {
    pub(crate) data_sources: DashMap<String, Arc<dyn Pool>>,
    pub(crate) blacklist: DashSet<String>,
    pub(crate) selector: RwLock<Option<Arc<dyn DataSourceSelector>>>,
    pub(crate) test_on_borrow: AtomicBool,
    pub(crate) test_on_return: AtomicBool,
    pub(crate) initialized: AtomicBool,
    pub(crate) init_lock: tokio::sync::Mutex<()>,
    pub(crate) config: RwLock<HighAvailableDataSourceConfig>,
    pub(crate) data_source_creator: DataSourceCreator,
    pub(crate) pool_updater: RwLock<Option<Arc<PoolUpdater>>>,
    pub(crate) node_listener: RwLock<Option<Arc<dyn NodeListener>>>,
}

impl HighAvailableDataSourceInner {
    pub(crate) fn data_source_map(&self) -> HashMap<String, Arc<dyn Pool>> {
        self.data_sources
            .iter()
            .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
            .collect()
    }

    pub(crate) fn available_data_source_map(&self) -> HashMap<String, Arc<dyn Pool>> {
        self.data_sources
            .iter()
            .filter(|entry| !self.blacklist.contains(entry.key()))
            .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
            .collect()
    }
}

/// 包含多个 Pool 并通过 Druid selector 选择节点的高可用数据源。
#[derive(Clone)]
pub struct HighAvailableDataSource {
    name: String,
    driver_name: String,
    inner: Arc<HighAvailableDataSourceInner>,
}

impl HighAvailableDataSource {
    /// 创建空 HA 数据源；首次 init 默认安装 `random` 选择器。
    ///
    /// `data_source_creator` 负责根据 URL 创建物理连接工厂；具体驱动由
    /// `druid-wrapper` 注入，Core 不绑定。
    #[must_use]
    pub fn new(name: impl Into<String>, data_source_creator: DataSourceCreator) -> Self {
        Self {
            name: name.into(),
            driver_name: "druid-ha".to_owned(),
            inner: Arc::new(HighAvailableDataSourceInner {
                data_sources: DashMap::new(),
                blacklist: DashSet::new(),
                selector: RwLock::new(None),
                test_on_borrow: AtomicBool::new(false),
                test_on_return: AtomicBool::new(false),
                initialized: AtomicBool::new(false),
                init_lock: tokio::sync::Mutex::new(()),
                config: RwLock::new(HighAvailableDataSourceConfig::default()),
                data_source_creator,
                pool_updater: RwLock::new(None),
                node_listener: RwLock::new(None),
            }),
        }
    }

    pub(crate) fn weak_inner(&self) -> Weak<HighAvailableDataSourceInner> {
        Arc::downgrade(&self.inner)
    }

    /// 幂等初始化节点更新器、节点监听器和默认随机选择器。
    pub async fn init(&self) -> Result<(), DruidError> {
        if self.inner.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let _guard = self.inner.init_lock.lock().await;
        if self.inner.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        if self.inner.data_sources.is_empty() {
            let config = self.inner.config.read().clone();
            let updater = Arc::new(PoolUpdater::new(
                self.weak_inner(),
                DataSourceCreator::new(self.inner.data_source_creator.clone_factory_creator()),
            ));
            updater.set_interval_seconds(config.pool_purge_interval_seconds);
            updater.set_allow_empty_pool(config.allow_empty_pool_when_update);
            updater.init().await;
            *self.inner.pool_updater.write() = Some(Arc::clone(&updater));

            let listener = self.inner.node_listener.read().clone().unwrap_or_else(|| {
                let listener = Arc::new(FileNodeListener::new(config.data_source_file));
                listener.set_prefix(config.property_prefix);
                listener as Arc<dyn NodeListener>
            });
            listener.set_observer(updater);
            Arc::clone(&listener).init().await?;
            listener.update().await;
            *self.inner.node_listener.write() = Some(listener);
        }
        if self.inner.selector.read().is_none() {
            self.set_selector("random");
        }
        if self.inner.data_sources.is_empty() {
            tracing::warn!("没有可用的 HA 数据源，请检查节点配置");
        }
        self.inner.initialized.store(true, Ordering::Release);
        Ok(())
    }

    /// 销毁监听器、更新器、selector 和所有支持生命周期关闭的子池。
    pub async fn destroy(&self) {
        let listener = self.inner.node_listener.read().clone();
        if let Some(listener) = listener {
            listener.destroy().await;
        }
        let updater = self.inner.pool_updater.read().clone();
        if let Some(updater) = updater {
            updater.destroy().await;
        }
        if let Some(selector) = self.inner.selector.read().as_ref() {
            selector.destroy();
        }
        let data_sources: Vec<Arc<dyn Pool>> = self
            .inner
            .data_sources
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect();
        for data_source in data_sources {
            data_source.close_pool().await;
        }
    }

    /// 添加或替换命名数据源。
    pub fn insert_data_source(&self, name: impl Into<String>, data_source: Arc<dyn Pool>) {
        self.inner.data_sources.insert(name.into(), data_source);
    }

    /// 删除命名数据源。
    pub fn remove_data_source(&self, name: &str) -> Option<Arc<dyn Pool>> {
        self.inner
            .data_sources
            .remove(name)
            .map(|(_, data_source)| data_source)
    }

    /// 原子替换完整数据源 map。
    pub fn set_data_source_map(&self, data_sources: HashMap<String, Arc<dyn Pool>>) {
        self.inner.data_sources.clear();
        for (name, data_source) in data_sources {
            self.inner.data_sources.insert(name, data_source);
        }
    }

    /// 返回完整数据源 map 快照。
    #[must_use]
    pub fn data_source_map(&self) -> HashMap<String, Arc<dyn Pool>> {
        self.inner.data_source_map()
    }

    /// 返回排除 HA 外层 blacklist 后的数据源 map。
    #[must_use]
    pub fn available_data_source_map(&self) -> HashMap<String, Arc<dyn Pool>> {
        self.inner.available_data_source_map()
    }

    /// 仅在名称存在时加入外层 blacklist。
    pub fn add_blacklist(&self, name: &str) {
        if self.inner.data_sources.contains_key(name) {
            self.inner.blacklist.insert(name.to_owned());
        }
    }

    /// 从外层 blacklist 删除名称。
    pub fn remove_blacklist(&self, name: &str) {
        self.inner.blacklist.remove(name);
    }

    /// 判断名称是否在外层 blacklist。
    #[must_use]
    pub fn is_in_blacklist(&self, name: &str) -> bool {
        self.inner.blacklist.contains(name)
    }

    /// 按内置名称创建、初始化并替换 selector；未知名称无副作用。
    pub fn set_selector(&self, name: &str) {
        let Some(selector) = DataSourceSelectorFactory::get_selector(name, self) else {
            return;
        };
        selector.init();
        if let Some(old) = self.inner.selector.write().replace(selector) {
            old.destroy();
        }
    }

    /// 直接替换 selector。
    pub fn set_data_source_selector(&self, selector: Arc<dyn DataSourceSelector>) {
        if let Some(old) = self.inner.selector.write().replace(selector) {
            old.destroy();
        }
    }

    /// 返回当前 selector 名称。
    #[must_use]
    pub fn selector_name(&self) -> Option<&'static str> {
        self.inner
            .selector
            .read()
            .as_ref()
            .map(|selector| selector.name())
    }

    /// 设置命名 selector 的当前执行上下文目标。
    pub fn set_target_data_source(&self, target_name: Option<String>) {
        if let Some(selector) = self.inner.selector.read().as_ref() {
            selector.set_target(target_name);
        }
    }

    /// 选择节点并获取连接；无节点时保留 Java 的 null 语义为 `Ok(None)`。
    pub async fn get_connection(&self) -> Result<Option<DruidPooledConnection>, DruidError> {
        self.init().await?;
        let selector = self.inner.selector.read().clone();
        let Some(data_source) = selector.and_then(|selector| selector.get()) else {
            return Ok(None);
        };
        data_source.get().await.map(Some)
    }

    /// 返回 testOnBorrow。
    #[must_use]
    pub fn is_test_on_borrow(&self) -> bool {
        self.inner.test_on_borrow.load(Ordering::Acquire)
    }

    /// 设置 testOnBorrow。
    pub fn set_test_on_borrow(&self, value: bool) {
        self.inner.test_on_borrow.store(value, Ordering::Release);
        self.inner.config.write().test_on_borrow = value;
    }

    /// 返回 testOnReturn。
    #[must_use]
    pub fn is_test_on_return(&self) -> bool {
        self.inner.test_on_return.load(Ordering::Acquire)
    }

    /// 设置 testOnReturn。
    pub fn set_test_on_return(&self, value: bool) {
        self.inner.test_on_return.store(value, Ordering::Release);
        self.inner.config.write().test_on_return = value;
    }

    /// 设置节点监听器；初始化时会注入当前 PoolUpdater。
    pub fn set_node_listener(&self, listener: Arc<dyn NodeListener>) {
        *self.inner.node_listener.write() = Some(listener);
    }

    /// 返回节点监听器。
    #[must_use]
    pub fn node_listener(&self) -> Option<Arc<dyn NodeListener>> {
        self.inner.node_listener.read().clone()
    }

    /// 设置节点 properties 文件路径。
    pub fn set_data_source_file(&self, file: impl Into<PathBuf>) {
        self.inner.config.write().data_source_file = file.into();
    }

    /// 设置 properties 节点键前缀。
    pub fn set_property_prefix(&self, prefix: impl Into<String>) {
        self.inner.config.write().property_prefix = prefix.into();
    }

    /// 设置延迟摘除扫描周期。
    pub fn set_pool_purge_interval_seconds(&self, interval_seconds: i32) {
        self.inner.config.write().pool_purge_interval_seconds = interval_seconds;
    }

    /// 设置动态更新时是否允许摘除最后一个可用节点。
    pub fn set_allow_empty_pool_when_update(&self, allow: bool) {
        self.inner.config.write().allow_empty_pool_when_update = allow;
    }

    /// 设置 Java 驱动类名，仅保留为兼容元数据；Rust 建连由 Adapter/Factory 决定。
    pub fn set_driver_class_name(&self, driver_class_name: Option<String>) {
        self.inner.config.write().driver_class_name = driver_class_name;
    }

    /// 合并物理连接属性；与 Java `putAll` 一致，不清空既有键。
    pub fn set_connect_properties(&self, properties: Option<HashMap<String, String>>) {
        if let Some(properties) = properties {
            self.inner
                .config
                .write()
                .connect_properties
                .extend(properties);
        }
    }

    /// 解析分号分隔的 Java `connectionProperties` 并合并到连接属性。
    pub fn set_connection_properties(&self, connection_properties: Option<&str>) {
        let mut config = self.inner.config.write();
        config.connection_properties = connection_properties.map(ToOwned::to_owned);
        let Some(connection_properties) = connection_properties else {
            config.connect_properties.clear();
            return;
        };
        if connection_properties.trim().is_empty() {
            config.connect_properties.clear();
            return;
        }
        let properties = connection_properties
            .split(';')
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                entry.split_once('=').map_or_else(
                    || (entry.to_owned(), String::new()),
                    |(name, value)| (name.to_owned(), value.to_owned()),
                )
            });
        config.connect_properties.extend(properties);
    }

    /// 设置子池 initialSize。
    pub fn set_initial_size(&self, initial_size: usize) {
        self.inner.config.write().initial_size = initial_size;
    }

    /// 设置子池 maxActive。
    pub fn set_max_active(&self, max_active: usize) {
        self.inner.config.write().max_active = max_active;
    }

    /// 设置子池 minIdle。
    pub fn set_min_idle(&self, min_idle: usize) {
        self.inner.config.write().min_idle = min_idle;
    }

    /// 设置子池 maxWait。
    pub fn set_max_wait(&self, max_wait: Duration) {
        self.inner.config.write().max_wait = max_wait;
    }

    /// 设置连接校验 SQL。
    pub fn set_validation_query(&self, validation_query: Option<String>) {
        self.inner.config.write().validation_query = validation_query;
    }

    /// 设置连接校验超时。
    pub fn set_validation_query_timeout(&self, timeout: Duration) {
        self.inner.config.write().validation_query_timeout = timeout;
    }

    /// 设置 testWhileIdle。
    pub fn set_test_while_idle(&self, value: bool) {
        self.inner.config.write().test_while_idle = value;
    }

    /// 设置是否缓存 PreparedStatement。
    pub fn set_pool_prepared_statements(&self, value: bool) {
        self.inner.config.write().pool_prepared_statements = value;
    }

    /// 设置是否跨逻辑连接共享 PreparedStatement。
    pub fn set_share_prepared_statements(&self, value: bool) {
        self.inner.config.write().share_prepared_statements = value;
    }

    /// 设置每物理连接 PreparedStatement 缓存上限。
    pub fn set_max_pool_prepared_statement_per_connection_size(&self, value: usize) {
        self.inner
            .config
            .write()
            .max_pool_prepared_statement_per_connection_size = value;
    }

    /// 设置普通查询超时秒数。
    pub fn set_query_timeout(&self, value: i32) {
        self.inner.config.write().query_timeout = value;
    }

    /// 设置事务查询超时秒数。
    pub fn set_transaction_query_timeout(&self, value: i32) {
        self.inner.config.write().transaction_query_timeout = value;
    }

    /// 设置空闲检查周期。
    pub fn set_time_between_eviction_runs(&self, value: Duration) {
        self.inner.config.write().time_between_eviction_runs = value;
    }

    /// 设置最小空闲驱逐时间。
    pub fn set_min_evictable_idle_time(&self, value: Duration) {
        self.inner.config.write().min_evictable_idle_time = value;
    }

    /// 设置最大空闲驱逐时间。
    pub fn set_max_evictable_idle_time(&self, value: Duration) {
        self.inner.config.write().max_evictable_idle_time = value;
    }

    /// 设置物理连接最大生命周期；`None` 对应 Java `-1`。
    pub fn set_physical_timeout(&self, value: Option<Duration>) {
        self.inner.config.write().physical_timeout = value;
    }

    /// 设置连续建连错误重试间隔。
    pub fn set_time_between_connect_error(&self, value: Duration) {
        self.inner.config.write().time_between_connect_error = value;
    }

    /// 设置连接泄漏回收开关。
    pub fn set_remove_abandoned(&self, value: bool) {
        self.inner.config.write().remove_abandoned = value;
    }

    /// 设置连接泄漏超时。
    pub fn set_remove_abandoned_timeout(&self, value: Duration) {
        self.inner.config.write().remove_abandoned_timeout = value;
    }

    /// 设置泄漏诊断记录开关；输出仍使用 Rust `tracing`。
    pub fn set_log_abandoned(&self, value: bool) {
        self.inner.config.write().log_abandoned = value;
    }

    /// 设置 Java Filter 别名字符串。
    pub fn set_filters(&self, filters: Option<String>) {
        self.inner.config.write().filters = filters;
    }
}

#[async_trait::async_trait]
impl Pool for HighAvailableDataSource {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection()
            .await?
            .ok_or(DruidError::DataSourceNotAvailable { cause: None })
    }

    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        self.init().await?;
        let selector = self.inner.selector.read().clone();
        let data_source = selector
            .and_then(|selector| selector.get())
            .ok_or(DruidError::DataSourceNotAvailable { cause: None })?;
        data_source.get_timeout(timeout).await
    }

    async fn close_pool(&self) {
        self.destroy().await;
    }

    fn state(&self) -> PoolState {
        let mut state = self
            .inner
            .selector
            .read()
            .as_ref()
            .and_then(|selector| selector.get())
            .map_or_else(PoolState::default, |data_source| data_source.state());
        state.name.clone_from(&self.name);
        state
    }

    fn driver_name(&self) -> &str {
        &self.driver_name
    }

    fn name(&self) -> &str {
        &self.name
    }
}
