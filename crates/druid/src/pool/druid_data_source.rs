use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::RwLock;

use crate::core::{DruidError, DruidPooledConnection, Pool, PoolState};
use crate::stats::{DataSourceMonitorable, DruidDataSourceStatManager};
use serde_json::json;
use std::sync::Arc;

use super::managed_data_source::ManagedDataSource;
use super::DruidPool;

/// Druid 的 canonical 数据源门面。
///
/// 对应 Java: `com.alibaba.druid.pool.DruidDataSource`。底层复用已经实现 native
/// acquire/recycle/shrink 状态机的 [`DruidPool`]，并补齐 Java
/// `ManagedDataSource` 的 enable 与 objectName 管理语义。该门面不是第二层池，
/// 不会形成 pool-in-pool。
pub struct DruidDataSource {
    pool: DruidPool,
    enable: AtomicBool,
    object_name: RwLock<Option<String>>,
}

impl DruidDataSource {
    /// 将 native pool 提升为 canonical `DruidDataSource`。
    #[must_use]
    pub fn from_pool(pool: DruidPool) -> Self {
        Self {
            pool,
            enable: AtomicBool::new(true),
            object_name: RwLock::new(None),
        }
    }

    /// 获取连接；禁用时返回 Java `DataSourceDisableException` 对应错误。
    pub async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.ensure_enabled()?;
        self.pool.get().await
    }

    /// 幂等初始化数据源并预建 `initialSize` 个连接。
    pub async fn init(&self) -> Result<(), DruidError> {
        self.ensure_enabled()?;
        self.pool.init().await
    }

    /// 在指定超时内获取连接；禁用检查先于等待。
    pub async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.ensure_enabled()?;
        self.pool.get_timeout(timeout).await
    }

    /// 返回连接池状态快照。
    #[must_use]
    pub fn state(&self) -> PoolState {
        self.pool.state()
    }

    /// 执行默认空闲连接收缩。
    pub async fn shrink(&self) {
        self.pool.shrink().await;
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

    /// 将池内物理连接总数填充到 `minIdle`。
    pub async fn fill(&self) -> Result<usize, DruidError> {
        self.ensure_enabled()?;
        self.pool.fill().await
    }

    /// 将池内物理连接总数填充到指定数量。
    pub async fn fill_to(&self, to_count: usize) -> Result<usize, DruidError> {
        self.ensure_enabled()?;
        self.pool.fill_to(to_count).await
    }

    /// 关闭数据源及全部空闲物理连接。
    pub async fn close(&self) {
        self.pool.close().await;
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
        let monitorable: Arc<dyn DataSourceMonitorable> = Arc::clone(self) as Arc<_>;
        DruidDataSourceStatManager::global().register(monitorable)
    }

    fn ensure_enabled(&self) -> Result<(), DruidError> {
        if self.is_enable() {
            Ok(())
        } else {
            Err(DruidError::DataSourceDisabled)
        }
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
            "PoolingCount": state.idle_count,
            "WaitThreadCount": state.wait_count,
            "MaxWaitThreadCount": state.max_wait_thread_count
                .and_then(|value| i64::try_from(value).ok())
                .unwrap_or(-1),
            "CreateCount": state.create_count,
            "DestroyCount": state.destroy_count,
            "ConnectCount": state.connect_count,
            "CloseCount": state.close_count,
            "ConnectErrorCount": state.connect_error_count,
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
            .filter_map(|stat| {
                let mut value = serde_json::to_value(stat.stat_value()).ok()?;
                value
                    .as_object_mut()?
                    .insert("DataSource".to_owned(), self.pool.name().into());
                Some(value)
            })
            .collect()
    }

    fn wall_stat_data(&self) -> serde_json::Value {
        let provider = self.pool.wall_provider();
        let white_list = provider
            .white_list_values(false)
            .into_iter()
            .map(|value| serde_json::Value::Object(value.to_map()))
            .collect::<Vec<_>>();
        let black_list = provider
            .black_list_values(false)
            .into_iter()
            .map(|value| serde_json::Value::Object(value.to_map()))
            .collect::<Vec<_>>();
        let tables = provider
            .table_stat_values(false)
            .into_iter()
            .map(|value| serde_json::Value::Object(value.to_map()))
            .collect::<Vec<_>>();
        let functions = provider
            .function_stat_values(false)
            .into_iter()
            .map(|value| serde_json::Value::Object(value.to_map()))
            .collect::<Vec<_>>();
        json!({
            "checkCount": provider.check_count(),
            "hardCheckCount": provider.hard_check_count(),
            "violationCount": provider.violation_count(),
            "violationEffectRowCount": provider.violation_effect_row_count(),
            "blackListHitCount": provider.black_list_hit_count(),
            "blackListSize": provider.black_list_size(),
            "whiteListHitCount": provider.white_list_hit_count(),
            "whiteListSize": provider.white_list_size(),
            "syntaxErrorCount": provider.syntax_error_count(),
            "tables": tables,
            "functions": functions,
            "blackList": black_list,
            "whiteList": white_list,
        })
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
        self.enable.load(Ordering::Acquire)
    }

    fn set_enable(&self, value: bool) {
        self.enable.store(value, Ordering::Release);
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
