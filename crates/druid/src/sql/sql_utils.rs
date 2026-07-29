use super::DbType;
use crate::core::DruidError;
use sqlparser::ast::Statement;
use sqlparser::dialect::{
    BigQueryDialect, ClickHouseDialect, Dialect, GenericDialect, HiveDialect, MsSqlDialect,
    MySqlDialect, PostgreSqlDialect, RedshiftSqlDialect, SQLiteDialect, SnowflakeDialect,
};
use sqlparser::parser::Parser;

/// Druid SQL parse/format 门面。
///
/// 对应 Java：`com.alibaba.druid.sql.SQLUtils`。AST 平台按迁移规划替换为
/// `sqlparser-rs`，但方言选择、完整输入消费、多语句与单语句约束保持显式。
#[derive(Debug, Default, Clone, Copy)]
pub struct SqlUtils;

impl SqlUtils {
    /// 解析全部 SQL statements。
    pub fn parse_statements(sql: &str, db_type: DbType) -> Result<Vec<Statement>, DruidError> {
        Parser::parse_sql(Self::dialect(db_type).as_ref(), sql)
            .map_err(|error| DruidError::SqlParseError(error.to_string()))
    }

    /// 解析且只允许一条 statement。
    pub fn parse_single_statement(sql: &str, db_type: DbType) -> Result<Statement, DruidError> {
        let mut statements = Self::parse_statements(sql, db_type)?;
        if statements.len() != 1 {
            return Err(DruidError::SqlParseError(format!(
                "expected exactly one statement, found {}",
                statements.len()
            )));
        }
        Ok(statements.remove(0))
    }

    /// 格式化 SQL；多条语句以 Java Druid 风格的 `;\n` 连接。
    pub fn format(sql: &str, db_type: DbType) -> Result<String, DruidError> {
        Self::parse_statements(sql, db_type).map(|statements| Self::to_sql_string(&statements))
    }

    /// 将 AST 列表输出为 SQL。
    #[must_use]
    pub fn to_sql_string(statements: &[Statement]) -> String {
        statements
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(";\n")
    }

    /// 参数化字面量并返回模板。
    #[must_use]
    pub fn parameterize(sql: &str) -> String {
        crate::stats::parameterize(sql).template
    }

    /// 返回数据库类型对应的 sqlparser 方言。
    #[must_use]
    pub fn dialect(db_type: DbType) -> Box<dyn Dialect> {
        match db_type {
            DbType::MySql
            | DbType::MariaDb
            | DbType::OceanBase
            | DbType::Drds
            | DbType::TiDb
            | DbType::GoldenDb
            | DbType::PolarDbX
            | DbType::AdbMySql => Box::new(MySqlDialect {}),
            DbType::PostgreSql
            | DbType::Edb
            | DbType::Greenplum
            | DbType::GaussDb
            | DbType::Hologres => Box::new(PostgreSqlDialect {}),
            DbType::SqlServer | DbType::Jtds | DbType::Synapse => Box::new(MsSqlDialect {}),
            DbType::SQLite => Box::new(SQLiteDialect {}),
            DbType::ClickHouse => Box::new(ClickHouseDialect {}),
            DbType::BigQuery => Box::new(BigQueryDialect {}),
            DbType::Snowflake => Box::new(SnowflakeDialect {}),
            DbType::Redshift => Box::new(RedshiftSqlDialect {}),
            DbType::Hive => Box::new(HiveDialect {}),
            _ => Box::new(GenericDialect {}),
        }
    }
}
