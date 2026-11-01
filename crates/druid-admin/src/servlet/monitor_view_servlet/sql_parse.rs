//! SQL 文本的服务端解析统计。
//!
//! 对应 Java: druid-admin 在 `sql-detail` 响应里附带 `formattedSql` 与
//! `parsedTable` / `parsedFields` / `parsedConditions` /
//! `parsedRelationships` / `parsedOrderbycolumns`（由 SQLParser 生成）。
//! Rust 侧在详情路由返回时按需解析，列表路由不付这个成本。
//! 解析失败（方言不支持、非 SQL 文本）时全部字段返回 `None`，
//! 前端回退到客户端简易解析。

use serde::Serialize;
use serde_json::{json, Value};
use sqlparser::ast::{
    Expr, JoinConstraint, JoinOperator, ObjectName, Query, Select, SelectItem, SetExpr, Statement,
    TableFactor, TableWithJoins,
};
use sqlparser::dialect::{GenericDialect, MySqlDialect, PostgreSqlDialect};
use sqlparser::parser::Parser;

/// 一条 SQL 的解析统计结果。所有字段在解析失败时为 `None`。
///
/// 对应 Java: druid SQLParser 的 SQLStat（SchemaStatVisitor 汇总）。
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SqlParseStat {
    /// 规范化后多行排版的 SQL。
    pub formatted_sql: Option<String>,
    /// 涉及的表名。
    pub parsed_table: Option<Vec<String>>,
    /// 查询输出或写入的字段。
    pub parsed_fields: Option<Vec<String>>,
    /// WHERE 条件（按顶层 AND 拆分）。
    pub parsed_conditions: Option<Vec<String>>,
    /// 表间关联（JOIN ON 条件）。
    pub parsed_relationships: Option<Vec<String>>,
    /// ORDER BY 列。
    pub parsed_orderby_columns: Option<Vec<String>>,
}

impl SqlParseStat {
    /// 序列化为 camelCase JSON，字段名对齐 Java 详情响应。
    pub fn to_json(&self) -> Value {
        json!({
            "formattedSql": self.formatted_sql,
            "parsedTable": self.parsed_table,
            "parsedFields": self.parsed_fields,
            "parsedConditions": self.parsed_conditions,
            "parsedRelationships": self.parsed_relationships,
            "parsedOrderbycolumns": self.parsed_orderby_columns,
        })
    }
}

/// 解析 SQL 文本，产出统计信息。解析失败返回全 `None` 的结构。
pub fn parse_sql_stat(sql: &str) -> SqlParseStat {
    let Some(statements) = parse_any_dialect(sql) else {
        return SqlParseStat::default();
    };
    let mut stat = SqlParseStat {
        formatted_sql: Some(format_statements(&statements)),
        ..SqlParseStat::default()
    };
    for statement in &statements {
        walk_statement(statement, &mut stat);
    }
    stat
}

/// 依次尝试多种方言解析；全部失败返回 `None`。
fn parse_any_dialect(sql: &str) -> Option<Vec<Statement>> {
    let attempts: Vec<Box<dyn sqlparser::dialect::Dialect>> = vec![
        Box::new(GenericDialect {}),
        Box::new(MySqlDialect {}),
        Box::new(PostgreSqlDialect {}),
    ];
    for dialect in attempts {
        if let Ok(statements) = Parser::parse_sql(dialect.as_ref(), sql) {
            return Some(statements);
        }
    }
    None
}

/// 把语句规范化为多行排版：每行一条子句。
fn format_statements(statements: &[Statement]) -> String {
    let normalized = statements
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(";\n");
    break_lines(&normalized)
}

/// 在主要子句关键字前换行（关键字前后须是空格，避免误切列名里的子串）。
fn break_lines(text: &str) -> String {
    const CLAUSES: [&str; 15] = [
        " FROM ",
        " WHERE ",
        " GROUP BY ",
        " HAVING ",
        " ORDER BY ",
        " LIMIT ",
        " OFFSET ",
        " SET ",
        " VALUES ",
        " RETURNING ",
        " UNION ",
        " WINDOW ",
        " INNER JOIN ",
        " CROSS JOIN ",
        " FULL JOIN ",
    ];
    // JOIN 变体先处理（最长前缀优先，避免 LEFT 吃掉 LEFT OUTER）。
    let mut rest = text.trim().to_owned();
    for kw in [
        " LEFT OUTER JOIN ",
        " RIGHT OUTER JOIN ",
        " FULL OUTER JOIN ",
        " LEFT JOIN ",
        " RIGHT JOIN ",
        " INNER JOIN ",
        " CROSS JOIN ",
        " FULL JOIN ",
        " JOIN ",
    ] {
        rest = rest.replace(kw, &format!("\n{} ", kw.trim()));
    }
    // 其余子句关键字在每一行内换行（跳过第一行开头的 SELECT/UPDATE/INSERT/DELETE）。
    let mut out = String::new();
    for (index, chunk) in rest.split('\n').enumerate() {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut line = String::new();
        let mut tail = trimmed.to_owned();
        loop {
            let hit = CLAUSES
                .iter()
                .filter_map(|kw| tail.find(kw).map(|pos| (pos, *kw)))
                .min_by_key(|(pos, _)| *pos);
            match hit {
                Some((pos, kw)) if pos > 0 => {
                    line.push_str(tail[..pos].trim_end());
                    line.push('\n');
                    tail = tail[pos + 1..].to_owned();
                }
                Some((0, kw)) => {
                    // 行首已是子句关键字：保留并继续处理剩余部分。
                    line.push_str(kw.trim());
                    line.push(' ');
                    tail = tail[kw.len()..].to_owned();
                }
                _ => {
                    line.push_str(&tail);
                    break;
                }
            }
        }
        if index > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end());
    }
    out
}

/// 递归收集一条语句里的表、字段、条件、关联与排序列。
fn walk_statement(statement: &Statement, stat: &mut SqlParseStat) {
    match statement {
        Statement::Query(query) => walk_query(query, stat),
        Statement::Update {
            table,
            assignments,
            selection,
            ..
        } => {
            collect_table_with_joins(table, stat);
            // SET 赋值目标计入字段（Java UpdateStat 的 columns 口径）。
            let targets = assignments
                .iter()
                .map(|assignment| assignment.target.to_string())
                .collect::<Vec<_>>();
            push_all(&mut stat.parsed_fields, targets);
            if let Some(selection) = selection {
                let conditions = split_and_conditions(selection);
                push_all(&mut stat.parsed_conditions, conditions);
            }
        }
        Statement::Insert(insert) => {
            collect_object_name(&insert.table_name, stat);
            if let Some(source) = &insert.source {
                walk_query(source, stat);
            }
        }
        Statement::Delete(delete) => {
            let from_tables = match &delete.from {
                sqlparser::ast::FromTable::WithFromKeyword(tables)
                | sqlparser::ast::FromTable::WithoutKeyword(tables) => tables,
            };
            for table in from_tables {
                collect_table_with_joins(table, stat);
            }
            if let Some(selection) = &delete.selection {
                let conditions = split_and_conditions(selection);
                push_all(&mut stat.parsed_conditions, conditions);
            }
        }
        _ => {}
    }
}

/// 遍历查询：ORDER BY、WITH 子查询、SELECT 主体。
fn walk_query(query: &Query, stat: &mut SqlParseStat) {
    if let Some(order_by) = &query.order_by {
        for item in &order_by.exprs {
            let direction = if item.asc.unwrap_or(true) {
                ""
            } else {
                " DESC"
            };
            push_all(
                &mut stat.parsed_orderby_columns,
                vec![format!("{}{}", item.expr, direction)],
            );
        }
    }
    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            walk_query(&cte.query, stat);
        }
    }
    walk_set_expr(&query.body, stat);
}

fn walk_set_expr(expr: &SetExpr, stat: &mut SqlParseStat) {
    match expr {
        SetExpr::Select(select) => walk_select(select, stat),
        SetExpr::Query(inner) => walk_query(inner, stat),
        SetExpr::SetOperation { left, right, .. } => {
            walk_set_expr(left, stat);
            walk_set_expr(right, stat);
        }
        _ => {}
    }
}

fn walk_select(select: &Select, stat: &mut SqlParseStat) {
    for item in &select.projection {
        match item {
            SelectItem::UnnamedExpr(expr) => {
                push_all(&mut stat.parsed_fields, vec![expr.to_string()])
            }
            SelectItem::ExprWithAlias { expr, alias } => push_all(
                &mut stat.parsed_fields,
                vec![format!("{} AS {}", expr, alias)],
            ),
            SelectItem::QualifiedWildcard(name, _) => push_all(
                &mut stat.parsed_fields,
                vec![format!("{}.*", object_name_text(name))],
            ),
            SelectItem::Wildcard(_) => push_all(&mut stat.parsed_fields, vec!["*".to_owned()]),
        }
    }
    for table in &select.from {
        collect_table_with_joins(table, stat);
    }
    if let Some(selection) = &select.selection {
        let conditions = split_and_conditions(selection);
        push_all(&mut stat.parsed_conditions, conditions);
    }
}

/// 收集 FROM 表与其所有 JOIN 的表和 ON 关联条件。
fn collect_table_with_joins(table: &TableWithJoins, stat: &mut SqlParseStat) {
    collect_table_factor(&table.relation, stat);
    for join in &table.joins {
        collect_table_factor(&join.relation, stat);
        let constraint = match &join.join_operator {
            JoinOperator::Inner(c)
            | JoinOperator::LeftOuter(c)
            | JoinOperator::RightOuter(c)
            | JoinOperator::FullOuter(c)
            | JoinOperator::LeftSemi(c)
            | JoinOperator::RightSemi(c)
            | JoinOperator::LeftAnti(c)
            | JoinOperator::RightAnti(c) => Some(c),
            _ => None,
        };
        if let Some(JoinConstraint::On(expr)) = constraint {
            push_all(&mut stat.parsed_relationships, vec![expr.to_string()]);
        }
        if let Some(JoinConstraint::Using(columns)) = constraint {
            push_all(
                &mut stat.parsed_relationships,
                vec![format!(
                    "USING ({})",
                    columns
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )],
            );
        }
    }
}

fn collect_table_factor(factor: &TableFactor, stat: &mut SqlParseStat) {
    match factor {
        TableFactor::Table { name, .. } => collect_object_name(name, stat),
        TableFactor::Derived { subquery, .. } => walk_query(subquery, stat),
        _ => {}
    }
}

fn collect_object_name(name: &ObjectName, stat: &mut SqlParseStat) {
    push_all(&mut stat.parsed_table, vec![object_name_text(name)]);
}

fn object_name_text(name: &ObjectName) -> String {
    name.0
        .iter()
        .map(|part| part.value.clone())
        .collect::<Vec<_>>()
        .join(".")
}

/// 把 WHERE 条件按顶层 AND 拆成单条件列表（Java 的 ConditionList 口径）。
fn split_and_conditions(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::BinaryOp {
            left,
            op: sqlparser::ast::BinaryOperator::And,
            right,
        } => {
            let mut out = split_and_conditions(left);
            out.extend(split_and_conditions(right));
            out
        }
        other => vec![other.to_string()],
    }
}

/// 去重追加，保持首次出现顺序。
fn push_all(target: &mut Option<Vec<String>>, items: Vec<String>) {
    let entry = target.get_or_insert_with(Vec::new);
    for item in items {
        if !entry.contains(&item) {
            entry.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_with_join_parses_full_stat() {
        let stat = parse_sql_stat(
            "select o.id, i.name from orders o join order_items i on o.id = i.order_id where o.status = ? and i.qty > 2 order by o.created_at desc",
        );
        assert!(stat.formatted_sql.is_some());
        assert_eq!(
            stat.parsed_table.as_deref(),
            Some(&["orders".to_owned(), "order_items".to_owned()][..])
        );
        assert!(stat
            .parsed_fields
            .as_ref()
            .unwrap()
            .contains(&"o.id".to_owned()));
        assert!(stat
            .parsed_fields
            .as_ref()
            .unwrap()
            .contains(&"i.name".to_owned()));
        assert_eq!(
            stat.parsed_conditions.as_deref(),
            Some(&["o.status = ?".to_owned(), "i.qty > 2".to_owned()][..])
        );
        assert_eq!(
            stat.parsed_relationships.as_deref(),
            Some(&["o.id = i.order_id".to_owned()][..])
        );
        assert_eq!(
            stat.parsed_orderby_columns.as_deref(),
            Some(&["o.created_at DESC".to_owned()][..])
        );
    }

    #[test]
    fn update_parses_table_fields_conditions() {
        let stat = parse_sql_stat("update orders set status = ? where id = ?");
        assert_eq!(
            stat.parsed_table.as_deref(),
            Some(&["orders".to_owned()][..])
        );
        assert!(stat
            .parsed_fields
            .as_ref()
            .unwrap()
            .contains(&"status".to_owned()));
        assert_eq!(
            stat.parsed_conditions.as_deref(),
            Some(&["id = ?".to_owned()][..])
        );
    }

    #[test]
    fn insert_parses_table_and_source() {
        let stat = parse_sql_stat(
            "insert into audit_log (actor, action) select actor, action from staging_audit",
        );
        assert!(stat
            .parsed_table
            .as_ref()
            .unwrap()
            .contains(&"audit_log".to_owned()));
        assert!(stat
            .parsed_table
            .as_ref()
            .unwrap()
            .contains(&"staging_audit".to_owned()));
    }

    #[test]
    fn unparseable_text_returns_none_fields() {
        let stat = parse_sql_stat("not a sql at all !!!");
        assert!(stat.formatted_sql.is_none());
        assert!(stat.parsed_table.is_none());
        assert!(stat.parsed_conditions.is_none());
    }

    #[test]
    fn mysql_dialect_fallback_parses_backticks() {
        let stat = parse_sql_stat("select `id` from `orders` where `status` = 1");
        assert_eq!(
            stat.parsed_table.as_deref(),
            Some(&["orders".to_owned()][..])
        );
        assert!(stat.parsed_fields.is_some());
    }
}
