use parking_lot::RwLock;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Statement 层运行统计。
///
/// 对应 Java：`com.alibaba.druid.stat.RdbcStatementStat`。JMX `MBean` 与
/// `CompositeData` 属于 Java 宿主管理协议，不在 Rust 中创建伪类型。
pub struct RdbcStatementStat {
    create_count: AtomicU64,
    prepare_count: AtomicU64,
    prepare_call_count: AtomicU64,
    close_count: AtomicU64,
    running_count: AtomicI32,
    concurrent_max: AtomicI32,
    execute_count: AtomicU64,
    error_count: AtomicU64,
    nano_total: AtomicU64,
    last_error: RwLock<Option<String>>,
    last_error_time_millis: AtomicU64,
    last_sample_time_millis: AtomicU64,
    histogram: [AtomicU64; 5],
}

impl RdbcStatementStat {
    /// 创建全部计数为零的 Statement 统计。
    #[must_use]
    pub fn new() -> Self {
        Self {
            create_count: AtomicU64::new(0),
            prepare_count: AtomicU64::new(0),
            prepare_call_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            running_count: AtomicI32::new(0),
            concurrent_max: AtomicI32::new(0),
            execute_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            nano_total: AtomicU64::new(0),
            last_error: RwLock::new(None),
            last_error_time_millis: AtomicU64::new(0),
            last_sample_time_millis: AtomicU64::new(0),
            histogram: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    /// 在一次 Statement 执行前增加 running/count 并维护并发峰值。
    pub fn before_execute(&self) {
        let running = self.running_count.fetch_add(1, Ordering::AcqRel) + 1;
        update_i32_max(&self.concurrent_max, running);
        self.execute_count.fetch_add(1, Ordering::Relaxed);
        self.last_sample_time_millis
            .store(now_millis(), Ordering::Release);
    }

    /// 在执行结束时减少 running，累计耗时并记录 Java 四阈值直方图。
    pub fn after_execute(&self, elapsed: Duration) {
        self.running_count.fetch_sub(1, Ordering::AcqRel);
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.nano_total.fetch_add(nanos, Ordering::Relaxed);
        record_histogram(
            &self.histogram,
            &[10, 100, 1_000, 10_000],
            elapsed.as_millis(),
        );
    }

    /// 记录一次执行错误及最后错误文本/时间。
    pub fn error(&self, error: impl ToString) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        *self.last_error.write() = Some(error.to_string());
        self.last_error_time_millis
            .store(now_millis(), Ordering::Release);
    }

    /// 记录成功创建普通 Statement。
    pub fn increment_create_counter(&self) {
        self.create_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录成功创建 `PreparedStatement`。
    pub fn increment_prepare_counter(&self) {
        self.prepare_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录成功创建 `CallableStatement`。
    pub fn increment_prepare_call_count(&self) {
        self.prepare_call_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录成功关闭一个逻辑 Statement wrapper。
    pub fn increment_statement_close_counter(&self) {
        self.close_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn create_count(&self) -> u64 {
        self.create_count.load(Ordering::Relaxed)
    }

    pub fn prepare_count(&self) -> u64 {
        self.prepare_count.load(Ordering::Relaxed)
    }

    pub fn prepare_call_count(&self) -> u64 {
        self.prepare_call_count.load(Ordering::Relaxed)
    }

    pub fn close_count(&self) -> u64 {
        self.close_count.load(Ordering::Relaxed)
    }

    pub fn running_count(&self) -> i32 {
        self.running_count.load(Ordering::Relaxed)
    }

    pub fn concurrent_max(&self) -> i32 {
        self.concurrent_max.load(Ordering::Relaxed)
    }

    pub fn execute_count(&self) -> u64 {
        self.execute_count.load(Ordering::Relaxed)
    }

    pub fn execute_success_count(&self) -> i128 {
        i128::from(self.execute_count())
            - i128::from(self.error_count())
            - i128::from(self.running_count())
    }

    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    pub fn nano_total(&self) -> u64 {
        self.nano_total.load(Ordering::Relaxed)
    }

    pub fn millis_total(&self) -> u64 {
        self.nano_total() / 1_000_000
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.read().clone()
    }

    pub fn last_error_time_millis(&self) -> Option<u64> {
        non_zero(self.last_error_time_millis.load(Ordering::Acquire))
    }

    pub fn execute_last_time_millis(&self) -> Option<u64> {
        non_zero(self.last_sample_time_millis.load(Ordering::Acquire))
    }

    pub const fn histogram_ranges() -> [u64; 4] {
        [10, 100, 1_000, 10_000]
    }

    pub fn histogram_values(&self) -> [u64; 5] {
        std::array::from_fn(|index| self.histogram[index].load(Ordering::Relaxed))
    }

    /// 重置累计字段；与 Java 一样把当前 runningCount 也清零。
    pub fn reset(&self) {
        self.create_count.store(0, Ordering::Release);
        self.prepare_count.store(0, Ordering::Release);
        self.prepare_call_count.store(0, Ordering::Release);
        self.close_count.store(0, Ordering::Release);
        self.running_count.store(0, Ordering::Release);
        self.concurrent_max.store(0, Ordering::Release);
        self.execute_count.store(0, Ordering::Release);
        self.error_count.store(0, Ordering::Release);
        self.nano_total.store(0, Ordering::Release);
        *self.last_error.write() = None;
        self.last_error_time_millis.store(0, Ordering::Release);
        self.last_sample_time_millis.store(0, Ordering::Release);
        for bucket in &self.histogram {
            bucket.store(0, Ordering::Release);
        }
    }
}

impl Default for RdbcStatementStat {
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
