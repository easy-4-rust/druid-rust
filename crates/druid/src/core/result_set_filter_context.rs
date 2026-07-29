//! `ResultSet Filter` 调用上下文。
//!
//! 对应 Java 平台对象：
//! `com.alibaba.druid.proxy.jdbc.ResultSetProxyImpl` 中由 Filter 使用的状态。

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// 在一条 `ResultSet Filter` 调用链中共享的可观测状态。
///
/// 该对象保留 Java `constructNano`、`fetchRowCount` 与 `closeCount` 的更新时机，
/// 但不持有物理结果集，避免与池化 Statement 形成所有权环。
#[derive(Debug)]
pub struct ResultSetFilterContext {
    construct_time: OnceLock<Instant>,
    fetch_row_count: AtomicI32,
    close_count: AtomicU64,
}

impl ResultSetFilterContext {
    /// 创建尚未设置构造时刻、抓取数和关闭数均为零的上下文。
    pub fn new() -> Self {
        Self {
            construct_time: OnceLock::new(),
            fetch_row_count: AtomicI32::new(0),
            close_count: AtomicU64::new(0),
        }
    }

    /// 仅在尚未设置时记录构造时刻。
    ///
    /// 对应 Java：`ResultSetProxyImpl#setConstructNano()`。
    pub fn set_construct_time(&self) {
        let _ = self.construct_time.set(Instant::now());
    }

    /// 返回从构造时刻到当前的耗时；尚未设置时返回 `None`。
    pub fn elapsed(&self) -> Option<Duration> {
        self.construct_time.get().map(Instant::elapsed)
    }

    /// 记录成功抓取的历史峰值行号。
    pub fn record_fetch_row_count(&self, fetch_row_count: i32) {
        self.fetch_row_count
            .fetch_max(fetch_row_count, Ordering::AcqRel);
    }

    /// 返回成功抓取的历史峰值行号。
    pub fn fetch_row_count(&self) -> i32 {
        self.fetch_row_count.load(Ordering::Acquire)
    }

    /// 在整条物理 close 链成功后增加关闭次数。
    ///
    /// 对应 Java：`ResultSetProxyImpl#close()` 在
    /// `chain.resultSet_close(this)` 返回之后执行 `closeCount++`。
    pub fn increment_close_count(&self) {
        self.close_count.fetch_add(1, Ordering::AcqRel);
    }

    /// 返回成功完成的 Filter close 链次数。
    pub fn close_count(&self) -> u64 {
        self.close_count.load(Ordering::Acquire)
    }
}

impl Default for ResultSetFilterContext {
    fn default() -> Self {
        Self::new()
    }
}
