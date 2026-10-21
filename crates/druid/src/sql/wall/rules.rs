use super::*;

/// 返回 FROM 子句中全部表连接的引用。
pub(crate) fn tables_of(from: &FromTable) -> &[sqlparser::ast::TableWithJoins] {
    match from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    }
}

pub(crate) fn from_has_join(from: &FromTable) -> bool {
    let tables = match from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    };
    tables.iter().any(|table| !table.joins.is_empty())
}

pub(crate) fn comment_is_accepted_after(token: &Token) -> bool {
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

pub(crate) fn is_number_literal(expression: &Expr) -> bool {
    matches!(expression, Expr::Value(Value::Number(_, _)))
}

pub(crate) fn simple_name(name: &ObjectName) -> String {
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

pub(crate) fn split_boolean_and<'a>(expression: &'a Expr, conditions: &mut Vec<&'a Expr>) {
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

pub(crate) fn expression_contains_column(expression: &Expr, column_name: &str) -> bool {
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

pub(crate) fn expression_is_column(expression: &Expr, column_name: &str) -> bool {
    match expression {
        Expr::Identifier(identifier) => identifier.value.eq_ignore_ascii_case(column_name),
        Expr::CompoundIdentifier(identifiers) => identifiers
            .last()
            .is_some_and(|identifier| identifier.value.eq_ignore_ascii_case(column_name)),
        _ => false,
    }
}

pub(crate) fn is_literal_or_placeholder(expression: &Expr) -> bool {
    matches!(expression, Expr::Value(_))
}

pub(crate) fn placeholder_indices(statement: &Statement) -> HashMap<usize, usize> {
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

pub(crate) fn expression_parameter_index(
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

pub(crate) fn explicit_parameter_index(placeholder: &str) -> Option<usize> {
    placeholder
        .strip_prefix('$')
        .or_else(|| placeholder.strip_prefix('?'))
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<usize>().ok())
}

pub(crate) fn is_zero_literal(expression: &Expr) -> bool {
    matches!(expression, Expr::Value(Value::Number(value, _)) if value == "0")
}

pub(crate) fn is_always_true(expression: &Expr) -> bool {
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
pub(crate) fn const_bool_value(expression: &Expr) -> Option<bool> {
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

pub(crate) fn const_scalar(expression: &Expr) -> Option<Value> {
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
pub(crate) fn is_const_expr(expression: &Expr) -> bool {
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

pub(crate) fn is_const_bool_true_expr(expression: &Expr) -> bool {
    const_bool_value(expression) == Some(true)
}

pub(crate) fn contains_disallowed_comment(sql: &str, hint_allow: bool) -> bool {
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
