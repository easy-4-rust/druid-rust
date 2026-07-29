//! 对应 Java 类：`com.alibaba.druid.stat.JdbcDataSourceStat`。
//!
//! 数据源级统计收集器。

use super::{JdbcResultSetStat, SqlMerger};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 数据源统计收集器。
///
/// 对应 Druid Java 的 `JdbcDataSourceStat`，聚合池级 + SQL 级统计。
pub struct JdbcDataSourceStat {
    pub name: String,
    pub sql_merger: Arc<SqlMerger>,
    /// `ResultSet` 层统计；对应 Java `JdbcDataSourceStat#getResultSetStat()`。
    pub result_set_stat: Arc<JdbcResultSetStat>,
    // 连接级统计
    pub connect_count: AtomicU64,
    pub connect_error_count: AtomicU64,
    pub close_count: AtomicU64,
    pub active_count: AtomicU64,
    // 慢 SQL 阈值
    pub slow_sql_threshold: Duration,
    // 慢 SQL 计数
    pub slow_sql_count: AtomicU64,
    /// `executeBatch` 调用次数。
    pub execute_batch_count: AtomicU64,
    /// 所有 batch 的 SQL 项数总和。
    pub execute_batch_size_total: AtomicU64,
}

impl JdbcDataSourceStat {
    pub fn new(name: impl Into<String>, slow_sql_threshold: Duration) -> Self {
        Self {
            name: name.into(),
            sql_merger: Arc::new(SqlMerger::new()),
            result_set_stat: Arc::new(JdbcResultSetStat::new()),
            connect_count: AtomicU64::new(0),
            connect_error_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            active_count: AtomicU64::new(0),
            slow_sql_threshold,
            slow_sql_count: AtomicU64::new(0),
            execute_batch_count: AtomicU64::new(0),
            execute_batch_size_total: AtomicU64::new(0),
        }
    }

    /// 记录一次 SQL 执行。
    pub fn record_sql(&self, sql: &str, elapsed: Duration, ok: bool) {
        self.record_sql_with_merge(sql, elapsed, ok, true);
    }

    /// 按 `StatFilter.mergeSql` 开关记录 SQL。
    ///
    /// 对应 Java：`StatFilter#createSqlStat`。普通 `StatFilter` 保存原 SQL，
    /// `MergeStatFilter` 才按参数化模板聚合。
    pub fn record_sql_with_merge(&self, sql: &str, elapsed: Duration, ok: bool, merge_sql: bool) {
        self.sql_merger
            .record_with_merge(sql, elapsed, ok, merge_sql);
        if elapsed >= self.slow_sql_threshold {
            self.slow_sql_count.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(sql = %sql, elapsed_ms = elapsed.as_millis(), "slow SQL detected");
        }
    }

    /// 记录连接创建。
    pub fn record_connect(&self) {
        self.connect_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录连接错误。
    pub fn record_connect_error(&self) {
        self.connect_error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录连接关闭。
    pub fn record_close(&self) {
        self.close_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn connect_count(&self) -> u64 {
        self.connect_count.load(Ordering::Relaxed)
    }
    pub fn slow_sql_count(&self) -> u64 {
        self.slow_sql_count.load(Ordering::Relaxed)
    }

    /// 记录一次批处理及其条目数。
    ///
    /// 对应 Java `incrementExecuteBatchCount()` 与
    /// `JdbcSqlStat#addExecuteBatchCount(long)`。
    pub fn record_execute_batch(&self, batch_size: usize) {
        self.execute_batch_count.fetch_add(1, Ordering::Relaxed);
        self.execute_batch_size_total.fetch_add(
            u64::try_from(batch_size).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }

    /// 返回批处理调用次数。
    pub fn execute_batch_count(&self) -> u64 {
        self.execute_batch_count.load(Ordering::Relaxed)
    }

    /// 返回累计批处理条目数。
    pub fn execute_batch_size_total(&self) -> u64 {
        self.execute_batch_size_total.load(Ordering::Relaxed)
    }

    /// 返回本数据源共享的 `ResultSet` 统计对象。
    pub fn result_set_stat(&self) -> &JdbcResultSetStat {
        self.result_set_stat.as_ref()
    }

    /// 重置本数据源的累计 SQL、连接、批处理与 ResultSet 统计。
    pub fn reset(&self) {
        self.sql_merger.reset();
        self.result_set_stat.reset();
        self.connect_count.store(0, Ordering::Release);
        self.connect_error_count.store(0, Ordering::Release);
        self.close_count.store(0, Ordering::Release);
        self.slow_sql_count.store(0, Ordering::Release);
        self.execute_batch_count.store(0, Ordering::Release);
        self.execute_batch_size_total.store(0, Ordering::Release);
    }
}

impl Default for JdbcDataSourceStat {
    fn default() -> Self {
        Self::new("default", Duration::from_secs(2))
    }
}
