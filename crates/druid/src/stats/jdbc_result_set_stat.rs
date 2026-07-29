//! `ResultSet` 全局统计对象。
//!
//! 对应 Java：`com.alibaba.druid.stat.JdbcResultSetStat`。

use crate::core::DruidError;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 汇总 `ResultSet` 打开、关闭、存活时间、错误和读取行数。
///
/// 所有计数均使用原子类型，保留 Java 对象可被多个连接并发更新的语义。
#[derive(Debug)]
pub struct JdbcResultSetStat {
    opening_count: AtomicI32,
    opening_max: AtomicI32,
    open_count: AtomicU64,
    error_count: AtomicU64,
    alive_nano_total: AtomicU64,
    alive_nano_max: AtomicU64,
    alive_nano_min: AtomicU64,
    last_error: Mutex<Option<DruidError>>,
    last_error_time: AtomicU64,
    last_open_time: AtomicU64,
    fetch_row_count: AtomicU64,
    close_count: AtomicU64,
}

impl JdbcResultSetStat {
    /// 创建全部累计值为零的统计对象。
    pub fn new() -> Self {
        Self {
            opening_count: AtomicI32::new(0),
            opening_max: AtomicI32::new(0),
            open_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            alive_nano_total: AtomicU64::new(0),
            alive_nano_max: AtomicU64::new(0),
            alive_nano_min: AtomicU64::new(0),
            last_error: Mutex::new(None),
            last_error_time: AtomicU64::new(0),
            last_open_time: AtomicU64::new(0),
            fetch_row_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
        }
    }

    /// 重置 Java `reset()` 涵盖的累计字段。
    ///
    /// Java 原实现不会重置当前 `openingCount`，此处刻意保留该行为。
    pub fn reset(&self) {
        self.opening_max.store(0, Ordering::SeqCst);
        self.open_count.store(0, Ordering::SeqCst);
        self.error_count.store(0, Ordering::SeqCst);
        self.alive_nano_total.store(0, Ordering::SeqCst);
        self.alive_nano_max.store(0, Ordering::SeqCst);
        self.alive_nano_min.store(0, Ordering::SeqCst);
        *self
            .last_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        self.last_error_time.store(0, Ordering::SeqCst);
        self.last_open_time.store(0, Ordering::SeqCst);
        self.fetch_row_count.store(0, Ordering::SeqCst);
        self.close_count.store(0, Ordering::SeqCst);
    }

    /// 记录 `ResultSet` 打开，并按 Java CAS 规则更新并发峰值。
    pub fn before_open(&self) {
        let invoking = self.opening_count.fetch_add(1, Ordering::SeqCst) + 1;
        let mut maximum = self.opening_max.load(Ordering::SeqCst);
        while invoking > maximum {
            match self.opening_max.compare_exchange(
                maximum,
                invoking,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(current) => maximum = current,
            }
        }
        self.open_count.fetch_add(1, Ordering::SeqCst);
        self.last_open_time.store(now_millis(), Ordering::SeqCst);
    }

    /// 返回累计错误数。
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::SeqCst)
    }

    /// 返回当前仍打开的 `ResultSet` 数。
    pub fn opening_count(&self) -> i32 {
        self.opening_count.load(Ordering::SeqCst)
    }

    /// 返回历史同时打开峰值。
    pub fn opening_max(&self) -> i32 {
        self.opening_max.load(Ordering::SeqCst)
    }

    /// 返回累计打开次数。
    pub fn open_count(&self) -> u64 {
        self.open_count.load(Ordering::SeqCst)
    }

    /// 返回最近打开时间的 Unix 毫秒；尚未打开时返回 `None`。
    pub fn last_open_time_millis(&self) -> Option<u64> {
        non_zero(self.last_open_time.load(Ordering::SeqCst))
    }

    /// 返回累计存活纳秒。
    pub fn alive_nano_total(&self) -> u64 {
        self.alive_nano_total.load(Ordering::SeqCst)
    }

    /// 返回累计存活毫秒，使用与 Java 相同的整数截断。
    pub fn alive_millis_total(&self) -> u64 {
        self.alive_nano_total() / 1_000_000
    }

    /// 返回最短存活毫秒。
    ///
    /// Java 字段初始为 0 且只在新值更小时更新，正耗时下会保持 0。
    pub fn alive_millis_min(&self) -> u64 {
        self.alive_nano_min.load(Ordering::SeqCst) / 1_000_000
    }

    /// 返回最长存活毫秒。
    pub fn alive_millis_max(&self) -> u64 {
        self.alive_nano_max.load(Ordering::SeqCst) / 1_000_000
    }

    /// 记录 `ResultSet` 关闭及其存活纳秒。
    pub fn after_close(&self, alive_nano: u64) {
        self.opening_count.fetch_sub(1, Ordering::SeqCst);
        self.alive_nano_total
            .fetch_add(alive_nano, Ordering::SeqCst);
        update_max(&self.alive_nano_max, alive_nano);

        let mut minimum = self.alive_nano_min.load(Ordering::SeqCst);
        while alive_nano < minimum {
            match self.alive_nano_min.compare_exchange(
                minimum,
                alive_nano,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(current) => minimum = current,
            }
        }
    }

    /// 返回最近一次错误的值副本。
    pub fn last_error(&self) -> Option<DruidError> {
        self.last_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 返回最近错误时间的 Unix 毫秒；尚无错误时返回 `None`。
    pub fn last_error_time_millis(&self) -> Option<u64> {
        non_zero(self.last_error_time.load(Ordering::SeqCst))
    }

    /// 记录一次 `ResultSet` 错误及发生时间。
    pub fn error(&self, error: DruidError) {
        self.error_count.fetch_add(1, Ordering::SeqCst);
        *self
            .last_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
        self.last_error_time.store(now_millis(), Ordering::SeqCst);
    }

    /// 返回累计持有毫秒；对应 Java `MBean` 的 `getHoldMillisTotal()`。
    pub fn hold_millis_total(&self) -> u64 {
        self.alive_nano_total() / 1_000_000
    }

    /// 返回累计读取行数。
    pub fn fetch_row_count(&self) -> u64 {
        self.fetch_row_count.load(Ordering::SeqCst)
    }

    /// 返回累计关闭计数。
    pub fn close_count(&self) -> u64 {
        self.close_count.load(Ordering::SeqCst)
    }

    /// 增加累计读取行数。
    pub fn add_fetch_row_count(&self, fetch_count: u64) {
        self.fetch_row_count
            .fetch_add(fetch_count, Ordering::SeqCst);
    }

    /// 增加关闭计数。
    pub fn increment_close_counter(&self) {
        self.close_count.fetch_add(1, Ordering::SeqCst);
    }
}

impl Default for JdbcResultSetStat {
    fn default() -> Self {
        Self::new()
    }
}

fn update_max(target: &AtomicU64, value: u64) {
    let mut maximum = target.load(Ordering::SeqCst);
    while value > maximum {
        match target.compare_exchange(maximum, value, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(current) => maximum = current,
        }
    }
}

fn now_millis() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

fn non_zero(value: u64) -> Option<u64> {
    (value != 0).then_some(value)
}
