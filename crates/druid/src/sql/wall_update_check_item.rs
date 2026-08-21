//! 对应 Java：`com.alibaba.druid.wall.WallUpdateCheckItem`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/wall/WallUpdateCheckItem.java`。

use crate::core::Value;
use bigdecimal::BigDecimal;
use std::str::FromStr;

/// Wall 对 UPDATE 赋值与过滤值的检查描述。
///
/// Java 使用 Druid `SQLExpr`；Rust 使用当前 SQL AST 平台
/// `sqlparser::ast::Expr`，保留表名、列名、赋值表达式和过滤表达式列表。
#[derive(Debug, Clone, PartialEq)]
pub struct WallUpdateCheckItem {
    pub table_name: String,
    pub column_name: String,
    pub value: sqlparser::ast::Expr,
    pub filter_values: Vec<sqlparser::ast::Expr>,
    value_parameter_index: Option<usize>,
    filter_parameter_indices: Vec<Option<usize>>,
}

impl WallUpdateCheckItem {
    /// 创建 UPDATE 检查项。
    ///
    /// 对应 Java：
    /// `WallUpdateCheckItem(String,String,SQLExpr,List<SQLExpr>)`。
    #[must_use]
    pub fn new(
        table_name: impl Into<String>,
        column_name: impl Into<String>,
        value: sqlparser::ast::Expr,
        filter_values: Vec<sqlparser::ast::Expr>,
    ) -> Self {
        Self {
            table_name: table_name.into(),
            column_name: column_name.into(),
            value,
            filter_parameter_indices: vec![None; filter_values.len()],
            filter_values,
            value_parameter_index: None,
        }
    }

    pub(crate) fn with_parameter_indices(
        table_name: impl Into<String>,
        column_name: impl Into<String>,
        value: sqlparser::ast::Expr,
        value_parameter_index: Option<usize>,
        filter_values: Vec<sqlparser::ast::Expr>,
        filter_parameter_indices: Vec<Option<usize>>,
    ) -> Self {
        debug_assert_eq!(filter_values.len(), filter_parameter_indices.len());
        Self {
            table_name: table_name.into(),
            column_name: column_name.into(),
            value,
            filter_values,
            value_parameter_index,
            filter_parameter_indices,
        }
    }

    /// 返回 SET 表达式对应的 Java 1-based 参数下标。
    ///
    /// 字面量返回 `None`；匿名 `?` 使用 AST 源顺序，`$n`/`?n` 使用显式下标。
    #[must_use]
    pub const fn value_parameter_index(&self) -> Option<usize> {
        self.value_parameter_index
    }

    /// 返回过滤表达式对应的 Java 1-based 参数下标。
    #[must_use]
    pub fn filter_parameter_indices(&self) -> &[Option<usize>] {
        &self.filter_parameter_indices
    }

    /// 当 SET 与过滤表达式全部为 SQL 字面量时返回其通用值。
    ///
    /// 对应 Java visitor 中 `SQLValuableExpr` 的即时 handler 分支。
    #[must_use]
    pub fn literal_values(&self) -> Option<(Value, Vec<Value>)> {
        let set_value = literal_value(&self.value)?;
        let filter_values = self
            .filter_values
            .iter()
            .map(literal_value)
            .collect::<Option<Vec<_>>>()?;
        Some((set_value, deduplicate_values(filter_values)))
    }

    /// 使用 `PreparedStatement` 的有序参数求值。
    ///
    /// Java 参数下标从 1 开始；参数不存在时与 Java `parameterMap.get(index) ==
    /// null` 一致，返回 SQL NULL。表达式不是字面量或占位符时返回 `None`。
    #[must_use]
    pub fn resolve_values(&self, parameters: &[Value]) -> Option<(Value, Vec<Value>)> {
        let set_value = resolve_value(&self.value, self.value_parameter_index, parameters)?;
        let filter_values = self
            .filter_values
            .iter()
            .zip(&self.filter_parameter_indices)
            .map(|(expression, parameter_index)| {
                resolve_value(expression, *parameter_index, parameters)
            })
            .collect::<Option<Vec<_>>>()?;
        Some((set_value, filter_values))
    }
}

fn resolve_value(
    expression: &sqlparser::ast::Expr,
    parameter_index: Option<usize>,
    parameters: &[Value],
) -> Option<Value> {
    if let Some(value) = literal_value(expression) {
        return Some(value);
    }
    if matches!(
        expression,
        sqlparser::ast::Expr::Value(sqlparser::ast::Value::Placeholder(_))
    ) {
        return Some(
            parameter_index
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| parameters.get(index))
                .cloned()
                .unwrap_or(Value::Null),
        );
    }
    None
}

fn literal_value(expression: &sqlparser::ast::Expr) -> Option<Value> {
    let sqlparser::ast::Expr::Value(value) = expression else {
        return None;
    };
    match value {
        sqlparser::ast::Value::Number(value, _) => value
            .parse::<i64>()
            .map(Value::Int)
            .or_else(|_| BigDecimal::from_str(value).map(Value::Decimal))
            .ok(),
        sqlparser::ast::Value::SingleQuotedString(value)
        | sqlparser::ast::Value::DoubleQuotedString(value)
        | sqlparser::ast::Value::TripleSingleQuotedString(value)
        | sqlparser::ast::Value::TripleDoubleQuotedString(value)
        | sqlparser::ast::Value::EscapedStringLiteral(value)
        | sqlparser::ast::Value::UnicodeStringLiteral(value)
        | sqlparser::ast::Value::NationalStringLiteral(value)
        | sqlparser::ast::Value::SingleQuotedRawStringLiteral(value)
        | sqlparser::ast::Value::DoubleQuotedRawStringLiteral(value)
        | sqlparser::ast::Value::TripleSingleQuotedRawStringLiteral(value)
        | sqlparser::ast::Value::TripleDoubleQuotedRawStringLiteral(value) => {
            Some(Value::String(value.clone()))
        }
        sqlparser::ast::Value::DollarQuotedString(value) => {
            Some(Value::String(value.value.clone()))
        }
        sqlparser::ast::Value::SingleQuotedByteStringLiteral(value)
        | sqlparser::ast::Value::DoubleQuotedByteStringLiteral(value)
        | sqlparser::ast::Value::TripleSingleQuotedByteStringLiteral(value)
        | sqlparser::ast::Value::TripleDoubleQuotedByteStringLiteral(value) => {
            Some(Value::Bytes(value.as_bytes().to_vec()))
        }
        sqlparser::ast::Value::HexStringLiteral(value) => decode_hex(value).map(Value::Bytes),
        sqlparser::ast::Value::Boolean(value) => Some(Value::Bool(*value)),
        sqlparser::ast::Value::Null => Some(Value::Null),
        sqlparser::ast::Value::Placeholder(_) => None,
    }
}

fn deduplicate_values(values: Vec<Value>) -> Vec<Value> {
    let mut deduplicated = Vec::with_capacity(values.len());
    for value in values {
        if !deduplicated.contains(&value) {
            deduplicated.push(value);
        }
    }
    deduplicated
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect()
}
