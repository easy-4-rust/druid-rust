//! Wall Visitor 公共多租户改写算法。
//!
//! 对应 Java：`com.alibaba.druid.wall.spi.WallVisitorUtils` 中
//! `checkSelectForMultiTenant`、`checkUpdateForMultiTenant`、
//! `checkInsertForMultiTenant` 与 `generateTenantValue`。

use super::{TenantStatementType, WallConfig, WallProvider};
use crate::core::{DruidError, Value as JdbcValue};
use sqlparser::ast::{
    Assignment, AssignmentTarget, Expr, Ident, ObjectName, Query, Select, SelectItem, SetExpr,
    Statement, TableFactor, TableWithJoins, Value as SqlValue,
};

/// 与具体 SQL 方言 Visitor 解耦的 Wall 公共改写算法。
#[derive(Debug, Default, Clone, Copy)]
pub struct WallVisitorUtils;

impl WallVisitorUtils {
    /// 按 Java Wall 多租户规则原地改写顶层 statement，返回是否发生修改。
    ///
    /// SELECT 为匹配表追加租户投影；UPDATE 追加租户赋值；INSERT 同时追加目标列
    /// 和每个 VALUES/SELECT 分支的租户值。当前 Java 活跃路径未对 DELETE 注入
    /// 条件，因此 Rust 不擅自增加该行为。
    pub fn rewrite_for_multi_tenant(
        statements: &mut [Statement],
        config: &WallConfig,
    ) -> Result<bool, DruidError> {
        if config.tenant_call_back().is_none() && config.tenant_table_pattern.is_empty() {
            return Ok(false);
        }
        let mut modified = false;
        for statement in statements {
            modified |= match statement {
                Statement::Query(query) => Self::rewrite_query(query, config)?,
                Statement::Update {
                    table, assignments, ..
                } => Self::rewrite_update(table, assignments, config)?,
                Statement::Insert(insert) => Self::rewrite_insert(insert, config)?,
                _ => false,
            };
        }
        Ok(modified)
    }

    fn rewrite_query(query: &mut Query, config: &WallConfig) -> Result<bool, DruidError> {
        Self::rewrite_select_set_expr(query.body.as_mut(), config)
    }

    fn rewrite_select_set_expr(
        set_expr: &mut SetExpr,
        config: &WallConfig,
    ) -> Result<bool, DruidError> {
        match set_expr {
            SetExpr::Select(select) => Self::rewrite_select(select, config),
            SetExpr::Query(query) => Self::rewrite_query(query, config),
            SetExpr::SetOperation { left, right, .. } => {
                Ok(Self::rewrite_select_set_expr(left, config)?
                    | Self::rewrite_select_set_expr(right, config)?)
            }
            _ => Ok(false),
        }
    }

    fn rewrite_select(select: &mut Select, config: &WallConfig) -> Result<bool, DruidError> {
        let Some(from) = select.from.first() else {
            return Ok(false);
        };
        let mut projections = Vec::new();

        if let Some((table_name, qualifier)) = table_identity(&from.relation, false) {
            if let Some(tenant_column) =
                tenant_column(config, TenantStatementType::Select, table_name)
            {
                projections.push(SelectItem::UnnamedExpr(column_expr(
                    qualifier,
                    &tenant_column,
                )));
            }
        }

        for join in &from.joins {
            if let Some((table_name, qualifier)) = table_identity(&join.relation, true) {
                if let Some(tenant_column) =
                    tenant_column(config, TenantStatementType::Select, table_name)
                {
                    projections.push(SelectItem::UnnamedExpr(column_expr(
                        qualifier,
                        &tenant_column,
                    )));
                }
            }
        }

        let modified = !projections.is_empty();
        select.projection.extend(projections);
        Ok(modified)
    }

    fn rewrite_update(
        table: &TableWithJoins,
        assignments: &mut Vec<Assignment>,
        config: &WallConfig,
    ) -> Result<bool, DruidError> {
        if !table.joins.is_empty() {
            return Ok(false);
        }
        let Some((table_name, qualifier)) = table_identity(&table.relation, false) else {
            return Ok(false);
        };
        let Some(tenant_column) = tenant_column(config, TenantStatementType::Update, table_name)
        else {
            return Ok(false);
        };
        let tenant_value = tenant_value(config, TenantStatementType::Update, table_name)?;
        assignments.push(Assignment {
            target: AssignmentTarget::ColumnName(column_name(qualifier, &tenant_column)),
            value: tenant_value,
        });
        Ok(true)
    }

    fn rewrite_insert(
        insert: &mut sqlparser::ast::Insert,
        config: &WallConfig,
    ) -> Result<bool, DruidError> {
        let Some(table_name) = simple_object_name(&insert.table_name) else {
            return Ok(false);
        };
        let Some(tenant_column) = tenant_column(config, TenantStatementType::Insert, table_name)
        else {
            return Ok(false);
        };
        let tenant_value = tenant_value(config, TenantStatementType::Insert, table_name)?;
        insert.columns.push(Ident::new(tenant_column));

        if let Some(source) = insert.source.as_mut() {
            append_insert_value(source.body.as_mut(), &tenant_value);
        }
        Ok(true)
    }
}

fn table_identity(table: &TableFactor, default_to_table: bool) -> Option<(&str, Option<&str>)> {
    let TableFactor::Table {
        name, alias, args, ..
    } = table
    else {
        return None;
    };
    if args.is_some() {
        return None;
    }
    let table_name = simple_object_name(name)?;
    let qualifier = alias
        .as_ref()
        .map(|alias| alias.name.value.as_str())
        .or(default_to_table.then_some(table_name));
    Some((table_name, qualifier))
}

fn simple_object_name(name: &ObjectName) -> Option<&str> {
    (name.0.len() == 1).then(|| name.0[0].value.as_str())
}

fn tenant_column(
    config: &WallConfig,
    statement_type: TenantStatementType,
    table_name: &str,
) -> Option<String> {
    let callback_column = config
        .tenant_call_back()
        .and_then(|callback| callback.tenant_column(statement_type, table_name))
        .filter(|column| !column.is_empty());
    callback_column.or_else(|| {
        (servlet_path_matches(config.tenant_table_pattern.as_str(), table_name)
            && !config.tenant_column.is_empty())
        .then(|| config.tenant_column.clone())
    })
}

fn tenant_value(
    config: &WallConfig,
    statement_type: TenantStatementType,
    table_name: &str,
) -> Result<Expr, DruidError> {
    let value = config
        .tenant_call_back()
        .and_then(|callback| callback.tenant_value(statement_type, table_name))
        .or_else(WallProvider::tenant_value)
        .ok_or_else(|| DruidError::Other("tenant value not support type null".to_owned()))?;
    match value {
        JdbcValue::Int(value) => Ok(Expr::Value(SqlValue::Number(value.to_string(), false))),
        JdbcValue::Float(value) if value.is_finite() => {
            Ok(Expr::Value(SqlValue::Number(value.to_string(), false)))
        }
        JdbcValue::Decimal(value) => Ok(Expr::Value(SqlValue::Number(value.to_string(), false))),
        JdbcValue::String(value) => Ok(Expr::Value(SqlValue::SingleQuotedString(value))),
        value => Err(DruidError::Other(format!(
            "tenant value not support type {value:?}"
        ))),
    }
}

fn column_expr(qualifier: Option<&str>, column: &str) -> Expr {
    qualifier.map_or_else(
        || Expr::Identifier(Ident::new(column)),
        |qualifier| Expr::CompoundIdentifier(vec![Ident::new(qualifier), Ident::new(column)]),
    )
}

fn column_name(qualifier: Option<&str>, column: &str) -> ObjectName {
    ObjectName(qualifier.map_or_else(
        || vec![Ident::new(column)],
        |qualifier| vec![Ident::new(qualifier), Ident::new(column)],
    ))
}

fn append_insert_value(set_expr: &mut SetExpr, tenant_value: &Expr) {
    match set_expr {
        SetExpr::Values(values) => {
            for row in &mut values.rows {
                row.push(tenant_value.clone());
            }
        }
        SetExpr::Select(select) => {
            select
                .projection
                .push(SelectItem::UnnamedExpr(tenant_value.clone()));
        }
        SetExpr::Query(query) => append_insert_value(query.body.as_mut(), tenant_value),
        SetExpr::SetOperation { left, right, .. } => {
            append_insert_value(left, tenant_value);
            append_insert_value(right, tenant_value);
        }
        _ => {}
    }
}

/// 逐分支迁移 Java `ServletPathMatcher#matches`。
pub(crate) fn servlet_path_matches(pattern: &str, source: &str) -> bool {
    let pattern = pattern.trim();
    let source = source.trim();
    if let Some(prefix) = pattern.strip_suffix('*') {
        source.starts_with(prefix)
    } else if let Some(suffix) = pattern.strip_prefix('*') {
        source.ends_with(suffix)
    } else if let (Some(start), Some(end)) = (pattern.find('*'), pattern.rfind('*')) {
        source.starts_with(&pattern[..start]) && source.ends_with(&pattern[end + 1..])
    } else {
        pattern == source
    }
}
