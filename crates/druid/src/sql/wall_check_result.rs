use super::{WallSqlStat, WallUpdateCheckItem, WallViolation};
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
    sql_stat: Option<Arc<WallSqlStat>>,
    update_check_items: Option<Vec<WallUpdateCheckItem>>,
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
            sql_stat: Some(sql_stat),
            update_check_items: None,
        }
    }

    /// 创建 privileged 快速通行结果。
    ///
    /// 对应 Java `WallProvider#checkInternal` 的 privileged 分支：只保存原 SQL，
    /// 不产生 AST、违规或 `WallSqlStat`。
    #[must_use]
    pub fn privileged(sql: String) -> Self {
        Self {
            sql,
            statements: Vec::new(),
            violations: Vec::new(),
            syntax_error: false,
            sql_stat: None,
            update_check_items: None,
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
    pub fn sql_stat(&self) -> Option<&Arc<WallSqlStat>> {
        self.sql_stat.as_ref()
    }

    /// 返回 UPDATE 赋值/过滤检查项。
    #[must_use]
    pub fn update_check_items(&self) -> Option<&[WallUpdateCheckItem]> {
        self.update_check_items.as_deref()
    }

    /// 设置 UPDATE 赋值/过滤检查项；`None` 与空列表保持不同。
    pub fn set_update_check_items(&mut self, update_check_items: Option<Vec<WallUpdateCheckItem>>) {
        self.update_check_items = update_check_items;
    }
}
