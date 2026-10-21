//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource（内部状态）
//!
//! 连接池内部状态：空闲队列、活跃计数、等待通知。

use crate::core::connection_info::ConnectionInfo;
use crate::core::fatal_error_handler::FatalErrorHandler;
use crate::core::Value as RdbcValue;
use crate::core::{
    Connection, ConnectionFactory, ConnectionOpenResult, ConnectionRecycleDisposition,
    ConnectionState, DruidConnectionHolder, DruidError, FilterChain, PreparedStatementCacheStats,
    RdbcError, RdbcString, ValidConnectionCheckerAdapter,
};
use crate::sql::{DbType, RdbcUtils};
use crate::stats::StatsCollector;
use serde_json::{Number as JsonNumber, Value as JsonValue};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::sync::{watch, Notify, Semaphore};

use super::connection_close_worker::ConnectionCloseCommand;

mod create;
mod recycle;
mod state;

/// `DruidAbstractDataSource` fatal-error 字段的同锁状态。
///
/// Java 依赖数据源锁原子更新这些字段；Rust 把它们收拢到同一互斥区，避免多个
/// 原子字段产生 Java 中不存在的组合快照。计数保留 Java `int` 的 wrapping
/// 算术。
#[derive(Default)]
struct FatalErrorState {
    on_fatal_error: bool,
    fatal_error_count: i32,
    fatal_error_count_last_shrink: i32,
    last_fatal_error_at: Option<Instant>,
    last_fatal_error_time_millis: u64,
    last_fatal_error_sql: Option<RdbcString>,
    last_fatal_error: Option<DruidError>,
}

/// 单次 shrink 开始时取得的 fatal-error 快照。
pub(crate) struct FatalShrinkSnapshot {
    on_fatal_error: bool,
    fatal_error_increment: i32,
    last_fatal_error_at: Option<Instant>,
}

/// Java `createError/lastCreateError/failContinuous` 的同锁快照。
#[derive(Default)]
struct CreateFailureState {
    create_started_at: Option<Instant>,
    create_error: Option<DruidError>,
    last_create_error: Option<DruidError>,
    last_create_error_time_millis: u64,
    fail_continuous: bool,
    fail_continuous_time_millis: u64,
}

/// 连接池内部状态。
pub struct PoolInner {
    pub(crate) factory: Arc<dyn ConnectionFactory>,
    /// 包围物理驱动建连的 Druid Filter chain。
    ///
    /// 受监管 creator 只持有 `PoolInner`，因此该链必须与 factory 同属内部
    /// 状态，才能覆盖后台和直接创建两条路径。
    filter_chain: parking_lot::RwLock<Option<Arc<FilterChain>>>,
    pub(crate) config: super::config::PoolInnerConfig,
    pub(crate) idle: parking_lot::Mutex<VecDeque<DruidConnectionHolder>>,
    pub(crate) notify: Notify,
    pub(crate) active_count: AtomicUsize,
    /// 已同步完成丢弃计数、但仍由逻辑租约持有的连接数量。
    pending_owner_discards: AtomicUsize,
    pub(crate) active_peak: AtomicUsize,
    pub(crate) active_peak_time_millis: AtomicU64,
    pub(crate) wait_count: AtomicUsize,
    /// 正在执行物理 factory 创建的任务数，用于 close/restart 排空旧代次。
    pub(crate) creating_count: AtomicUsize,
    creating_idle: Notify,
    pub(crate) total_count: AtomicUsize,
    pub(crate) next_id: AtomicU64,
    pub(crate) user_password_version: AtomicU64,
    /// 每次 restart 递增，用于隔离关闭前已经开始的异步获取/创建任务。
    pub(crate) lifecycle_generation: AtomicU64,
    /// Java `DruidDataSource.enable` 的运行期借用门禁。
    pub(crate) enabled: AtomicBool,
    pub(crate) closed: AtomicBool,
    /// Java `closeTimeMillis`；尚未关闭时沿用 Java 的 `-1` 哨兵所对应的零值。
    pub(crate) close_time_millis: AtomicU64,
    fatal_error_state: parking_lot::Mutex<FatalErrorState>,
    create_failure_state: parking_lot::Mutex<CreateFailureState>,
    // 统计
    pub(crate) create_count: AtomicU64,
    pub(crate) close_count: AtomicU64,
    pub(crate) destroy_count: AtomicU64,
    pub(crate) connect_count: AtomicU64,
    pub(crate) connect_error_count: AtomicU64,
    pub(crate) driver_connect_error_count: AtomicU64,
    pub(crate) pooling_peak: AtomicUsize,
    pub(crate) pooling_peak_time_millis: AtomicU64,
    pub(crate) not_empty_wait_count: AtomicU64,
    pub(crate) not_empty_wait_nanos: AtomicU64,
    pub(crate) recycle_count: AtomicU64,
    pub(crate) recycle_error_count: AtomicU64,
    pub(crate) discard_count: AtomicU64,
    pub(crate) keep_alive_check_count: AtomicU64,
    pub(crate) keep_alive_check_error_count: AtomicU64,
    pub(crate) prepared_statement_stats: Arc<PreparedStatementCacheStats>,
    pub(crate) stats_collector: Arc<StatsCollector>,
    create_sender: parking_lot::RwLock<Option<watch::Sender<Option<usize>>>>,
    close_sender: parking_lot::RwLock<Option<mpsc::Sender<ConnectionCloseCommand>>>,
    pub(crate) close_queue_permits: Arc<Semaphore>,
    pub(crate) close_queue_rejected_total: AtomicU64,
    pub(crate) close_timeout_total: Arc<AtomicU64>,
    pub(crate) close_worker_terminated: Arc<AtomicBool>,
    pub(crate) refill_target: AtomicUsize,
}


impl FatalErrorHandler for PoolInner {
    fn handle_fatal_error(&self, error: &DruidError, sql: Option<&str>) -> bool {
        let mut state = self.fatal_error_state.lock();
        let now = Instant::now();
        state.last_fatal_error_at = Some(now);
        state.last_fatal_error_time_millis = Self::now_millis();
        state.fatal_error_count = state.fatal_error_count.wrapping_add(1);
        let increment = state
            .fatal_error_count
            .wrapping_sub(state.fatal_error_count_last_shrink);
        if increment > self.config.on_fatal_error_max_active {
            // Java 主动推进 lastShrink 一次，避免随后 shrink 重复触发同一轮
            // emptySignal。
            state.fatal_error_count_last_shrink =
                state.fatal_error_count_last_shrink.wrapping_add(1);
            state.on_fatal_error = true;
        } else {
            state.on_fatal_error = false;
        }
        state.last_fatal_error = Some(error.clone());
        state.last_fatal_error_sql = sql.map(|sql| {
            let mut code_units = sql.encode_utf16().collect::<Vec<_>>();
            if code_units.len() > 1024 {
                code_units.truncate(1024);
            }
            RdbcString::from_utf16(code_units)
        });
        state.on_fatal_error
    }

    fn request_fatal_error_refill(&self) {
        // Java `emptySignal()` 请求一次创建，不受 minIdle 是否为零影响。
        self.request_refill(1);
    }
}

/// Java `DruidDataSource#getStatValueAndReset()` 的池级区间快照。
pub(crate) struct PoolStatSnapshot {
    pub(crate) active_count: usize,
    pub(crate) active_peak: usize,
    pub(crate) active_peak_time_millis: u64,
    pub(crate) pooling_count: usize,
    pub(crate) pooling_peak: usize,
    pub(crate) pooling_peak_time_millis: u64,
    pub(crate) connect_count: u64,
    pub(crate) close_count: u64,
    pub(crate) wait_thread_count: usize,
    pub(crate) not_empty_wait_count: u64,
    pub(crate) not_empty_wait_nanos: u64,
    pub(crate) logic_connect_error_count: u64,
    pub(crate) driver_connect_count: u64,
    pub(crate) driver_close_count: u64,
    pub(crate) driver_connect_error_count: u64,
    pub(crate) keep_alive_check_count: u64,
    pub(crate) pstmt_cache_hit_count: u64,
    pub(crate) pstmt_cache_miss_count: u64,
}

/// 物理连接创建期间的容量预留守卫。
///
/// Rust future 可在任意 `.await` 被取消；Java creator 线程则依靠 finally
/// 释放 creatingCount。守卫只有在 holder 完整构造后 commit，其余错误或取消
/// 路径都会恢复 `total_count`。
struct ConnectionSlotReservation<'a> {
    total_count: &'a AtomicUsize,
    committed: bool,
}

/// 统计正在进行的物理创建任务，并在最后一个任务退出时唤醒生命周期等待者。
struct CreatingTaskRegistration<'a> {
    creating_count: &'a AtomicUsize,
    creating_idle: &'a Notify,
}

impl<'a> CreatingTaskRegistration<'a> {
    fn new(creating_count: &'a AtomicUsize, creating_idle: &'a Notify) -> Self {
        creating_count.fetch_add(1, Ordering::AcqRel);
        Self {
            creating_count,
            creating_idle,
        }
    }
}

impl Drop for CreatingTaskRegistration<'_> {
    fn drop(&mut self) {
        if self.creating_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.creating_idle.notify_waiters();
        }
    }
}

impl<'a> ConnectionSlotReservation<'a> {
    fn new(total_count: &'a AtomicUsize) -> Self {
        Self {
            total_count,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for ConnectionSlotReservation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self
                .total_count
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    (current > 0).then_some(current - 1)
                });
        }
    }
}

/// 已从池队列摘出的连接持有者守卫。
///
/// 对应 Java `shrink` 在锁内维护的 `evictConnections` /
/// `keepAliveConnections` 临时数组。Rust 的异步调用可在校验或关闭时被取消，
/// 因此临时所有权必须由 Drop 托底；显式 `take` 表示 holder 已安全回到 idle
/// 队列，`destroy_now` 表示已经计入销毁并等待物理关闭。
struct DetachedHolder<'a> {
    inner: &'a PoolInner,
    holder: Option<DruidConnectionHolder>,
}

impl<'a> DetachedHolder<'a> {
    fn new(inner: &'a PoolInner, holder: DruidConnectionHolder) -> Self {
        Self {
            inner,
            holder: Some(holder),
        }
    }

    fn holder_mut(&mut self) -> &mut DruidConnectionHolder {
        self.holder.as_mut().expect("detached holder is present")
    }

    fn take(&mut self) -> DruidConnectionHolder {
        self.holder.take().expect("detached holder is present")
    }

    async fn destroy_now(&mut self) {
        if let Some(holder) = self.holder.take() {
            self.inner.destroy_holder_now(holder).await;
        }
    }
}

impl Drop for DetachedHolder<'_> {
    fn drop(&mut self) {
        if let Some(holder) = self.holder.take() {
            self.inner.destroy_holder(holder);
        }
    }
}

/// 确保最后一个活跃连接的关闭命令先于 worker shutdown 命令入队。
struct CloseWorkerTerminationGuard<'a> {
    inner: &'a PoolInner,
}

impl Drop for CloseWorkerTerminationGuard<'_> {
    fn drop(&mut self) {
        self.inner.request_close_worker_shutdown_if_idle();
    }
}
