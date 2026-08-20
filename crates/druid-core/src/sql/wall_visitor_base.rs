//! 对应 Java：`com.alibaba.druid.wall.spi.WallVisitorBase`。

use super::{DbType, WallProvider, WallUpdateCheckItem, WallViolation};
use sqlparser::ast::{
    visit_expressions, visit_relations, Expr, FromTable, ObjectName, Statement, TableFactor,
};
use std::ops::ControlFlow;

/// 各数据库 WallVisitor 共用的状态与基础规则。
pub struct WallVisitorBase<'a> {
    provider: &'a WallProvider,
    violations: Vec<WallViolation>,
    sql_modified: bool,
    sql_end_of_comment: bool,
    update_check_items: Option<Vec<WallUpdateCheckItem>>,
}

impl<'a> WallVisitorBase<'a> {
    /// 绑定 canonical Provider。
    #[must_use]
    pub fn new(provider: &'a WallProvider) -> Self {
        Self {
            provider,
            violations: Vec::new(),
            sql_modified: false,
            sql_end_of_comment: false,
            update_check_items: None,
        }
    }

    /// 返回 Provider。
    #[must_use]
    pub fn provider(&self) -> &'a WallProvider {
        self.provider
    }

    /// 执行所有方言共享的 deny/read-only 检查。
    pub fn check_common(&mut self, statements: &[Statement]) {
        let config = self.provider.config();
        let _: ControlFlow<()> = statements.iter().try_for_each(|statement| {
            let _: ControlFlow<()> = visit_relations(statement, |relation| {
                let name = normalize_name(relation);
                if config.table_check
                    && config
                        .deny_tables
                        .iter()
                        .any(|deny| deny.eq_ignore_ascii_case(&name))
                {
                    self.push_unique(WallViolation::DeniedTable(name));
                }
                ControlFlow::Continue(())
            });

            for table in mutation_tables(statement) {
                if config
                    .read_only_tables
                    .iter()
                    .any(|read_only| read_only.eq_ignore_ascii_case(&table))
                {
                    self.push_unique(WallViolation::ReadOnlyTable(table));
                }
            }
            ControlFlow::Continue(())
        });
    }

    /// 检查 Oracle/MySQL/SQL Server 方言变量。
    pub fn check_deny_variants(&mut self, statements: &[Statement]) {
        if !self.provider.config().variant_check {
            return;
        }
        for statement in statements {
            let _: ControlFlow<()> = visit_expressions(statement, |expression| {
                for name in expression_names(expression) {
                    if self
                        .provider
                        .config()
                        .deny_variants
                        .iter()
                        .any(|deny| deny.eq_ignore_ascii_case(&name))
                    {
                        self.push_unique(WallViolation::DeniedVariant(name));
                    }
                }
                ControlFlow::Continue(())
            });
        }
    }

    /// 拒绝 DB2/PostgreSQL/Oracle 的 `v$`/`v_$` 虚拟表。
    pub fn check_virtual_tables(&mut self, statements: &[Statement]) {
        if !self.provider.config().table_check {
            return;
        }
        for statement in statements {
            let _: ControlFlow<()> = visit_relations(statement, |relation| {
                let name = normalize_name(relation);
                if name.starts_with("v$") || name.starts_with("v_$") {
                    self.push_unique(WallViolation::DeniedTable(name));
                }
                ControlFlow::Continue(())
            });
        }
    }

    /// 增加不重复的违规。
    pub fn push_unique(&mut self, violation: WallViolation) {
        if !self.violations.contains(&violation) {
            self.violations.push(violation);
        }
    }

    /// 返回违规。
    #[must_use]
    pub fn violations(&self) -> &[WallViolation] {
        &self.violations
    }

    /// 返回 SQL 修改状态。
    #[must_use]
    pub fn sql_modified(&self) -> bool {
        self.sql_modified
    }

    /// 设置 SQL 修改状态。
    pub fn set_sql_modified(&mut self, sql_modified: bool) {
        self.sql_modified = sql_modified;
    }

    /// 返回尾注释状态。
    #[must_use]
    pub fn sql_end_of_comment(&self) -> bool {
        self.sql_end_of_comment
    }

    /// 设置尾注释状态。
    pub fn set_sql_end_of_comment(&mut self, sql_end_of_comment: bool) {
        self.sql_end_of_comment = sql_end_of_comment;
    }

    /// 增加 UPDATE 检查项。
    pub fn add_wall_update_check_item(&mut self, item: WallUpdateCheckItem) {
        self.update_check_items
            .get_or_insert_with(Vec::new)
            .push(item);
    }

    /// 返回 UPDATE 检查项。
    #[must_use]
    pub fn update_check_items(&self) -> Option<&[WallUpdateCheckItem]> {
        self.update_check_items.as_deref()
    }

    /// 返回方言 Provider 实际数据库类型。
    #[must_use]
    pub fn db_type(&self) -> DbType {
        self.provider.db_type()
    }
}

fn normalize_name(name: &ObjectName) -> String {
    name.to_string()
        .trim_matches(['\'', '`', '"'])
        .to_lowercase()
}

fn expression_names(expression: &Expr) -> Vec<String> {
    match expression {
        Expr::Identifier(identifier) => vec![normalize_variant(&identifier.value)],
        Expr::CompoundIdentifier(identifiers) => {
            let mut names = identifiers
                .iter()
                .map(|identifier| identifier.value.as_str())
                .collect::<Vec<_>>();
            let joined = normalize_variant(&names.join("."));
            let last = names.pop().map(normalize_variant);
            last.into_iter().chain([joined]).collect()
        }
        _ => Vec::new(),
    }
}

fn normalize_variant(name: &str) -> String {
    name.trim_matches(['\'', '`', '"'])
        .trim_start_matches("@@")
        .trim_start_matches("session.")
        .trim_start_matches("global.")
        .to_lowercase()
}

fn mutation_tables(statement: &Statement) -> Vec<String> {
    match statement {
        Statement::Insert(insert) => vec![normalize_name(&insert.table_name)],
        Statement::Update { table, .. } => table_factor_name(&table.relation).into_iter().collect(),
        Statement::Delete(delete) => {
            let tables = match &delete.from {
                FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
            };
            tables
                .iter()
                .filter_map(|table| table_factor_name(&table.relation))
                .collect()
        }
        _ => Vec::new(),
    }
}

fn table_factor_name(table: &TableFactor) -> Option<String> {
    match table {
        TableFactor::Table { name, .. } => Some(normalize_name(name)),
        _ => None,
    }
}
