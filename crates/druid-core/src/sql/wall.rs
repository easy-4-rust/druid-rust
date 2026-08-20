//! 对应 Java 类：`com.alibaba.druid.wall.WallProvider` + `WallVisitor`
//!
//! SQL 防火墙，基于 `sqlparser-rs` AST 检查 SQL 安全性。

use super::wall_config::WallConfig;
use super::wall_violation::WallViolation;
use super::{DbType, SqlUtils, WallContext, WallUpdateCheckItem};
use parking_lot::RwLock;
use sqlparser::ast::{
    visit_expressions, AssignmentTarget, BinaryOperator, DescribeAlias, Expr, FromTable,
    ObjectName, ObjectType, Query, Select, SelectItem, SetExpr, SetOperator, Statement,
    TableFactor, Value,
};
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, Tokenizer, Whitespace};
use std::collections::HashMap;
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
        let (violations, _) = self.check_parsed(sql, &ast);
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    pub(crate) fn check_parsed(
        &self,
        sql: &str,
        ast: &[Statement],
    ) -> (Vec<WallViolation>, Option<Vec<WallUpdateCheckItem>>) {
        if !self.config.comment_allow && contains_disallowed_comment(sql, self.config.hint_allow) {
            return (
                vec![WallViolation::OperationNotAllowed("COMMENT".to_owned())],
                None,
            );
        }
        if self.config.comment_allow {
            self.collect_comment_warnings(sql);
        }
        let mut violations = Vec::new();
        if ast.len() > 1 && !self.config.multi_statement_allow {
            violations.push(WallViolation::MultiStatementNotAllowed);
        }
        let mut update_check_items = Vec::new();
        for stmt in ast {
            self.check_statement(stmt, &mut violations);
            Self::collect_wall_context_warnings(stmt);
            self.collect_update_check_items(stmt, &mut violations, &mut update_check_items);
            let _: ControlFlow<()> = visit_expressions(stmt, |expression| {
                if self.config.function_check {
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
                }
                if self.config.variant_check {
                    if let Expr::Identifier(identifier) = expression {
                        if identifier.value.starts_with('@') {
                            let variant_name = identifier
                                .value
                                .trim_start_matches('@')
                                .to_ascii_lowercase();
                            if self.config.deny_variants.iter().any(|deny| {
                                deny.eq_ignore_ascii_case(&identifier.value)
                                    || deny.eq_ignore_ascii_case(&variant_name)
                            }) {
                                Self::push_unique(
                                    &mut violations,
                                    WallViolation::DeniedVariant(identifier.value.clone()),
                                );
                            }
                        }
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
        let update_check_items = if update_check_items.is_empty() {
            None
        } else {
            Some(update_check_items)
        };
        (violations, update_check_items)
    }

    fn check_statement(&self, stmt: &Statement, v: &mut Vec<WallViolation>) {
        match stmt {
            Statement::Query(query) => {
                if !self.config.select_allow {
                    v.push(WallViolation::OperationNotAllowed("SELECT".to_owned()));
                }
                self.check_query(query, v);
            }
            Statement::Delete(delete) => self.check_delete_statement(delete, v),
            Statement::Update {
                table, selection, ..
            } => self.check_update_statement(table, selection.as_ref(), v),
            Statement::Insert(insert) => {
                if !self.config.insert_allow {
                    v.push(WallViolation::OperationNotAllowed("INSERT".to_owned()));
                }
                self.check_object_name(&insert.table_name, v);
                self.check_read_only(&insert.table_name, v);
            }
            Statement::Drop {
                object_type, names, ..
            } => {
                if !self.config.drop_table_allow {
                    if *object_type == ObjectType::Table {
                        for name in names {
                            v.push(WallViolation::DropTableNotAllowed(name.to_string()));
                        }
                    } else {
                        v.push(WallViolation::OperationNotAllowed(format!(
                            "DROP {object_type}"
                        )));
                    }
                }
            }
            Statement::Truncate { table_names, .. } => {
                if !self.config.truncate_allow {
                    v.push(WallViolation::TruncateNotAllowed);
                }
                for target in table_names {
                    self.check_object_name(&target.name, v);
                    self.check_read_only(&target.name, v);
                }
            }
            Statement::CreateTable(_)
            | Statement::AlterTable { .. }
            | Statement::Commit { .. }
            | Statement::Rollback { .. }
            | Statement::StartTransaction { .. }
            | Statement::SetVariable { .. }
            | Statement::Use(_)
            | Statement::Call(_)
            | Statement::ShowFunctions { .. }
            | Statement::ShowVariable { .. }
            | Statement::ShowStatus { .. }
            | Statement::ShowVariables { .. }
            | Statement::ShowCreate { .. }
            | Statement::ShowColumns { .. }
            | Statement::ShowDatabases { .. }
            | Statement::ShowSchemas { .. }
            | Statement::ShowTables { .. }
            | Statement::ShowViews { .. }
            | Statement::ShowCollation { .. } => {
                if let Some(violation) = self.statement_gate(stmt) {
                    v.push(violation);
                }
            }
            // Java preVisitCheck：SQLExplainStatement → allow=true（无需配置）。
            Statement::Explain { .. } => {}
            Statement::ExplainTable {
                describe_alias: DescribeAlias::Desc | DescribeAlias::Describe,
                ..
            } => {
                if !self.config.describe_allow {
                    v.push(WallViolation::OperationNotAllowed("DESCRIBE".to_owned()));
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

    /// DELETE 语句门控 + WHERE 语义 + 只读表检查。
    ///
    /// 对应 Java `WallVisitorUtils#checkDelete`。
    fn check_delete_statement(&self, delete: &sqlparser::ast::Delete, v: &mut Vec<WallViolation>) {
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
        self.check_condition_opt(delete.selection.as_ref(), v);
        self.check_from_table(&delete.from, v);
        for table_with_joins in tables_of(&delete.from) {
            if let TableFactor::Table { name, .. } = &table_with_joins.relation {
                self.check_read_only(name, v);
            }
        }
    }

    /// UPDATE 语句门控 + WHERE 语义 + 只读表检查。
    ///
    /// 对应 Java `WallVisitorUtils#checkUpdate`。
    fn check_update_statement(
        &self,
        table: &sqlparser::ast::TableWithJoins,
        selection: Option<&Expr>,
        v: &mut Vec<WallViolation>,
    ) {
        if !self.config.update_allow {
            v.push(WallViolation::OperationNotAllowed("UPDATE".to_owned()));
        }
        if selection.is_none() && self.config.update_must_have_where {
            v.push(WallViolation::UpdateWithoutWhere);
        }
        if self.config.update_where_alway_true_check && selection.is_some_and(is_always_true) {
            Self::push_unique(
                v,
                WallViolation::AlwaysTrueCondition("UPDATE WHERE".to_owned()),
            );
        }
        self.check_condition_opt(selection, v);
        self.check_table_factor(&table.relation, v);
        if let TableFactor::Table { name, .. } = &table.relation {
            self.check_read_only(name, v);
        }
    }

    /// 单一布尔开关语句门控；对应 Java `preVisitCheck` 的 allow 位查表。
    fn statement_gate(&self, stmt: &Statement) -> Option<WallViolation> {
        let (allow, name): (bool, &str) = match stmt {
            Statement::CreateTable(_) => (self.config.create_table_allow, "CREATE TABLE"),
            Statement::AlterTable { .. } => (self.config.alter_table_allow, "ALTER TABLE"),
            Statement::Commit { .. } => (self.config.commit_allow, "COMMIT"),
            Statement::Rollback { .. } => (self.config.rollback_allow, "ROLLBACK"),
            Statement::StartTransaction { .. } => {
                (self.config.start_transaction_allow, "START TRANSACTION")
            }
            Statement::SetVariable { .. } => (self.config.set_allow, "SET"),
            Statement::Use(_) => (self.config.use_allow, "USE"),
            Statement::Call(_) => (self.config.call_allow, "CALL"),
            _ => (self.config.show_allow, "SHOW"),
        };
        (!allow).then(|| WallViolation::OperationNotAllowed(name.to_owned()))
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
            SetExpr::SetOperation {
                op, left, right, ..
            } => {
                if *op == SetOperator::Intersect && !self.config.intersect_allow {
                    Self::push_unique(
                        v,
                        WallViolation::OperationNotAllowed("INTERSECT".to_owned()),
                    );
                }
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
        self.check_condition_opt(select.selection.as_ref(), v);
        self.check_condition_opt(select.having.as_ref(), v);
        // 检查 FROM 子句中的表
        for table_with_joins in &select.from {
            self.check_table_factor(&table_with_joins.relation, v);
        }
    }

    /// 对可选条件表达式执行 Java `WallVisitorUtils#checkCondition` 语义检查。
    fn check_condition_opt(&self, expression: Option<&Expr>, v: &mut Vec<WallViolation>) {
        if let Some(expression) = expression {
            self.check_condition(expression, v);
        }
    }

    /// 迁移 Java `getConditionValue`/`getValue_and` 的条件语义检查族。
    ///
    /// 覆盖 `conditionAndAlwayTrueAllow`、`conditionAndAlwayFalseAllow`、
    /// `conditionDoubleConstAllow`、`conditionOpXorAllow`、
    /// `conditionOpBitwiseAllow`、`constArithmeticAllow`、
    /// `conditionLikeTrueAllow`（same-const like）与 `caseConditionConstAllow`。
    fn check_condition(&self, expression: &Expr, v: &mut Vec<WallViolation>) {
        let mut parts = Vec::new();
        split_boolean_and(expression, &mut parts);
        let mut consecutive_const = 0_usize;
        for (index, part) in parts.iter().enumerate() {
            match const_bool_value(part) {
                Some(true) => {
                    if index > 0 && !self.config.condition_and_alway_true_allow {
                        Self::push_unique(v, WallViolation::AlwaysTrueCondition("part".to_owned()));
                    }
                    consecutive_const += 1;
                }
                Some(false) => {
                    if index > 0 && !self.config.condition_and_alway_false_allow {
                        Self::push_unique(
                            v,
                            WallViolation::AlwaysFalseCondition("part".to_owned()),
                        );
                    }
                    consecutive_const += 1;
                }
                None => consecutive_const = 0,
            }
            if consecutive_const == 2 && !self.config.condition_double_const_allow {
                Self::push_unique(v, WallViolation::DoubleConstCondition);
            }
        }
        let _: ControlFlow<()> = visit_expressions(expression, |expr| {
            if let Expr::BinaryOp { left, op, right } = expr {
                match op {
                    BinaryOperator::Xor if !self.config.condition_op_xor_allow => {
                        Self::push_unique(v, WallViolation::XorNotAllowed);
                    }
                    BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::PGBitwiseXor
                    | BinaryOperator::PGBitwiseShiftLeft
                    | BinaryOperator::PGBitwiseShiftRight
                        if !self.config.condition_op_bitwise_allow =>
                    {
                        Self::push_unique(v, WallViolation::BitwiseNotAllowed);
                    }
                    BinaryOperator::Plus
                    | BinaryOperator::Minus
                    | BinaryOperator::Multiply
                    | BinaryOperator::Modulo
                    | BinaryOperator::Divide
                        if !self.config.const_arithmetic_allow
                            && is_const_expr(left)
                            && is_const_expr(right) =>
                    {
                        Self::push_unique(v, WallViolation::ConstArithmeticNotAllowed);
                    }
                    _ => {}
                }
            }
            if let Expr::Like { expr, pattern, .. } = expr {
                if let (
                    Expr::Value(
                        Value::SingleQuotedString(left_value)
                        | Value::DoubleQuotedString(left_value),
                    ),
                    Expr::Value(
                        Value::SingleQuotedString(right_value)
                        | Value::DoubleQuotedString(right_value),
                    ),
                ) = (expr.as_ref(), pattern.as_ref())
                {
                    if left_value == right_value {
                        Self::push_unique(v, WallViolation::SameConstLike);
                    }
                }
            }
            if !self.config.case_condition_const_allow {
                if let Expr::Case {
                    operand: None,
                    conditions,
                    ..
                } = expr
                {
                    if conditions.iter().any(is_const_bool_true_expr) {
                        Self::push_unique(v, WallViolation::ConstCaseCondition);
                    }
                }
            }
            ControlFlow::Continue(())
        });
    }

    /// 检查写入目标是否命中只读表清单。
    ///
    /// 对应 Java `WallVisitorUtils#checkReadOnly` + `WallConfig#isReadOnly`。
    fn check_read_only(&self, name: &ObjectName, v: &mut Vec<WallViolation>) {
        let table = simple_name(name);
        if table.is_empty() {
            return;
        }
        if self
            .config
            .read_only_tables
            .iter()
            .any(|deny| deny.eq_ignore_ascii_case(&table))
        {
            Self::push_unique(v, WallViolation::ReadOnlyTable(name.to_string()));
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
        if self.config.schema_check && segments.len() > 1 {
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
        if self.config.object_check {
            for deny in &self.config.deny_objects {
                if name_str.eq_ignore_ascii_case(deny) {
                    Self::push_unique(v, WallViolation::DeniedObject(name_str.clone()));
                }
            }
        }
        if self.config.table_check {
            let table = segments.last().copied().unwrap_or(name_str.as_str());
            for deny in &self.config.deny_tables {
                if table.eq_ignore_ascii_case(deny) {
                    Self::push_unique(v, WallViolation::DeniedTable(name_str.clone()));
                }
            }
        }
    }

    fn push_unique(violations: &mut Vec<WallViolation>, violation: WallViolation) {
        if !violations.contains(&violation) {
            violations.push(violation);
        }
    }

    fn collect_wall_context_warnings(statement: &Statement) {
        let Some(context) = WallContext::current() else {
            return;
        };
        let mut context = context.lock();
        match statement {
            Statement::Update {
                selection: None, ..
            } => context.increment_update_none_condition_warnings(),
            Statement::Delete(delete)
                if delete.selection.is_none()
                    && delete.using.is_none()
                    && !from_has_join(&delete.from) =>
            {
                context.increment_delete_none_condition_warnings();
            }
            _ => {}
        }

        let _: ControlFlow<()> = visit_expressions(statement, |expression| {
            if let Expr::Like { expr, pattern, .. } = expression {
                if is_number_literal(expr) || is_number_literal(pattern) {
                    context.increment_like_number_warnings();
                }
            }
            ControlFlow::Continue(())
        });
    }

    fn collect_comment_warnings(&self, sql: &str) {
        let Some(context) = WallContext::current() else {
            return;
        };
        let dialect = SqlUtils::dialect(self.db_type());
        let Ok(tokens) = Tokenizer::new(dialect.as_ref(), sql).tokenize() else {
            return;
        };
        let mut previous = None;
        let mut count = 0_u32;
        for token in tokens {
            match &token {
                Token::Whitespace(
                    Whitespace::SingleLineComment { .. } | Whitespace::MultiLineComment(_),
                ) => {
                    if previous
                        .as_ref()
                        .is_some_and(|previous| !comment_is_accepted_after(previous))
                    {
                        count = count.wrapping_add(1);
                    }
                }
                Token::Whitespace(_) | Token::EOF => {}
                _ => previous = Some(token),
            }
        }
        let mut context = context.lock();
        for _ in 0..count {
            context.increment_comment_count();
        }
    }

    fn collect_update_check_items(
        &self,
        statement: &Statement,
        violations: &mut Vec<WallViolation>,
        update_check_items: &mut Vec<WallUpdateCheckItem>,
    ) {
        let Statement::Update {
            table,
            assignments,
            selection: Some(selection),
            ..
        } = statement
        else {
            return;
        };
        let TableFactor::Table { name, .. } = &table.relation else {
            return;
        };
        let Some(handler) = self.config.update_check_handler() else {
            return;
        };
        let table_name = simple_name(name);
        let Some(check_column) = self
            .config
            .update_check_table(&table_name)
            .and_then(|columns| columns.first().cloned())
        else {
            return;
        };
        let Some(value_expression) = assignments.iter().find_map(|assignment| {
            let AssignmentTarget::ColumnName(column) = &assignment.target else {
                return None;
            };
            simple_name(column)
                .eq_ignore_ascii_case(&check_column)
                .then_some(&assignment.value)
        }) else {
            return;
        };

        let placeholder_indices = placeholder_indices(statement);
        let mut conditions = Vec::new();
        split_boolean_and(selection, &mut conditions);
        let mut filter_value_expressions = Vec::new();
        for condition in conditions {
            match condition {
                Expr::BinaryOp {
                    left,
                    op: BinaryOperator::Eq,
                    right,
                } if expression_contains_column(condition, &check_column) => {
                    if is_literal_or_placeholder(left) {
                        filter_value_expressions.push(left.as_ref());
                    } else if is_literal_or_placeholder(right) {
                        filter_value_expressions.push(right.as_ref());
                    }
                }
                Expr::InList {
                    expr,
                    list,
                    negated: false,
                } if expression_is_column(expr, &check_column) => {
                    filter_value_expressions.extend(list);
                }
                _ => {}
            }
        }
        let value_parameter_index =
            expression_parameter_index(value_expression, &placeholder_indices);
        let filter_parameter_indices = filter_value_expressions
            .iter()
            .map(|expression| expression_parameter_index(expression, &placeholder_indices))
            .collect();
        let filter_values = filter_value_expressions
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let item = WallUpdateCheckItem::with_parameter_indices(
            table_name,
            check_column,
            value_expression.clone(),
            value_parameter_index,
            filter_values,
            filter_parameter_indices,
        );
        if let Some((set_value, filter_values)) = item.literal_values() {
            if !handler.check(
                &item.table_name,
                &item.column_name,
                &set_value,
                &filter_values,
            ) {
                Self::push_unique(violations, WallViolation::UpdateCheckFailed);
            }
        } else {
            update_check_items.push(item);
        }
    }
}

/// 返回 FROM 子句中全部表连接的引用。
fn tables_of(from: &FromTable) -> &[sqlparser::ast::TableWithJoins] {
    match from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    }
}

fn from_has_join(from: &FromTable) -> bool {
    let tables = match from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    };
    tables.iter().any(|table| !table.joins.is_empty())
}

fn comment_is_accepted_after(token: &Token) -> bool {
    let Token::Word(word) = token else {
        return false;
    };
    if word.quote_style.is_some() {
        return false;
    }
    matches!(
        word.value.to_ascii_uppercase().as_str(),
        "SELECT"
            | "INSERT"
            | "DELETE"
            | "UPDATE"
            | "TRUNCATE"
            | "SET"
            | "CREATE"
            | "ALTER"
            | "DROP"
            | "SHOW"
            | "REPLACE"
    )
}

fn is_number_literal(expression: &Expr) -> bool {
    matches!(expression, Expr::Value(Value::Number(_, _)))
}

fn simple_name(name: &ObjectName) -> String {
    name.0
        .last()
        .map(|identifier| {
            identifier
                .value
                .trim_matches(['`', '"', '[', ']'])
                .to_lowercase()
        })
        .unwrap_or_default()
}

fn split_boolean_and<'a>(expression: &'a Expr, conditions: &mut Vec<&'a Expr>) {
    match expression {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => {
            split_boolean_and(left, conditions);
            split_boolean_and(right, conditions);
        }
        Expr::Nested(expression) => split_boolean_and(expression, conditions),
        _ => conditions.push(expression),
    }
}

fn expression_contains_column(expression: &Expr, column_name: &str) -> bool {
    let mut found = false;
    let _: ControlFlow<()> = visit_expressions(expression, |candidate| {
        if expression_is_column(candidate, column_name) {
            found = true;
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    found
}

fn expression_is_column(expression: &Expr, column_name: &str) -> bool {
    match expression {
        Expr::Identifier(identifier) => identifier.value.eq_ignore_ascii_case(column_name),
        Expr::CompoundIdentifier(identifiers) => identifiers
            .last()
            .is_some_and(|identifier| identifier.value.eq_ignore_ascii_case(column_name)),
        _ => false,
    }
}

fn is_literal_or_placeholder(expression: &Expr) -> bool {
    matches!(expression, Expr::Value(_))
}

fn placeholder_indices(statement: &Statement) -> HashMap<usize, usize> {
    let mut indices = HashMap::new();
    let mut occurrence = 0_usize;
    let _: ControlFlow<()> = visit_expressions(statement, |expression| {
        if let Expr::Value(Value::Placeholder(placeholder)) = expression {
            occurrence += 1;
            let index = explicit_parameter_index(placeholder).unwrap_or(occurrence);
            indices.insert(std::ptr::from_ref::<Expr>(expression) as usize, index);
        }
        ControlFlow::Continue(())
    });
    indices
}

fn expression_parameter_index(
    expression: &Expr,
    placeholder_indices: &HashMap<usize, usize>,
) -> Option<usize> {
    if !matches!(expression, Expr::Value(Value::Placeholder(_))) {
        return None;
    }
    placeholder_indices
        .get(&(std::ptr::from_ref::<Expr>(expression) as usize))
        .copied()
        .or_else(|| {
            let Expr::Value(Value::Placeholder(placeholder)) = expression else {
                return None;
            };
            explicit_parameter_index(placeholder)
        })
}

fn explicit_parameter_index(placeholder: &str) -> Option<usize> {
    placeholder
        .strip_prefix('$')
        .or_else(|| placeholder.strip_prefix('?'))
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<usize>().ok())
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

/// 求常量表达式的布尔值；非常量返回 `None`。
///
/// 对应 Java `SQLEvalVisitorUtils#castToBoolean(getValue(...))` 的常量子集。
fn const_bool_value(expression: &Expr) -> Option<bool> {
    match expression {
        Expr::Value(Value::Boolean(value)) => Some(*value),
        Expr::Value(Value::Number(value, _)) => value.parse::<f64>().ok().map(|n| n != 0.0),
        Expr::Nested(expression) => const_bool_value(expression),
        Expr::BinaryOp { left, op, right } => match op {
            BinaryOperator::Eq => match (const_scalar(left), const_scalar(right)) {
                (Some(left), Some(right)) => Some(left == right),
                _ => None,
            },
            BinaryOperator::NotEq => match (const_scalar(left), const_scalar(right)) {
                (Some(left), Some(right)) => Some(left != right),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn const_scalar(expression: &Expr) -> Option<Value> {
    match expression {
        Expr::Value(
            value @ (Value::Boolean(_)
            | Value::Number(_, _)
            | Value::SingleQuotedString(_)
            | Value::DoubleQuotedString(_)),
        ) => Some(value.clone()),
        Expr::Nested(expression) => const_scalar(expression),
        _ => None,
    }
}

/// 判断表达式是否为纯常量（字面量或常量的算术组合）。
fn is_const_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Value(
            Value::Boolean(_)
            | Value::Number(_, _)
            | Value::SingleQuotedString(_)
            | Value::DoubleQuotedString(_),
        ) => true,
        Expr::Nested(expression) => is_const_expr(expression),
        Expr::BinaryOp { left, right, .. } => is_const_expr(left) && is_const_expr(right),
        _ => false,
    }
}

fn is_const_bool_true_expr(expression: &Expr) -> bool {
    const_bool_value(expression) == Some(true)
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
