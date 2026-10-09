//! SQLite RETURNING 的私有生成键投影，保留原查询结果的列和顺序。

use druid_rdbc::error::RdbcError;
use druid_rdbc::types::{Row, Value};
use sqlparser::ast::{Expr, Ident, SelectItem, Statement};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

/// 从当前返回行提取真实 rowid，避免连接级历史键污染 upsert。
/// 对应 Java：Statement#getGeneratedKeys 的当前执行结果语义。
pub(crate) struct SqliteReturningKeyPlan {
    statement: Statement,
    table: String,
    schema: Option<String>,
}

impl SqliteReturningKeyPlan {
    /// 解析单条 INSERT RETURNING；其他语句保持原查询路径。
    /// 参数 sql 为原执行语句；返回可选投影计划，解析失败交回原驱动执行。
    pub(crate) fn parse(sql: &str) -> Result<Option<Self>, RdbcError> {
        Self::parse_mode(sql, false)
    }

    /// 为更新入口构造 INSERT 的私有 RETURNING；参数为原SQL，返回可选投影计划。
    pub(crate) fn parse_update(sql: &str) -> Result<Option<Self>, RdbcError> {
        Self::parse_mode(sql, true)
    }

    fn parse_mode(sql: &str, update: bool) -> Result<Option<Self>, RdbcError> {
        // 辅助解析不负责 SQL 合法性裁定；失败时由原驱动保留 SQL 异常语义。
        let Ok(mut statements) = Parser::parse_sql(&SQLiteDialect {}, sql) else {
            return Ok(None);
        };
        if statements.len() != 1 {
            return Ok(None);
        }
        let mut statement = statements.remove(0);
        let Statement::Insert(insert) = &mut statement else {
            return Ok(None);
        };
        if update && insert.returning.is_none() {
            insert.returning = Some(Vec::new());
        }
        if insert.returning.is_none() {
            return Ok(None);
        }
        let parts = &insert.table_name.0;
        let (schema, table) = match parts.as_slice() {
            [table] => (None, table.value.clone()),
            [schema, table] => (Some(schema.value.clone()), table.value.clone()),
            _ => return Ok(None),
        };
        Ok(Some(Self {
            statement,
            table,
            schema,
        }))
    }

    fn literal(value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }

    /// 按 SQLite 的 temp/main/attached 查找顺序确定真实表及 WITHOUT ROWID 属性。
    /// 无参数；返回只读元数据查询语句。
    pub(crate) fn table_properties_sql(&self) -> String {
        let schema = self.schema.as_ref().map_or(String::new(), |schema| {
            format!(" AND t.schema = {} COLLATE NOCASE", Self::literal(schema))
        });
        // SQLite 标识符按 ASCII 大小写无关匹配，不能用默认二进制字符串比较。
        format!("SELECT CAST(t.schema AS TEXT), CAST(t.wr AS INTEGER), CAST(t.type AS TEXT) FROM pragma_table_list AS t JOIN pragma_database_list AS d ON t.schema = d.name WHERE t.name = {} COLLATE NOCASE{schema} ORDER BY CASE t.schema WHEN 'temp' THEN 0 WHEN 'main' THEN 1 ELSE d.seq + 2 END LIMIT 1", Self::literal(&self.table))
    }

    fn schema_from_properties(properties: &[Row]) -> Result<&str, RdbcError> {
        match properties.first().and_then(|row| row.values.first()) {
            Some(Value::String(schema)) => Ok(schema),
            _ => Err(RdbcError::InvalidArgument(
                "SQLite INSERT target table was not found".into(),
            )),
        }
    }

    /// 查询实际目标表的列，识别被用户列遮蔽的 rowid 名称。
    /// 参数 properties 为表属性查询结果；返回目标 schema 的列查询语句。
    pub(crate) fn columns_sql(&self, properties: &[Row]) -> Result<String, RdbcError> {
        Ok(format!("SELECT CAST(name AS TEXT), CAST(type AS TEXT), CAST(pk AS INTEGER) FROM pragma_table_xinfo({}, {})", Self::literal(&self.table), Self::literal(Self::schema_from_properties(properties)?)))
    }

    /// 查询主键索引，避免把 INTEGER PRIMARY KEY DESC 误认为 rowid 别名。
    /// 参数 properties 为表属性查询结果；返回目标 schema 的索引查询语句。
    pub(crate) fn indexes_sql(&self, properties: &[Row]) -> Result<String, RdbcError> {
        Ok(format!(
            "SELECT CAST(origin AS TEXT) FROM pragma_index_list({}, {})",
            Self::literal(&self.table),
            Self::literal(Self::schema_from_properties(properties)?)
        ))
    }

    /// 使用真实 schema 信息追加私有键列；无可寻址 rowid 时返回 NULL，不取历史键。
    /// 参数为表属性、列及索引查询结果；消费计划并返回保留原 RETURNING 列的 SQL。
    pub(crate) fn sql(mut self, properties: &[Row], columns: &[Row], indexes: &[Row]) -> String {
        let without_rowid = matches!(
            properties.first().and_then(|row| row.values.get(1)),
            Some(Value::Int(1))
        ) || matches!(properties.first().and_then(|row| row.values.get(2)), Some(Value::String(kind)) if kind == "view");
        let primary_columns: Vec<&Row> = columns
            .iter()
            .filter(|row| matches!(row.values.get(2), Some(Value::Int(position)) if *position > 0))
            .collect();
        let has_primary_index = indexes
            .iter()
            .any(|row| matches!(row.values.first(), Some(Value::String(origin)) if origin == "pk"));
        let integer_primary = if !without_rowid && !has_primary_index && primary_columns.len() == 1
        {
            let row = primary_columns[0];
            match (row.values.first(), row.values.get(1)) {
                (Some(Value::String(name)), Some(Value::String(kind)))
                    if kind.eq_ignore_ascii_case("INTEGER") =>
                {
                    Some(name.as_str())
                }
                _ => None,
            }
        } else {
            None
        };
        let key_name = integer_primary.or_else(|| {
            (!without_rowid).then(|| ["_rowid_", "rowid", "oid"].into_iter().find(|candidate| !columns.iter().any(|row| matches!(row.values.first(), Some(Value::String(name)) if name.eq_ignore_ascii_case(candidate))))).flatten()
        });
        let expr = key_name.map_or(Expr::Value(sqlparser::ast::Value::Null), |name| {
            Expr::Identifier(Ident::with_quote('"', name))
        });
        if let Statement::Insert(insert) = &mut self.statement {
            insert
                .returning
                .as_mut()
                .expect("plan contains RETURNING")
                .push(SelectItem::ExprWithAlias {
                    expr,
                    alias: Ident::with_quote('"', "__druid_generated_key"),
                });
        }
        self.statement.to_string()
    }

    /// 移除每行末尾的私有键列，保留原行；按返回行顺序收集全部实际键。
    /// 参数 rows 为待恢复的结果行；返回全部整数键，私有列类型错误时返回驱动错误。
    pub(crate) fn extract(rows: &mut [Row]) -> Result<Vec<i64>, RdbcError> {
        let mut keys = Vec::with_capacity(rows.len());
        for row in rows {
            match row.values.pop() {
                Some(Value::Int(key)) => keys.push(key),
                Some(Value::Null) => {}
                _ => {
                    return Err(RdbcError::DriverError(
                        "SQLite private generated key must be INTEGER or NULL".into(),
                    ))
                }
            }
        }
        Ok(keys)
    }

    /// 参数为私有返回行及同连接 changes 查询行；返回真实更新数和完整当前键集合。
    pub(crate) fn update_outcome(
        mut rows: Vec<Row>,
        changes: &[Row],
    ) -> Result<(i64, Vec<i64>), RdbcError> {
        let count = match changes.first().and_then(|row| row.values.first()) {
            Some(Value::Int(count)) if *count >= 0 => *count,
            _ => {
                return Err(RdbcError::DriverError(
                    "SQLite changes() must return a nonnegative integer".into(),
                ))
            }
        };
        Ok((count, Self::extract(&mut rows)?))
    }
}
