use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::RwLock;

use crate::core::{DruidError, DruidPooledConnection, PhysicalConnectionFactory, Pool, PoolState};
use crate::stats::{DataSourceMonitorable, DruidDataSourceStatManager, DruidDataSourceStatValue};
use serde_json::json;
use std::sync::Arc;

use super::managed_data_source::ManagedDataSource;
use super::{DataSourceProxy, DruidPool};

/// Druid 的 canonical 数据源门面。
///
/// 对应 Java: `com.alibaba.druid.pool.DruidDataSource`。底层复用已经实现 native
/// acquire/recycle/shrink 状态机的 [`DruidPool`]，并补齐 Java
/// `ManagedDataSource` 的 enable 与 objectName 管理语义。该门面不是第二层池，
/// 不会形成 pool-in-pool。
pub struct DruidDataSource {
    pool: DruidPool,
    object_name: RwLock<Option<String>>,
    data_source_id: OnceLock<u64>,
}

impl DruidDataSource {
    /// 将 native pool 提升为 canonical `DruidDataSource`。
    #[must_use]
    pub fn from_pool(pool: DruidPool) -> Self {
        Self {
            pool,
            object_name: RwLock::new(None),
            data_source_id: OnceLock::new(),
        }
    }

    /// 获取连接；禁用时返回 Java `DataSourceDisableException` 对应错误。
    pub async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.pool.get().await
    }

    /// 幂等初始化数据源并预建 `initialSize` 个连接。
    pub async fn init(&self) -> Result<(), DruidError> {
        self.pool.init().await
    }

    /// 在指定超时内获取连接；禁用检查先于等待。
    pub async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.pool.get_timeout(timeout).await
    }

    /// 返回连接池状态快照。
    #[must_use]
    pub fn state(&self) -> PoolState {
        self.pool.state()
    }

    /// 返回数据源当前是否处于 Java `onFatalError` 状态。
    #[must_use]
    pub fn is_on_fatal_error(&self) -> bool {
        self.pool.is_on_fatal_error()
    }

    /// 返回 fatal-error 活动连接门限。
    #[must_use]
    pub fn on_fatal_error_max_active(&self) -> i32 {
        self.pool.on_fatal_error_max_active()
    }

    /// 返回是否异步创建 initialSize 个连接。
    #[must_use]
    pub fn is_async_init(&self) -> bool {
        self.pool.is_async_init()
    }

    /// 返回同步初始化建连失败时是否抛错。
    #[must_use]
    pub fn is_init_exception_throw(&self) -> bool {
        self.pool.is_init_exception_throw()
    }

    /// 返回数据源是否处于连续物理创建失败状态。
    #[must_use]
    pub fn is_fail_continuous(&self) -> bool {
        self.pool.is_fail_continuous()
    }

    /// 返回最近一次物理创建错误。
    #[must_use]
    pub fn last_create_error(&self) -> Option<DruidError> {
        self.pool.last_create_error()
    }

    /// 返回最近一次物理创建错误时间。
    #[must_use]
    pub fn last_create_error_time_millis(&self) -> u64 {
        self.pool.last_create_error_time_millis()
    }

    /// 取得并重置数据源区间统计快照。
    #[must_use]
    pub fn stat_value_and_reset(&self) -> DruidDataSourceStatValue {
        self.pool.stat_value_and_reset()
    }

    /// 立即发布一份区间统计快照。
    pub fn publish_stats(&self) -> Result<(), DruidError> {
        self.pool.publish_stats()
    }

    /// 执行默认空闲连接收缩。
    pub async fn shrink(&self) {
        self.pool.shrink().await;
    }

    /// 按数据源 keepAlive 配置执行指定 checkTime 的收缩。
    pub async fn shrink_check_time(&self, check_time: bool) {
        self.pool.shrink_check_time(check_time).await;
    }

    /// 按时间和保活选项执行空闲连接收缩。
    pub async fn shrink_with_options(&self, check_time: bool, keep_alive: bool) {
        self.pool.shrink_with_options(check_time, keep_alive).await;
    }

    /// 扫描并失效超时的借出连接租约，返回本轮处理数量。
    ///
    /// 对应 Java：`DruidDataSource#removeAbandoned()`。
    pub fn remove_abandoned(&self) -> usize {
        self.pool.remove_abandoned()
    }

    /// 强制丢弃指定池化连接；`None` 对应 Java null 并返回 false。
    pub fn discard_connection(&self, connection: Option<&mut DruidPooledConnection>) -> bool {
        self.pool.discard_connection(connection)
    }

    /// 将池内物理连接总数填充到 `maxActive`。
    pub async fn fill(&self) -> Result<usize, DruidError> {
        self.pool.fill().await
    }

    /// 将池内物理连接总数填充到指定数量。
    pub async fn fill_to(&self, to_count: i32) -> Result<usize, DruidError> {
        self.pool.fill_to(to_count).await
    }

    /// 仅在当前已有 pooling connection 时尝试获取，不触发 init 或建连。
    pub async fn try_get_connection(&self) -> Result<Option<DruidPooledConnection>, DruidError> {
        self.pool.try_get_connection().await
    }

    /// 返回 active + pooling 是否已经达到 maxActive。
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.pool.is_full()
    }

    /// 通知数据源：外部物理连接 factory 的 URL/用户名/密码已经更新。
    ///
    /// 池不会保存新密码；调用方须先更新 factory，再调用本方法完成旧连接替换。
    pub async fn notify_credentials_changed(&self) -> Result<u64, DruidError> {
        self.pool.notify_credentials_changed().await
    }

    /// 返回当前数据源凭据版本。
    #[must_use]
    pub fn user_password_version(&self) -> u64 {
        self.pool.user_password_version()
    }

    /// 关闭数据源及全部空闲物理连接。
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// 关闭旧代次并恢复为尚未初始化的数据源。
    ///
    /// 活动连接不为零时返回
    /// [`DruidError::ActiveConnectionsPreventRestart`]；成功后 enable 恢复为
    /// `true`，下一次 `init/get/fill` 才创建新代次连接和后台任务。
    pub async fn restart(&self) -> Result<(), DruidError> {
        self.pool.restart().await
    }

    /// 返回数据源是否已经完成至少一次初始化且尚未 restart。
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.pool.is_initialized()
    }

    /// 返回数据源是否已经关闭。
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.pool.is_closed()
    }

    /// 返回底层 native pool；仅用于管理与兼容层，不改变所有权。
    #[must_use]
    pub fn native_pool(&self) -> &DruidPool {
        &self.pool
    }

    /// 将此数据源显式注册到进程级 Druid 管理协议并返回稳定 ID。
    ///
    /// Java 通过 JMX/弱引用在 `init()` 中注册；Rust 需要调用者持有 `Arc`，
    /// 因而把所有权要求显式放到类型签名中。
    pub fn register_monitoring(self: &Arc<Self>) -> u64 {
        *self.data_source_id.get_or_init(|| {
            let monitorable: Arc<dyn DataSourceMonitorable> = Arc::clone(self) as Arc<_>;
            DruidDataSourceStatManager::global().register(monitorable)
        })
    }
}

impl DataSourceProxy for DruidDataSource {
    fn data_source_stat(&self) -> &Arc<crate::stats::StatsCollector> {
        self.pool.stats_collector()
    }

    fn data_source_id(&self) -> u64 {
        self.data_source_id.get().copied().unwrap_or(0)
    }

    fn name(&self) -> &str {
        self.pool.name()
    }

    fn db_type(&self) -> Option<&str> {
        self.pool.db_type_name()
    }

    fn raw_driver(&self) -> &dyn PhysicalConnectionFactory {
        self.pool.raw_driver()
    }

    fn url(&self) -> Option<&str> {
        self.pool.url()
    }

    fn raw_jdbc_url(&self) -> Option<&str> {
        self.pool.raw_url()
    }

    fn proxy_filter_names(&self) -> Vec<String> {
        self.pool.filter_class_names()
    }

    fn create_connection_id(&self) -> u64 {
        self.pool.create_connection_id()
    }

    fn create_statement_id(&self) -> u64 {
        self.pool.create_statement_id()
    }

    fn create_result_set_id(&self) -> u64 {
        self.pool.create_result_set_id()
    }

    fn create_metadata_id(&self) -> u64 {
        self.pool.create_metadata_id()
    }

    fn create_transaction_id(&self) -> u64 {
        self.pool.create_transaction_id()
    }

    fn connect_properties(&self) -> &std::collections::HashMap<String, String> {
        self.pool.connect_properties()
    }
}

impl DataSourceMonitorable for DruidDataSource {
    fn name(&self) -> &str {
        self.pool.name()
    }

    fn data_source_stat_data(&self) -> serde_json::Value {
        let state = self.state();
        json!({
            "Name": state.name,
            "URL": state.url,
            "DriverClassName": state.driver_name,
            "MaxActive": state.max_open,
            "ActiveCount": state.active_count,
            "ActivePeak": state.active_peak,
            "ActivePeakTime": state.active_peak_time_millis,
            "PoolingCount": state.idle_count,
            "PoolingPeak": state.pooling_peak,
            "PoolingPeakTime": state.pooling_peak_time_millis,
            "WaitThreadCount": state.wait_count,
            "NotEmptyWaitCount": state.not_empty_wait_count,
            "NotEmptyWaitNanos": state.not_empty_wait_nanos,
            "NotEmptyWaitMillis": state.not_empty_wait_nanos / 1_000_000,
            "MaxWaitThreadCount": state.max_wait_thread_count
                .and_then(|value| i64::try_from(value).ok())
                .unwrap_or(-1),
            "CreateCount": state.create_count,
            "DestroyCount": state.destroy_count,
            "ConnectCount": state.connect_count,
            "CloseCount": state.close_count,
            "ConnectErrorCount": state.connect_error_count,
            "CreateErrorCount": state.physical_connect_error_count,
            "RecycleCount": state.recycle_count,
            "RecycleErrorCount": state.recycle_error_count,
            "DiscardCount": state.discard_count,
            "RemoveAbandonedCount": state.leak_detection_count,
            "KeepAliveCheckCount": state.keep_alive_check_count,
            "KeepAliveCheckErrorCount": state.keep_alive_check_error_count,
            "PreparedStatementCount": state.prepared_statement_count,
            "ClosedPreparedStatementCount": state.closed_prepared_statement_count,
            "CachedPreparedStatementCount": state.cached_prepared_statement_count,
            "CachedPreparedStatementHitCount": state.cached_prepared_statement_hit_count,
            "CachedPreparedStatementMissCount": state.cached_prepared_statement_miss_count,
            "Closed": state.closed,
        })
    }

    fn sql_stat_data(&self) -> Vec<serde_json::Value> {
        self.pool
            .stats_collector()
            .sql_merger
            .all_stats()
            .into_iter()
            // Java facade 不返回尚未完成且当前也未运行的预创建 SQL 条目。
            .filter(|stat| {
                stat.execute_count() != 0
                    || stat
                        .running_count
                        .load(std::sync::atomic::Ordering::Acquire)
                        != 0
            })
            .filter_map(|stat| serde_json::to_value(stat.stat_value()).ok())
            .collect()
    }

    fn wall_stat_data(&self) -> serde_json::Value {
        serde_json::Value::Object(self.pool.wall_provider().stats_map())
    }

    fn pooling_connection_info(&self) -> Vec<serde_json::Value> {
        self.pool.pooling_connection_info()
    }

    fn active_connection_stack_trace(&self) -> Vec<String> {
        self.pool.active_connection_stack_trace()
    }

    fn reset_stat(&self) {
        self.pool.reset_stats();
    }
}

impl ManagedDataSource for DruidDataSource {
    fn is_enable(&self) -> bool {
        self.pool.is_enabled()
    }

    fn set_enable(&self, value: bool) {
        self.pool.set_enabled(value);
    }

    fn object_name(&self) -> Option<String> {
        self.object_name.read().clone()
    }

    fn set_object_name(&self, object_name: Option<String>) {
        *self.object_name.write() = object_name;
    }
}

#[async_trait::async_trait]
impl Pool for DruidDataSource {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        DruidDataSource::get(self).await
    }

    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        DruidDataSource::get_timeout(self, timeout).await
    }

    fn state(&self) -> PoolState {
        DruidDataSource::state(self)
    }

    fn driver_name(&self) -> &str {
        self.pool.driver_name()
    }

    fn name(&self) -> &str {
        self.pool.name()
    }
}
