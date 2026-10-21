//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource
//!
//! Druid 风格连接池。

use super::active_connection_lease::ActiveConnectionLease;
use super::config::DruidPoolBuilder;
use super::connection_close_worker::ConnectionCloseWorker;
use super::connection_create_worker::ConnectionCreateWorker;
use super::pool_inner::PoolInner;
use crate::core::fatal_error_handler::FatalErrorHandler;
use crate::core::{
    ConnectionFactory, DataSourceConnectionProvider, Db2ExceptionSorter, DruidConnectionHolder,
    DruidError, DruidPooledConnection, DruidPooledConnectionOwner, ExceptionSorter, FilterChain,
    InformixExceptionSorter, MockExceptionSorter, MsSqlValidConnectionChecker,
    MySqlExceptionSorter, MySqlValidConnectionChecker, OceanBaseOracleExceptionSorter,
    OceanBaseValidConnectionChecker, OracleExceptionSorter, OracleValidConnectionChecker,
    PgExceptionSorter, PgValidConnectionChecker, PhoenixExceptionSorter, PoolState, RdbcError,
    SybaseExceptionSorter, ValidConnectionChecker,
};
use crate::sql::RdbcUrl;
use crate::sql::WallProvider;
use crate::stats::{DruidDataSourceStatValue, StatsCollector};
use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as AsyncMutex, Notify};

mod acquire;
mod construct;
mod state;

/// Druid 风格连接池。
///
/// 对应 Druid Java 的 `DruidDataSource`，实现 `max_open` / `min_idle` /
/// `acquire_timeout` / `FilterChain` 装配 / `DruidPooledConnection::drop` 归还。
pub struct DruidPool {
    name: String,
    driver_name: String,
    inner: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    filters_initialized: AtomicBool,
    initialized: AtomicBool,
    reset_stat_enable: AtomicBool,
    reset_count: AtomicU64,
    lifecycle_lock: AsyncMutex<()>,
    active_leases: Arc<DashMap<u64, ActiveConnectionLease>>,
    remove_abandoned_count: Arc<AtomicU64>,
    maintenance_shutdown: Arc<Notify>,
    maintenance_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
    stat_publish_shutdown: Arc<Notify>,
    stat_publish_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
    create_worker: parking_lot::Mutex<Option<ConnectionCreateWorker>>,
    create_worker_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
    close_worker: parking_lot::Mutex<Option<ConnectionCloseWorker>>,
    close_worker_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
    filter_destroy_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
    stats_collector: Arc<StatsCollector>,
    wall_provider: Arc<WallProvider>,
    statement_id_seed: Arc<AtomicU64>,
    result_set_id_seed: Arc<AtomicU64>,
    metadata_id_seed: Arc<AtomicU64>,
    transaction_id_seed: Arc<AtomicU64>,
    stat_snapshot_context: Arc<DataSourceStatSnapshotContext>,
}

/// 可安全移入 Tokio 任务的数据源区间快照上下文。
///
/// 只持有生产统计所需的共享对象，不持有 `DruidPool` 自引用，也不复制连接池。
struct DataSourceStatSnapshotContext {
    name: String,
    driver_name: String,
    inner: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    stats_collector: Arc<StatsCollector>,
}

impl DataSourceStatSnapshotContext {
    fn snapshot_and_reset(&self) -> DruidDataSourceStatValue {
        let pool = self.inner.stat_snapshot_and_reset();
        let runtime = self.stats_collector.runtime_snapshot_and_reset();
        let sql_list = self
            .stats_collector
            .sql_merger
            .all_stats()
            .into_iter()
            .map(|stat| stat.stat_value())
            .collect();
        self.stats_collector.sql_merger.reset();
        let transaction = runtime.transaction_histogram;

        DruidDataSourceStatValue {
            name: self.name.clone(),
            db_type: self.inner.config.db_type_name.clone(),
            driver_class_name: self.driver_name.clone(),
            // 统计 sink 可写入日志或远端系统，不能发布带凭据的原始 URL。
            url: self
                .inner
                .factory
                .connection_url()
                .map(redacted_diagnostic_url),
            user_name: self.inner.factory.user_name().map(str::to_owned),
            filter_class_names: self
                .filter_chain
                .as_ref()
                .map_or_else(Vec::new, |chain| chain.filter_class_names().to_vec()),
            remove_abandoned: self.inner.config.remove_abandoned,
            initial_size: self.inner.config.initial_size,
            min_idle: self.inner.config.min_idle,
            max_active: self.inner.config.max_open,
            query_timeout: self.inner.config.query_timeout,
            transaction_query_timeout: self.inner.config.transaction_query_timeout,
            login_timeout: self.inner.config.login_timeout,
            valid_connection_checker_class_name: self
                .inner
                .config
                .valid_connection_checker
                .as_ref()
                .map(|checker| checker.class_name().to_owned()),
            exception_sorter_class_name: self
                .exception_sorter
                .as_ref()
                .map(|sorter| sorter.class_name().to_owned()),
            test_on_borrow: self.inner.config.test_on_borrow,
            test_on_return: self.inner.config.test_on_return,
            test_while_idle: self.inner.config.test_while_idle,
            default_auto_commit: self.inner.config.default_auto_commit,
            default_read_only: self.inner.config.default_read_only.unwrap_or(false),
            default_transaction_isolation: self.inner.config.default_transaction_isolation,
            active_count: pool.active_count,
            active_peak: pool.active_peak,
            active_peak_time: (pool.active_peak_time_millis > 0)
                .then_some(pool.active_peak_time_millis),
            pooling_count: pool.pooling_count,
            pooling_peak: pool.pooling_peak,
            pooling_peak_time: (pool.pooling_peak_time_millis > 0)
                .then_some(pool.pooling_peak_time_millis),
            connect_count: pool.connect_count,
            close_count: pool.close_count,
            wait_thread_count: pool.wait_thread_count,
            not_empty_wait_count: pool.not_empty_wait_count,
            not_empty_wait_nanos: pool.not_empty_wait_nanos,
            logic_connect_error_count: pool.logic_connect_error_count,
            driver_connect_count: pool.driver_connect_count,
            driver_close_count: pool.driver_close_count,
            driver_connect_error_count: pool.driver_connect_error_count,
            execute_count: runtime.execute_count,
            error_count: runtime.error_count,
            commit_count: runtime.commit_count,
            rollback_count: runtime.rollback_count,
            pstmt_cache_hit_count: pool.pstmt_cache_hit_count,
            pstmt_cache_miss_count: pool.pstmt_cache_miss_count,
            start_transaction_count: runtime.start_transaction_count,
            keep_alive_check_count: pool.keep_alive_check_count,
            connection_hold_time_histogram: runtime.connection_hold_time_histogram,
            txn_0_1: transaction[0],
            txn_1_10: transaction[1],
            txn_10_100: transaction[2],
            txn_100_1000: transaction[3],
            txn_1000_10000: transaction[4],
            txn_10000_100000: transaction[5],
            txn_more: transaction[6],
            clob_open_count: runtime.clob_open_count,
            blob_open_count: runtime.blob_open_count,
            sql_skip_count: runtime.sql_skip_count,
            sql_list,
        }
    }
}

/// 仅为诊断输出保留无查询参数的规范 RDBC 地址；其他驱动地址整体遮蔽。
pub(super) fn redacted_diagnostic_url(url: &str) -> String {
    RdbcUrl::parse(url).map_or_else(
        |_| "<redacted connection URL>".to_owned(),
        |parsed| parsed.redacted(),
    )
}


/// idle holder 借出/校验期间的取消守卫。
///
/// 在异步 validation 完成并转为 active 前，future 被取消会立即丢弃候选 holder，
/// 归还容量并异步关闭物理连接，避免它既不在 idle 队列也不在 active 租约中。
struct BorrowCandidate {
    inner: Arc<PoolInner>,
    holder: Option<DruidConnectionHolder>,
}

/// 等待连接任务计数守卫。
///
/// 对应 Java `notEmptyWaitThreadCount`。Rust future 被取消时 Drop 必须减计数，
/// 否则 `maxWaitThreadCount` 会永久拒绝后续请求。
struct WaitTaskRegistration {
    inner: Arc<PoolInner>,
    started_at: Instant,
}

impl WaitTaskRegistration {
    fn register(inner: Arc<PoolInner>) -> Result<Self, DruidError> {
        if let Some(max) = inner.config.max_wait_thread_count.filter(|max| *max > 0) {
            let result =
                inner
                    .wait_count
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                        (current < max).then_some(current + 1)
                    });
            if let Err(current) = result {
                return Err(DruidError::MaxWaitThreadCountExceeded { max, current });
            }
        } else {
            inner.wait_count.fetch_add(1, Ordering::AcqRel);
        }
        Ok(Self {
            inner,
            started_at: Instant::now(),
        })
    }
}

impl Drop for WaitTaskRegistration {
    fn drop(&mut self) {
        self.inner.record_not_empty_wait(self.started_at.elapsed());
        let _ =
            self.inner
                .wait_count
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    (current > 0).then_some(current - 1)
                });
    }
}

impl BorrowCandidate {
    fn new(inner: Arc<PoolInner>, holder: DruidConnectionHolder) -> Self {
        Self {
            inner,
            holder: Some(holder),
        }
    }

    fn holder_mut(&mut self) -> &mut DruidConnectionHolder {
        self.holder
            .as_mut()
            .expect("borrow candidate holder is present")
    }

    fn take(&mut self) -> DruidConnectionHolder {
        self.holder
            .take()
            .expect("borrow candidate holder is present")
    }
}

impl Drop for BorrowCandidate {
    fn drop(&mut self) {
        if let Some(holder) = self.holder.take() {
            self.inner.destroy_holder(holder);
        }
    }
}

fn remove_abandoned_leases(
    inner: &PoolInner,
    active_leases: &DashMap<u64, ActiveConnectionLease>,
    remove_abandoned_count: &AtomicU64,
) -> usize {
    if !inner.config.remove_abandoned {
        return 0;
    }

    let mut stale_ids = Vec::new();
    let mut abandoned_ids = Vec::new();
    for entry in active_leases.iter() {
        let connection_id = *entry.key();
        let lease = entry.value();
        let Some(lease_active) = lease.lease_active.upgrade() else {
            stale_ids.push(connection_id);
            continue;
        };
        if !lease_active.load(Ordering::Acquire) {
            stale_ids.push(connection_id);
            continue;
        }
        if lease
            .execution_running
            .upgrade()
            .is_some_and(|running| running.load(Ordering::Acquire))
        {
            continue;
        }
        if lease.borrowed_at.elapsed() >= inner.config.remove_abandoned_timeout
            && lease_active
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            abandoned_ids.push(connection_id);
        }
    }

    for connection_id in stale_ids {
        active_leases.remove(&connection_id);
    }
    for connection_id in &abandoned_ids {
        active_leases.remove(connection_id);
        if inner.config.log_abandoned {
            tracing::warn!(
                connection_id,
                timeout_ms = inner.config.remove_abandoned_timeout.as_millis(),
                "remove abandoned pooled connection lease"
            );
        }
    }
    remove_abandoned_count.fetch_add(abandoned_ids.len() as u64, Ordering::Relaxed);
    abandoned_ids.len()
}

#[async_trait::async_trait]
impl DataSourceConnectionProvider for DruidPool {
    fn data_source_name(&self) -> &str {
        self.name()
    }

    fn data_source_state(&self) -> PoolState {
        self.state()
    }

    async fn get_connection_direct_for_filter(
        &self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection_direct(max_wait).await
    }
}

#[async_trait::async_trait]
impl crate::core::Pool for DruidPool {
    async fn get(&self) -> Result<DruidPooledConnection, RdbcError> {
        DruidPool::get(self).await.map_err(RdbcError::from)
    }

    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, RdbcError> {
        DruidPool::get_timeout(self, timeout)
            .await
            .map_err(RdbcError::from)
    }

    fn state(&self) -> PoolState {
        DruidPool::state(self)
    }

    fn driver_name(&self) -> &str {
        DruidPool::driver_name(self)
    }

    fn name(&self) -> &str {
        DruidPool::name(self)
    }
}

#[cfg(test)]
mod lifetime_tests;
