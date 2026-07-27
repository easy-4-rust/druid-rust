//! 对应 Java 类：com.alibaba.druid.stat.JdbcDataSourceStat + DruidDataSourceStatManager
//!
//! 数据源级统计收集器。

use crate::merge::SqlMerger;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 数据源统计收集器。
///
/// 对应 Druid Java 的 `JdbcDataSourceStat`，聚合池级 + SQL 级统计。
pub struct StatsCollector {
    pub name: String,
    pub sql_merger: Arc<SqlMerger>,
    // 连接级统计
    pub connect_count: AtomicU64,
    pub connect_error_count: AtomicU64,
    pub close_count: AtomicU64,
    pub active_count: AtomicU64,
    // 慢 SQL 阈值
    pub slow_sql_threshold: Duration,
    // 慢 SQL 计数
    pub slow_sql_count: AtomicU64,
}

impl StatsCollector {
    pub fn new(name: impl Into<String>, slow_sql_threshold: Duration) -> Self {
        Self {
            name: name.into(),
            sql_merger: Arc::new(SqlMerger::new()),
            connect_count: AtomicU64::new(0),
            connect_error_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            active_count: AtomicU64::new(0),
            slow_sql_threshold,
            slow_sql_count: AtomicU64::new(0),
        }
    }

    /// 记录一次 SQL 执行。
    pub fn record_sql(&self, sql: &str, elapsed: Duration, ok: bool) {
        self.sql_merger.record(sql, elapsed, ok);
        if elapsed >= self.slow_sql_threshold {
            self.slow_sql_count.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(sql = %sql, elapsed_ms = elapsed.as_millis(), "slow SQL detected");
        }
    }

    /// 记录连接创建。
    pub fn record_connect(&self) { self.connect_count.fetch_add(1, Ordering::Relaxed); }

    /// 记录连接错误。
    pub fn record_connect_error(&self) { self.connect_error_count.fetch_add(1, Ordering::Relaxed); }

    /// 记录连接关闭。
    pub fn record_close(&self) { self.close_count.fetch_add(1, Ordering::Relaxed); }

    pub fn connect_count(&self) -> u64 { self.connect_count.load(Ordering::Relaxed) }
    pub fn slow_sql_count(&self) -> u64 { self.slow_sql_count.load(Ordering::Relaxed) }
}

impl Default for StatsCollector {
    fn default() -> Self { Self::new("default", Duration::from_secs(2)) }
}
