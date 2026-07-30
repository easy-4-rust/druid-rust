//! 对应 Java 类：`com.alibaba.druid.stat.JdbcDataSourceStat`。
//!
//! 数据源级统计收集器。

use super::{JdbcConnectionStat, JdbcResultSetStat, JdbcSqlStat, JdbcStatementStat, SqlMerger};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 数据源统计收集器。
///
/// 对应 Druid Java 的 `JdbcDataSourceStat`，聚合池级 + SQL 级统计。
pub struct JdbcDataSourceStat {
    pub name: String,
    reset_stat_enable: AtomicBool,
    pub sql_merger: Arc<SqlMerger>,
    /// `ResultSet` 层统计；对应 Java `JdbcDataSourceStat#getResultSetStat()`。
    pub result_set_stat: Arc<JdbcResultSetStat>,
    /// `Connection` 层统计；对应 Java `getConnectionStat()`。
    pub connection_stat: Arc<JdbcConnectionStat>,
    /// `Statement` 层统计；对应 Java `getStatementStat()`。
    pub statement_stat: Arc<JdbcStatementStat>,
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
    /// 数据源层全部 Statement/PreparedStatement 执行次数。
    pub execute_count: AtomicU64,
    /// 数据源层执行失败次数。
    pub error_count: AtomicU64,
    /// 提交次数。
    pub commit_count: AtomicU64,
    /// 回滚次数。
    pub rollback_count: AtomicU64,
    /// 实际开始事务的次数。
    pub start_transaction_count: AtomicU64,
    /// 七个阈值形成的八档连接持有时长直方图（毫秒）。
    connection_hold_time_histogram: [AtomicU64; 8],
    /// 六个阈值形成的七档事务时长直方图（毫秒）。
    transaction_histogram: [AtomicU64; 7],
    clob_open_count: AtomicU64,
    blob_open_count: AtomicU64,
}

impl JdbcDataSourceStat {
    pub fn new(name: impl Into<String>, slow_sql_threshold: Duration) -> Self {
        Self {
            name: name.into(),
            reset_stat_enable: AtomicBool::new(true),
            sql_merger: Arc::new(SqlMerger::new()),
            result_set_stat: Arc::new(JdbcResultSetStat::new()),
            connection_stat: Arc::new(JdbcConnectionStat::new()),
            statement_stat: Arc::new(JdbcStatementStat::new()),
            connect_count: AtomicU64::new(0),
            connect_error_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            active_count: AtomicU64::new(0),
            slow_sql_threshold,
            slow_sql_count: AtomicU64::new(0),
            execute_batch_count: AtomicU64::new(0),
            execute_batch_size_total: AtomicU64::new(0),
            execute_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            commit_count: AtomicU64::new(0),
            rollback_count: AtomicU64::new(0),
            start_transaction_count: AtomicU64::new(0),
            connection_hold_time_histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            transaction_histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            clob_open_count: AtomicU64::new(0),
            blob_open_count: AtomicU64::new(0),
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

    /// 按 `StatFilter` 的动态慢 SQL 阈值记录执行。
    ///
    /// Java 的 `StatFilterMBean#setSlowSqlMillis` 可在运行期修改阈值，且允许
    /// 负数；因此本入口接收有符号毫秒值，不把它压成 `Duration`。
    pub fn record_sql_with_merge_and_slow_millis(
        &self,
        sql: &str,
        elapsed: Duration,
        ok: bool,
        merge_sql: bool,
        slow_sql_millis: i64,
    ) {
        self.record_sql_with_merge_and_slow_millis_stat(
            sql,
            elapsed,
            ok,
            merge_sql,
            slow_sql_millis,
        );
    }

    /// 记录 SQL 并返回本次命中的 SQL 统计对象。
    pub fn record_sql_with_merge_and_slow_millis_stat(
        &self,
        sql: &str,
        elapsed: Duration,
        ok: bool,
        merge_sql: bool,
        slow_sql_millis: i64,
    ) -> Arc<JdbcSqlStat> {
        let stat = self
            .sql_merger
            .record_with_merge_stat(sql, elapsed, ok, merge_sql);
        let elapsed_millis = i128::try_from(elapsed.as_millis()).unwrap_or(i128::MAX);
        if elapsed_millis >= i128::from(slow_sql_millis) {
            self.slow_sql_count.fetch_add(1, Ordering::Relaxed);
        }
        stat
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

    /// 返回 SQL 统计容量上限。
    pub fn max_sql_size(&self) -> i32 {
        self.sql_merger.max_sql_size()
    }

    /// 设置 SQL 统计容量上限。
    pub fn set_max_sql_size(&self, max_sql_size: i32) {
        self.sql_merger.set_max_sql_size(max_sql_size);
    }

    /// 返回因容量淘汰的已执行 SQL 数。
    pub fn skip_sql_count(&self) -> u64 {
        self.sql_merger.skip_sql_count()
    }

    /// 记录成功打开一个非空 Clob。
    pub fn record_clob_open(&self) {
        self.clob_open_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录成功打开一个非空 Blob。
    pub fn record_blob_open(&self) {
        self.blob_open_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录一次数据源层 SQL 执行结果。
    pub fn record_execute_result(&self, ok: bool) {
        self.execute_count.fetch_add(1, Ordering::Relaxed);
        if !ok {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 记录事务开始。
    pub fn record_start_transaction(&self) {
        self.start_transaction_count.fetch_add(1, Ordering::Relaxed);
        self.connection_stat.increment_transaction_start_count();
    }

    /// 记录事务提交及其持续时间。
    pub fn record_commit(&self, elapsed: Option<Duration>) {
        self.commit_count.fetch_add(1, Ordering::Relaxed);
        self.connection_stat.increment_connection_commit_count();
        if let Some(elapsed) = elapsed {
            Self::record_histogram(
                &self.transaction_histogram,
                &[1, 10, 100, 1_000, 10_000, 100_000],
                elapsed,
            );
        }
    }

    /// 记录事务回滚及其持续时间。
    pub fn record_rollback(&self, elapsed: Option<Duration>) {
        self.rollback_count.fetch_add(1, Ordering::Relaxed);
        self.connection_stat.increment_connection_rollback_count();
        if let Some(elapsed) = elapsed {
            Self::record_histogram(
                &self.transaction_histogram,
                &[1, 10, 100, 1_000, 10_000, 100_000],
                elapsed,
            );
        }
    }

    /// 记录一次连接租约持有时长。
    pub fn record_connection_hold(&self, elapsed: Duration) {
        Self::record_histogram(
            &self.connection_hold_time_histogram,
            &[1, 10, 100, 1_000, 10_000, 100_000, 1_000_000],
            elapsed,
        );
    }

    fn record_histogram<const N: usize>(
        buckets: &[AtomicU64; N],
        thresholds_millis: &[u64],
        elapsed: Duration,
    ) {
        let elapsed_millis = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        let index = thresholds_millis
            .iter()
            .position(|threshold| elapsed_millis < *threshold)
            .unwrap_or(thresholds_millis.len());
        buckets[index].fetch_add(1, Ordering::Relaxed);
    }

    /// 取得并重置数据源运行计数与直方图。
    pub(crate) fn runtime_snapshot_and_reset(&self) -> JdbcDataSourceRuntimeSnapshot {
        JdbcDataSourceRuntimeSnapshot {
            execute_count: self.execute_count.swap(0, Ordering::AcqRel),
            error_count: self.error_count.swap(0, Ordering::AcqRel),
            commit_count: self.commit_count.swap(0, Ordering::AcqRel),
            rollback_count: self.rollback_count.swap(0, Ordering::AcqRel),
            start_transaction_count: self.start_transaction_count.swap(0, Ordering::AcqRel),
            transaction_histogram: std::array::from_fn(|index| {
                self.transaction_histogram[index].swap(0, Ordering::AcqRel)
            }),
            connection_hold_time_histogram: std::array::from_fn(|index| {
                self.connection_hold_time_histogram[index].swap(0, Ordering::AcqRel)
            }),
            clob_open_count: self.clob_open_count.swap(0, Ordering::AcqRel),
            blob_open_count: self.blob_open_count.swap(0, Ordering::AcqRel),
            sql_skip_count: self.sql_merger.take_skip_sql_count(),
        }
    }

    /// 返回本数据源共享的 `ResultSet` 统计对象。
    pub fn result_set_stat(&self) -> &JdbcResultSetStat {
        self.result_set_stat.as_ref()
    }

    /// 返回本数据源共享的 Connection 层统计对象。
    pub fn connection_stat(&self) -> &Arc<JdbcConnectionStat> {
        &self.connection_stat
    }

    /// 返回本数据源共享的 Statement 层统计对象。
    pub fn statement_stat(&self) -> &Arc<JdbcStatementStat> {
        &self.statement_stat
    }

    /// 重置本数据源的累计 SQL、连接、批处理与 ResultSet 统计。
    pub fn reset(&self) {
        if !self.is_reset_stat_enable() {
            return;
        }
        self.sql_merger.reset();
        self.result_set_stat.reset();
        self.connection_stat.reset();
        self.statement_stat.reset();
        self.connect_count.store(0, Ordering::Release);
        self.connect_error_count.store(0, Ordering::Release);
        self.close_count.store(0, Ordering::Release);
        self.slow_sql_count.store(0, Ordering::Release);
        self.execute_batch_count.store(0, Ordering::Release);
        self.execute_batch_size_total.store(0, Ordering::Release);
        let _ = self.runtime_snapshot_and_reset();
    }

    /// 返回 Java `JdbcDataSourceStat#isResetStatEnable()`。
    #[must_use]
    pub fn is_reset_stat_enable(&self) -> bool {
        self.reset_stat_enable.load(Ordering::Acquire)
    }

    /// 设置 Java `JdbcDataSourceStat#setResetStatEnable(boolean)`。
    pub fn set_reset_stat_enable(&self, reset_stat_enable: bool) {
        self.reset_stat_enable
            .store(reset_stat_enable, Ordering::Release);
    }
}

/// `DruidDataSourceStatValue` 使用的数据源运行区间快照。
pub(crate) struct JdbcDataSourceRuntimeSnapshot {
    pub(crate) execute_count: u64,
    pub(crate) error_count: u64,
    pub(crate) commit_count: u64,
    pub(crate) rollback_count: u64,
    pub(crate) start_transaction_count: u64,
    pub(crate) transaction_histogram: [u64; 7],
    pub(crate) connection_hold_time_histogram: [u64; 8],
    pub(crate) clob_open_count: u64,
    pub(crate) blob_open_count: u64,
    pub(crate) sql_skip_count: u64,
}

impl Default for JdbcDataSourceStat {
    fn default() -> Self {
        Self::new("default", Duration::from_secs(2))
    }
}
