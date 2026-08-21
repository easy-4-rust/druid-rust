use super::DbType;
use crate::core::DruidError;
use sqlparser::ast::Statement;
use sqlparser::dialect::{
    BigQueryDialect, ClickHouseDialect, Dialect, GenericDialect, HiveDialect, MsSqlDialect,
    MySqlDialect, PostgreSqlDialect, RedshiftSqlDialect, SQLiteDialect, SnowflakeDialect,
};
use sqlparser::parser::Parser;

/// SQL 输出格式选项。
///
/// 对应 Java：`com.alibaba.druid.sql.SQLUtils.FormatOption`。该对象是
/// `SQLUtils` 的紧密内部对象，保留大小写、pretty、参数化和脱敏四项可观察
/// 配置，供日志过滤器及其他 SQL 格式化调用方保存配置契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlFormatOption {
    ucase: bool,
    pretty_format: bool,
    parameterized: bool,
    desensitize: bool,
}

impl SqlFormatOption {
    /// 创建格式选项。对应 Java 三布尔构造器。
    #[must_use]
    pub const fn new(ucase: bool, pretty_format: bool, parameterized: bool) -> Self {
        Self {
            ucase,
            pretty_format,
            parameterized,
            desensitize: false,
        }
    }

    /// 是否输出大写关键字。
    #[must_use]
    pub const fn is_ucase(self) -> bool {
        self.ucase
    }

    /// 设置是否输出大写关键字。
    pub fn set_ucase(&mut self, value: bool) {
        self.ucase = value;
    }

    /// 是否 pretty-format。
    #[must_use]
    pub const fn is_pretty_format(self) -> bool {
        self.pretty_format
    }

    /// 设置 pretty-format。
    pub fn set_pretty_format(&mut self, value: bool) {
        self.pretty_format = value;
    }

    /// 是否参数化字面量。
    #[must_use]
    pub const fn is_parameterized(self) -> bool {
        self.parameterized
    }

    /// 设置是否参数化字面量。
    pub fn set_parameterized(&mut self, value: bool) {
        self.parameterized = value;
    }

    /// 是否启用脱敏输出。
    #[must_use]
    pub const fn is_desensitize(self) -> bool {
        self.desensitize
    }

    /// 设置是否启用脱敏输出。
    pub fn set_desensitize(&mut self, value: bool) {
        self.desensitize = value;
    }
}

impl Default for SqlFormatOption {
    fn default() -> Self {
        Self::new(true, true, false)
    }
}

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
