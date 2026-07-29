use super::{WallSqlStat, WallViolation};
use std::sync::Arc;

/// 一次 Wall 检查的结构化结果。
///
/// 对应 Java：`com.alibaba.druid.wall.WallCheckResult`。Rust 的 SQL AST 来自
/// `sqlparser-rs`，属于规划迁移中的平台适配。
pub struct WallCheckResult {
    sql: String,
    statements: Vec<sqlparser::ast::Statement>,
    violations: Vec<WallViolation>,
    syntax_error: bool,
    sql_stat: Arc<WallSqlStat>,
}

impl WallCheckResult {
    /// 创建检查结果。
    #[must_use]
    pub fn new(
        sql: String,
        statements: Vec<sqlparser::ast::Statement>,
        violations: Vec<WallViolation>,
        syntax_error: bool,
        sql_stat: Arc<WallSqlStat>,
    ) -> Self {
        Self {
            sql,
            statements,
            violations,
            syntax_error,
            sql_stat,
        }
    }

    /// 返回原始 SQL。
    #[must_use]
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// 返回解析后的 statement 列表。
    #[must_use]
    pub fn statements(&self) -> &[sqlparser::ast::Statement] {
        &self.statements
    }

    /// 返回违规列表。
    #[must_use]
    pub fn violations(&self) -> &[WallViolation] {
        &self.violations
    }

    /// 返回是否为语法错误。
    #[must_use]
    pub fn is_syntax_error(&self) -> bool {
        self.syntax_error
    }

    /// 返回共享 SQL 统计对象。
    #[must_use]
    pub fn sql_stat(&self) -> &Arc<WallSqlStat> {
        &self.sql_stat
    }
}
