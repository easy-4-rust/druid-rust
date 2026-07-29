use super::JdbcSqlStatValue;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// 单条参数化 SQL 的并发运行统计。
///
/// 对应 Java：`com.alibaba.druid.stat.JdbcSqlStat`。
#[derive(Debug)]
pub struct JdbcSqlStat {
    pub sql: String,
    pub fingerprint: u64,
    pub execute_count: AtomicU64,
    pub total_time_ns: AtomicU64,
    pub max_time_ns: AtomicU64,
    pub error_count: AtomicU64,
    pub fetch_row_count: AtomicU64,
    pub running_count: AtomicU64,
    pub concurrent_max: AtomicU64,
}

impl JdbcSqlStat {
    /// 创建 SQL 统计对象。
    #[must_use]
    pub fn new(sql: String, fingerprint: u64) -> Self {
        Self {
            sql,
            fingerprint,
            execute_count: AtomicU64::new(0),
            total_time_ns: AtomicU64::new(0),
            max_time_ns: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            fetch_row_count: AtomicU64::new(0),
            running_count: AtomicU64::new(0),
            concurrent_max: AtomicU64::new(0),
        }
    }

    /// 记录一次完成的 SQL 执行。
    pub fn record(&self, elapsed: Duration, ok: bool) {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.execute_count.fetch_add(1, Ordering::Relaxed);
        self.total_time_ns.fetch_add(nanos, Ordering::Relaxed);
        self.max_time_ns.fetch_max(nanos, Ordering::Relaxed);
        if !ok {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 返回执行次数。
    #[must_use]
    pub fn execute_count(&self) -> u64 {
        self.execute_count.load(Ordering::Relaxed)
    }

    /// 返回总耗时毫秒。
    #[must_use]
    pub fn total_time_ms(&self) -> f64 {
        self.total_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// 返回最大耗时毫秒。
    #[must_use]
    pub fn max_time_ms(&self) -> f64 {
        self.max_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// 返回错误次数。
    #[must_use]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// 返回不可变管理快照。
    #[must_use]
    pub fn stat_value(&self) -> JdbcSqlStatValue {
        let id = self.fingerprint & i64::MAX as u64;
        JdbcSqlStatValue {
            id,
            sql: self.sql.clone(),
            hash: id,
            execute_count: self.execute_count(),
            total_time_millis: self.total_time_ms() as u64,
            max_timespan_millis: self.max_time_ms() as u64,
            error_count: self.error_count(),
            fetch_row_count: self.fetch_row_count.load(Ordering::Acquire),
            running_count: self.running_count.load(Ordering::Acquire),
            concurrent_max: self.concurrent_max.load(Ordering::Acquire),
        }
    }
}
