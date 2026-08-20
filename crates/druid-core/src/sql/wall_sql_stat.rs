use super::{WallSqlFunctionStat, WallSqlStatValue, WallSqlTableStat, WallViolation};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Wall 针对一条归一化 SQL 保存的运行统计。
///
/// 对应 Java：`com.alibaba.druid.wall.WallSqlStat`。
pub struct WallSqlStat {
    sql: String,
    sample: String,
    sql_hash: u64,
    violations: Vec<WallViolation>,
    syntax_error: bool,
    table_stats: HashMap<String, WallSqlTableStat>,
    function_stats: HashMap<String, WallSqlFunctionStat>,
    execute_count: AtomicU64,
    execute_error_count: AtomicU64,
    fetch_row_count: AtomicU64,
    update_count: AtomicU64,
}

impl WallSqlStat {
    /// 创建 SQL 统计对象。
    #[must_use]
    pub fn new(sql: String, violations: Vec<WallViolation>, syntax_error: bool) -> Self {
        Self::new_with_stats(
            sql,
            violations,
            syntax_error,
            HashMap::new(),
            HashMap::new(),
        )
    }

    /// 使用 SQL 级表/函数解析统计创建对象。
    #[must_use]
    pub fn new_with_stats(
        sql: String,
        violations: Vec<WallViolation>,
        syntax_error: bool,
        table_stats: HashMap<String, WallSqlTableStat>,
        function_stats: HashMap<String, WallSqlFunctionStat>,
    ) -> Self {
        let sql_hash = crate::stats::fingerprint(&sql);
        Self {
            sample: sql.clone(),
            sql,
            sql_hash,
            violations,
            syntax_error,
            table_stats,
            function_stats,
            execute_count: AtomicU64::new(0),
            execute_error_count: AtomicU64::new(0),
            fetch_row_count: AtomicU64::new(0),
            update_count: AtomicU64::new(0),
        }
    }

    /// 返回 SQL 文本。
    #[must_use]
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// 返回违规列表。
    #[must_use]
    pub fn violations(&self) -> &[WallViolation] {
        &self.violations
    }

    /// 返回解析阶段是否发生语法错误。
    ///
    /// 对应 Java：`WallSqlStat#isSyntaxError()`。
    #[must_use]
    pub const fn is_syntax_error(&self) -> bool {
        self.syntax_error
    }

    /// 返回该 SQL 涉及的表及操作次数。
    #[must_use]
    pub fn table_stats(&self) -> &HashMap<String, WallSqlTableStat> {
        &self.table_stats
    }

    /// 返回该 SQL 涉及的函数及调用次数。
    #[must_use]
    pub fn function_stats(&self) -> &HashMap<String, WallSqlFunctionStat> {
        &self.function_stats
    }

    /// 增加执行次数。
    pub fn increment_execute_count(&self) -> u64 {
        self.execute_count.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// 增加执行错误次数。
    pub fn increment_execute_error_count(&self) -> u64 {
        self.execute_error_count.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// 累加抓取行数。
    pub fn add_fetch_row_count(&self, delta: u64) -> u64 {
        self.fetch_row_count.fetch_add(delta, Ordering::AcqRel) + delta
    }

    /// 累加影响行数。
    pub fn add_update_count(&self, delta: u64) -> u64 {
        self.update_count.fetch_add(delta, Ordering::AcqRel) + delta
    }

    /// 获取快照；`reset=true` 时原子取走累计字段。
    #[must_use]
    pub fn stat_value(&self, reset: bool) -> WallSqlStatValue {
        let load = |value: &AtomicU64| {
            if reset {
                value.swap(0, Ordering::AcqRel)
            } else {
                value.load(Ordering::Acquire)
            }
        };
        WallSqlStatValue {
            sql: self.sql.clone(),
            sql_hash: self.sql_hash,
            sql_sample: self.sample.clone(),
            sql_sample_hash: crate::stats::fingerprint(&self.sample),
            execute_count: load(&self.execute_count),
            execute_error_count: load(&self.execute_error_count),
            fetch_row_count: load(&self.fetch_row_count),
            update_count: load(&self.update_count),
            syntax_error: self.syntax_error,
            violation_message: self.violations.first().map(ToString::to_string),
        }
    }
}
