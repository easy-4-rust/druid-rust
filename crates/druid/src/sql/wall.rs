//! 对应 Java 类：com.alibaba.druid.wall.WallProvider + WallVisitor
//!
//! SQL 防火墙，基于 sqlparser-rs AST 检查 SQL 安全性。

use super::wall_config::WallConfig;
use super::wall_violation::WallViolation;
use super::{DbType, SqlUtils};
use parking_lot::RwLock;
use sqlparser::ast::{
    visit_expressions, BinaryOperator, Expr, FromTable, ObjectName, ObjectType, Query, Select,
    SelectItem, SetExpr, Statement, TableFactor, Value,
};
use sqlparser::parser::Parser;
use std::ops::ControlFlow;
use std::sync::Arc;

/// SQL 防火墙。
pub struct Wall {
    config: Arc<WallConfig>,
    db_type: RwLock<DbType>,
}

impl Wall {
    pub fn new(config: WallConfig) -> Self {
        Self {
            config: Arc::new(config),
            db_type: RwLock::new(DbType::Other),
        }
    }

    /// 使用显式数据库方言创建 Wall。
    #[must_use]
    pub fn with_db_type(config: WallConfig, db_type: DbType) -> Self {
        Self {
            config: Arc::new(config),
            db_type: RwLock::new(db_type),
        }
    }

    pub fn config(&self) -> &WallConfig {
        &self.config
    }

    /// 切换后续 hard-check 使用的数据库方言。
    pub fn set_db_type(&self, db_type: DbType) {
        *self.db_type.write() = db_type;
    }

    /// 返回当前数据库方言。
    #[must_use]
    pub fn db_type(&self) -> DbType {
        *self.db_type.read()
    }

    /// 检查 SQL 是否合规。
    pub fn check(&self, sql: &str) -> Result<(), Vec<WallViolation>> {
        if !self.config.comment_allow && contains_disallowed_comment(sql, self.config.hint_allow) {
            return Err(vec![WallViolation::OperationNotAllowed(
                "COMMENT".to_owned(),
            )]);
        }
        let dialect = SqlUtils::dialect(self.db_type());
        let ast = Parser::parse_sql(dialect.as_ref(), sql)
            .map_err(|e| vec![WallViolation::SyntaxError(e.to_string())])?;
        let mut violations = Vec::new();
        if ast.len() > 1 && !self.config.multi_statement_allow {
            violations.push(WallViolation::MultiStatementNotAllowed);
        }
        for stmt in &ast {
            self.check_statement(stmt, &mut violations);
            let _: ControlFlow<()> = visit_expressions(stmt, |expression| {
                if let Expr::Function(function) = expression {
                    let function_name = function.name.to_string().to_ascii_lowercase();
                    if self
                        .config
                        .deny_functions
                        .iter()
                        .any(|deny| function_name.eq_ignore_ascii_case(deny))
                    {
                        Self::push_unique(
                            &mut violations,
                            WallViolation::DeniedFunction(function_name),
                        );
                    }
                }
                if self.config.must_parameterized
                    && matches!(expression, Expr::Value(value) if !matches!(value, Value::Placeholder(_)))
                {
                    Self::push_unique(&mut violations, WallViolation::MustParameterized);
                }
                ControlFlow::Continue(())
            });
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    fn check_statement(&self, stmt: &Statement, v: &mut Vec<WallViolation>) {
        match stmt {
            Statement::Query(query) => {
                if !self.config.select_allow {
                    v.push(WallViolation::OperationNotAllowed("SELECT".to_owned()));
                }
                self.check_query(query, v);
            }
            Statement::Delete(delete) => {
                if !self.config.delete_allow {
                    v.push(WallViolation::OperationNotAllowed("DELETE".to_owned()));
                }
                if delete.selection.is_none() && self.config.delete_must_have_where {
                    v.push(WallViolation::DeleteWithoutWhere);
                }
                if self.config.delete_where_alway_true_check
                    && delete.selection.as_ref().is_some_and(is_always_true)
                {
                    Self::push_unique(
                        v,
                        WallViolation::AlwaysTrueCondition("DELETE WHERE".to_owned()),
                    );
                }
                self.check_from_table(&delete.from, v);
            }
            Statement::Update {
                table, selection, ..
            } => {
                if !self.config.update_allow {
                    v.push(WallViolation::OperationNotAllowed("UPDATE".to_owned()));
                }
                if selection.is_none() && self.config.update_must_have_where {
                    v.push(WallViolation::UpdateWithoutWhere);
                }
                if self.config.update_where_alway_true_check
                    && selection.as_ref().is_some_and(is_always_true)
                {
                    Self::push_unique(
                        v,
                        WallViolation::AlwaysTrueCondition("UPDATE WHERE".to_owned()),
                    );
                }
                self.check_table_factor(&table.relation, v);
            }
            Statement::Insert(insert) => {
                if !self.config.insert_allow {
                    v.push(WallViolation::OperationNotAllowed("INSERT".to_owned()));
                }
                self.check_object_name(&insert.table_name, v);
            }
            Statement::Drop {
                object_type, names, ..
            } => {
                if *object_type == ObjectType::Table {
                    if !self.config.drop_table_allow {
                        for name in names {
                            v.push(WallViolation::DropTableNotAllowed(name.to_string()));
                        }
                    }
                }
            }
            Statement::Truncate { table_names, .. } => {
                if !self.config.truncate_allow {
                    v.push(WallViolation::TruncateNotAllowed);
                }
                for target in table_names {
                    self.check_object_name(&target.name, v);
                }
            }
            Statement::CreateTable(_) => {
                if !self.config.create_table_allow {
                    v.push(WallViolation::OperationNotAllowed(
                        "CREATE TABLE".to_owned(),
                    ));
                }
            }
            Statement::AlterTable { .. } => {
                if !self.config.alter_table_allow {
                    v.push(WallViolation::OperationNotAllowed("ALTER TABLE".to_owned()));
                }
            }
            Statement::Commit { .. } => {
                if !self.config.commit_allow {
                    v.push(WallViolation::OperationNotAllowed("COMMIT".to_owned()));
                }
            }
            Statement::Rollback { .. } => {
                if !self.config.rollback_allow {
                    v.push(WallViolation::OperationNotAllowed("ROLLBACK".to_owned()));
                }
            }
            Statement::StartTransaction { .. } => {
                if !self.config.start_transaction_allow {
                    v.push(WallViolation::OperationNotAllowed(
                        "START TRANSACTION".to_owned(),
                    ));
                }
            }
            Statement::SetVariable { .. } => {
                if !self.config.set_allow {
                    v.push(WallViolation::OperationNotAllowed("SET".to_owned()));
                }
            }
            _ if !self.config.none_base_statement_allow => {
                v.push(WallViolation::OperationNotAllowed(
                    stmt.to_string()
                        .split_ascii_whitespace()
                        .next()
                        .unwrap_or("STATEMENT")
                        .to_ascii_uppercase(),
                ));
            }
            _ => {}
        }
    }

    fn check_query(&self, query: &Query, v: &mut Vec<WallViolation>) {
        if !self.config.limit_zero_allow && query.limit.as_ref().is_some_and(is_zero_literal) {
            Self::push_unique(v, WallViolation::LimitZeroNotAllowed);
        }
        match &*query.body {
            SetExpr::Select(select) => {
                self.check_select(select, v);
            }
            SetExpr::Query(subquery) => {
                self.check_query(subquery, v);
            }
            SetExpr::SetOperation { left, right, .. } => {
                if let SetExpr::Select(l) = &**left {
                    self.check_select(l, v);
                }
                if let SetExpr::Select(r) = &**right {
                    self.check_select(r, v);
                }
            }
            _ => {}
        }
    }

    fn check_select(&self, select: &Select, v: &mut Vec<WallViolation>) {
        if !self.config.select_all_column_allow
            && select.projection.iter().any(|item| {
                matches!(
                    item,
                    SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _)
                )
            })
        {
            Self::push_unique(v, WallViolation::SelectAllColumnNotAllowed);
        }
        if !self.config.select_into_allow && select.into.is_some() {
            Self::push_unique(
                v,
                WallViolation::OperationNotAllowed("SELECT INTO".to_owned()),
            );
        }
        if self.config.select_where_alway_true_check
            && select.selection.as_ref().is_some_and(is_always_true)
        {
            Self::push_unique(
                v,
                WallViolation::AlwaysTrueCondition("SELECT WHERE".to_owned()),
            );
        }
        if self.config.select_having_alway_true_check
            && select.having.as_ref().is_some_and(is_always_true)
        {
            Self::push_unique(
                v,
                WallViolation::AlwaysTrueCondition("SELECT HAVING".to_owned()),
            );
        }
        // 检查 FROM 子句中的表
        for table_with_joins in &select.from {
            self.check_table_factor(&table_with_joins.relation, v);
        }
    }

    fn check_from_table(&self, from: &FromTable, v: &mut Vec<WallViolation>) {
        let tables = match from {
            FromTable::WithFromKeyword(t) | FromTable::WithoutKeyword(t) => t,
        };
        for twj in tables {
            self.check_table_factor(&twj.relation, v);
        }
    }

    fn check_table_factor(&self, factor: &TableFactor, v: &mut Vec<WallViolation>) {
        match factor {
            TableFactor::Table { name, .. } => {
                self.check_object_name(name, v);
            }
            TableFactor::Derived { subquery, .. } => {
                self.check_query(subquery, v);
            }
            _ => {}
        }
    }

    fn check_object_name(&self, name: &ObjectName, v: &mut Vec<WallViolation>) {
        let name_str = name.to_string().to_ascii_lowercase();
        let segments = name_str
            .split('.')
            .map(|segment| segment.trim_matches(['`', '"', '[', ']']))
            .collect::<Vec<_>>();
        if segments.len() > 1 {
            let schema = segments[..segments.len() - 1].join(".");
            if self
                .config
                .deny_schemas
                .iter()
                .any(|deny| schema.eq_ignore_ascii_case(deny))
            {
                Self::push_unique(v, WallViolation::DeniedSchema(schema));
            }
        }
        let table = segments.last().copied().unwrap_or(name_str.as_str());
        for deny in &self.config.deny_tables {
            if table.eq_ignore_ascii_case(deny) {
                Self::push_unique(v, WallViolation::DeniedTable(name_str.clone()));
            }
        }
    }

    fn push_unique(violations: &mut Vec<WallViolation>, violation: WallViolation) {
        if !violations.contains(&violation) {
            violations.push(violation);
        }
    }
}

fn is_zero_literal(expression: &Expr) -> bool {
    matches!(expression, Expr::Value(Value::Number(value, _)) if value == "0")
}

fn is_always_true(expression: &Expr) -> bool {
    match expression {
        Expr::Value(Value::Boolean(true)) => true,
        Expr::BinaryOp { left, op, right } if *op == BinaryOperator::Eq => {
            matches!((&**left, &**right), (Expr::Value(left), Expr::Value(right)) if left == right)
        }
        Expr::BinaryOp { left, op, right } if *op == BinaryOperator::And => {
            is_always_true(left) && is_always_true(right)
        }
        Expr::Nested(expression) => is_always_true(expression),
        _ => false,
    }
}

fn contains_disallowed_comment(sql: &str, hint_allow: bool) -> bool {
    let bytes = sql.as_bytes();
    let mut index = 0;
    let mut quote = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == active_quote {
                if index + 1 < bytes.len() && bytes[index + 1] == active_quote {
                    index += 2;
                    continue;
                }
                quote = None;
            } else if byte == b'\\' {
                index = (index + 2).min(bytes.len());
                continue;
            }
            index += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            return true;
        }
        if byte == b'#' {
            return true;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let hint = bytes.get(index + 2) == Some(&b'+');
            if !hint || !hint_allow {
                return true;
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    false
}
