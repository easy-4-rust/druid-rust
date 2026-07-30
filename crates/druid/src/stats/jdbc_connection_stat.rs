use dashmap::DashMap;
use parking_lot::RwLock;
use serde::Serialize;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 单个物理连接的管理统计条目。
///
/// 对应 Java：`JdbcConnectionStat.Entry`。Java 异常对象/栈被映射为可安全跨线程
/// 暴露的文本；JMX CompositeData 不进入 Rust 对象模型。
pub struct JdbcConnectionStatEntry {
    id: u64,
    data_source: String,
    establish_time_millis: AtomicU64,
    establish_started_at: RwLock<Option<Instant>>,
    connect_time_millis: AtomicU64,
    connect_timespan_nanos: AtomicU64,
    connect_stack_trace: RwLock<Option<String>>,
    last_sql: RwLock<Option<String>>,
    last_statement_stack_trace: RwLock<Option<String>>,
    last_error: RwLock<Option<String>>,
    last_error_time_millis: AtomicU64,
}

impl JdbcConnectionStatEntry {
    /// 创建指定数据源与连接 ID 的条目。
    #[must_use]
    pub fn new(data_source: impl Into<String>, connection_id: u64) -> Self {
        Self {
            id: connection_id,
            data_source: data_source.into(),
            establish_time_millis: AtomicU64::new(0),
            establish_started_at: RwLock::new(None),
            connect_time_millis: AtomicU64::new(0),
            connect_timespan_nanos: AtomicU64::new(0),
            connect_stack_trace: RwLock::new(None),
            last_sql: RwLock::new(None),
            last_statement_stack_trace: RwLock::new(None),
            last_error: RwLock::new(None),
            last_error_time_millis: AtomicU64::new(0),
        }
    }

    /// 标记连接建立完成时间。
    pub fn mark_established(&self) {
        self.establish_time_millis
            .store(now_millis(), Ordering::Release);
        *self.establish_started_at.write() = Some(Instant::now());
    }

    pub fn set_connect_time_millis(&self, value: u64) {
        self.connect_time_millis.store(value, Ordering::Release);
    }

    pub fn set_connect_timespan_nanos(&self, value: u64) {
        self.connect_timespan_nanos.store(value, Ordering::Release);
    }

    pub fn set_last_sql(&self, sql: Option<String>) {
        *self.last_sql.write() = sql;
    }

    pub fn set_connect_stack_trace(&self, stack_trace: Option<String>) {
        *self.connect_stack_trace.write() = stack_trace;
    }

    pub fn set_last_statement_stack_trace(&self, stack_trace: Option<String>) {
        *self.last_statement_stack_trace.write() = stack_trace;
    }

    pub fn error(&self, error: impl ToString) {
        *self.last_error.write() = Some(error.to_string());
        self.last_error_time_millis
            .store(now_millis(), Ordering::Release);
    }

    /// 清除 Java `Entry#reset()` 对应的易变诊断字段。
    pub fn reset(&self) {
        *self.last_sql.write() = None;
        *self.last_statement_stack_trace.write() = None;
        *self.last_error.write() = None;
        self.last_error_time_millis.store(0, Ordering::Release);
    }

    /// 返回当前管理快照。
    #[must_use]
    pub fn snapshot(&self) -> JdbcConnectionStatEntryValue {
        JdbcConnectionStatEntryValue {
            id: self.id,
            connect_time_millis: non_zero(self.connect_time_millis.load(Ordering::Acquire)),
            connect_timespan_millis: self.connect_timespan_nanos.load(Ordering::Acquire)
                / 1_000_000,
            establish_time_millis: non_zero(self.establish_time_millis.load(Ordering::Acquire)),
            alive_timespan_millis: self
                .establish_started_at
                .read()
                .as_ref()
                .map_or(0, |started_at| {
                    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
                }),
            last_sql: self.last_sql.read().clone(),
            last_error: self.last_error.read().clone(),
            last_error_time_millis: non_zero(self.last_error_time_millis.load(Ordering::Acquire)),
            connect_stack_trace: self.connect_stack_trace.read().clone(),
            last_statement_stack_trace: self.last_statement_stack_trace.read().clone(),
            data_source: self.data_source.clone(),
        }
    }
}

/// `JdbcConnectionStat.Entry` 的不可变管理快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct JdbcConnectionStatEntryValue {
    #[serde(rename = "ID")]
    pub id: u64,
    pub connect_time_millis: Option<u64>,
    pub connect_timespan_millis: u64,
    pub establish_time_millis: Option<u64>,
    pub alive_timespan_millis: u64,
    pub last_sql: Option<String>,
    pub last_error: Option<String>,
    pub last_error_time_millis: Option<u64>,
    pub connect_stack_trace: Option<String>,
    pub last_statement_stack_trace: Option<String>,
    pub data_source: String,
}

/// Connection 层运行统计。
///
/// 对应 Java：`com.alibaba.druid.stat.JdbcConnectionStat`。
pub struct JdbcConnectionStat {
    active_count: AtomicI32,
    active_count_max: AtomicI32,
    connecting_count: AtomicI32,
    connecting_max: AtomicI32,
    connect_count: AtomicU64,
    connect_error_count: AtomicU64,
    connect_error_last: RwLock<Option<String>>,
    connect_nano_total: AtomicU64,
    connect_nano_max: AtomicU64,
    error_count: AtomicU64,
    alive_nano_total: AtomicU64,
    last_error: RwLock<Option<String>>,
    last_error_time_millis: AtomicU64,
    connect_last_time_millis: AtomicU64,
    close_count: AtomicU64,
    transaction_start_count: AtomicU64,
    commit_count: AtomicU64,
    rollback_count: AtomicU64,
    alive_nano_min: AtomicU64,
    alive_nano_max: AtomicU64,
    histogram: [AtomicU64; 7],
    connections: DashMap<u64, Arc<JdbcConnectionStatEntry>>,
}

impl JdbcConnectionStat {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_count: AtomicI32::new(0),
            active_count_max: AtomicI32::new(0),
            connecting_count: AtomicI32::new(0),
            connecting_max: AtomicI32::new(0),
            connect_count: AtomicU64::new(0),
            connect_error_count: AtomicU64::new(0),
            connect_error_last: RwLock::new(None),
            connect_nano_total: AtomicU64::new(0),
            connect_nano_max: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            alive_nano_total: AtomicU64::new(0),
            last_error: RwLock::new(None),
            last_error_time_millis: AtomicU64::new(0),
            connect_last_time_millis: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            transaction_start_count: AtomicU64::new(0),
            commit_count: AtomicU64::new(0),
            rollback_count: AtomicU64::new(0),
            alive_nano_min: AtomicU64::new(0),
            alive_nano_max: AtomicU64::new(0),
            histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            connections: DashMap::new(),
        }
    }

    pub fn before_connect(&self) {
        let connecting = self.connecting_count.fetch_add(1, Ordering::AcqRel) + 1;
        update_i32_max(&self.connecting_max, connecting);
        self.connect_count.fetch_add(1, Ordering::Relaxed);
        self.connect_last_time_millis
            .store(now_millis(), Ordering::Release);
    }

    pub fn after_connected(&self, elapsed: Duration) {
        self.connecting_count.fetch_sub(1, Ordering::AcqRel);
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.connect_nano_total.fetch_add(nanos, Ordering::Relaxed);
        update_u64_max(&self.connect_nano_max, nanos);
        let active = self.active_count.fetch_add(1, Ordering::AcqRel) + 1;
        update_i32_max(&self.active_count_max, active);
    }

    pub fn connect_error(&self, error: impl ToString) {
        self.connect_error_count.fetch_add(1, Ordering::Relaxed);
        *self.connect_error_last.write() = Some(error.to_string());
        self.error(error);
    }

    pub fn error(&self, error: impl ToString) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        *self.last_error.write() = Some(error.to_string());
        self.last_error_time_millis
            .store(now_millis(), Ordering::Release);
    }

    pub fn after_close(&self, alive: Duration) {
        self.active_count.fetch_sub(1, Ordering::AcqRel);
        let nanos = u64::try_from(alive.as_nanos()).unwrap_or(u64::MAX);
        self.alive_nano_total.fetch_add(nanos, Ordering::Relaxed);
        update_u64_max(&self.alive_nano_max, nanos);
        update_u64_min_zero(&self.alive_nano_min, nanos);
        record_histogram(
            &self.histogram,
            &[1_000, 5_000, 15_000, 60_000, 300_000, 1_800_000],
            alive.as_millis(),
        );
    }

    pub fn register_entry(&self, entry: Arc<JdbcConnectionStatEntry>) {
        self.connections.insert(entry.id, entry);
    }

    /// 返回指定连接条目的共享引用。
    pub fn entry(&self, connection_id: u64) -> Option<Arc<JdbcConnectionStatEntry>> {
        self.connections
            .get(&connection_id)
            .map(|entry| Arc::clone(entry.value()))
    }

    pub fn remove_entry(&self, connection_id: u64) -> bool {
        self.connections.remove(&connection_id).is_some()
    }

    pub fn connection_entries(&self) -> Vec<JdbcConnectionStatEntryValue> {
        self.connections
            .iter()
            .map(|entry| entry.value().snapshot())
            .collect()
    }

    pub fn increment_connection_close_count(&self) {
        self.close_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_connection_commit_count(&self) {
        self.commit_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_connection_rollback_count(&self) {
        self.rollback_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_transaction_start_count(&self) {
        self.transaction_start_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn active_count(&self) -> i32 {
        self.active_count.load(Ordering::Relaxed)
    }

    pub fn active_max(&self) -> i32 {
        self.active_count_max.load(Ordering::Relaxed)
    }

    /// 保留 Java 历史拼写及其实际实现：返回当前 active，而不是 max。
    pub fn ative_count_max(&self) -> i32 {
        self.active_count()
    }

    pub fn connecting_count(&self) -> i32 {
        self.connecting_count.load(Ordering::Relaxed)
    }

    pub fn connecting_max(&self) -> i32 {
        self.connecting_max.load(Ordering::Relaxed)
    }

    pub fn connect_count(&self) -> u64 {
        self.connect_count.load(Ordering::Relaxed)
    }

    pub fn connect_error_count(&self) -> u64 {
        self.connect_error_count.load(Ordering::Relaxed)
    }

    pub fn connect_nano_total(&self) -> u64 {
        self.connect_nano_total.load(Ordering::Relaxed)
    }

    pub fn connect_nano_max(&self) -> u64 {
        self.connect_nano_max.load(Ordering::Relaxed)
    }

    pub fn close_count(&self) -> u64 {
        self.close_count.load(Ordering::Relaxed)
    }

    pub fn transaction_start_count(&self) -> u64 {
        self.transaction_start_count.load(Ordering::Relaxed)
    }

    pub fn commit_count(&self) -> u64 {
        self.commit_count.load(Ordering::Relaxed)
    }

    pub fn rollback_count(&self) -> u64 {
        self.rollback_count.load(Ordering::Relaxed)
    }

    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    pub fn alive_nano_total(&self) -> u64 {
        self.alive_nano_total.load(Ordering::Relaxed)
    }

    pub fn alive_nano_min(&self) -> u64 {
        self.alive_nano_min.load(Ordering::Relaxed)
    }

    pub fn alive_nano_max(&self) -> u64 {
        self.alive_nano_max.load(Ordering::Relaxed)
    }

    pub const fn histogram_ranges() -> [u64; 6] {
        [1_000, 5_000, 15_000, 60_000, 300_000, 1_800_000]
    }

    pub fn histogram_values(&self) -> [u64; 7] {
        std::array::from_fn(|index| self.histogram[index].load(Ordering::Relaxed))
    }

    /// 重置 Java reset 涉及的累计字段，保留 active/activeMax/connecting/current entries。
    pub fn reset(&self) {
        self.connecting_max.store(0, Ordering::Release);
        self.connect_error_count.store(0, Ordering::Release);
        self.error_count.store(0, Ordering::Release);
        self.alive_nano_total.store(0, Ordering::Release);
        self.alive_nano_min.store(0, Ordering::Release);
        self.alive_nano_max.store(0, Ordering::Release);
        *self.last_error.write() = None;
        self.last_error_time_millis.store(0, Ordering::Release);
        self.connect_last_time_millis.store(0, Ordering::Release);
        self.connect_count.store(0, Ordering::Release);
        self.close_count.store(0, Ordering::Release);
        self.transaction_start_count.store(0, Ordering::Release);
        self.commit_count.store(0, Ordering::Release);
        self.rollback_count.store(0, Ordering::Release);
        self.connect_nano_total.store(0, Ordering::Release);
        self.connect_nano_max.store(0, Ordering::Release);
        for bucket in &self.histogram {
            bucket.store(0, Ordering::Release);
        }
    }
}

impl Default for JdbcConnectionStat {
    fn default() -> Self {
        Self::new()
    }
}

fn update_i32_max(target: &AtomicI32, value: i32) {
    let mut current = target.load(Ordering::Acquire);
    while value > current {
        match target.compare_exchange_weak(current, value, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

fn update_u64_max(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Acquire);
    while value > current {
        match target.compare_exchange_weak(current, value, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

fn update_u64_min_zero(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Acquire);
    while current == 0 || value < current {
        match target.compare_exchange_weak(current, value, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

fn record_histogram<const N: usize>(buckets: &[AtomicU64; N], thresholds: &[u128], value: u128) {
    let index = thresholds
        .iter()
        .position(|threshold| value < *threshold)
        .unwrap_or(thresholds.len());
    buckets[index].fetch_add(1, Ordering::Relaxed);
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

const fn non_zero(value: u64) -> Option<u64> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}
