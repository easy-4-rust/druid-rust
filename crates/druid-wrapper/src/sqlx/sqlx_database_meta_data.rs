//! SQLx 数据库元数据 Adapter。

use druid::core::{
    DruidError, PhysicalDatabaseMetaData, PhysicalResultSet, Row, RowSetResultSet, Value,
};
use sqlx::{Any, AnyConnection, Row as SqlxRow, Sqlite, SqliteConnection};
use std::sync::Arc;

const JDBC_TYPE_NULL: i32 = 0;
const JDBC_TYPE_INTEGER: i32 = 4;
const JDBC_TYPE_FLOAT: i32 = 6;
const JDBC_TYPE_REAL: i32 = 7;
const JDBC_TYPE_VARCHAR: i32 = 12;
const JDBC_TYPE_BLOB: i32 = 2004;
const JDBC_TYPE_NULLABLE: i32 = 1;
const JDBC_TYPE_SEARCHABLE: i32 = 3;
const JDBC_TABLE_INDEX_OTHER: i32 = 3;
const JDBC_COLUMN_NO_NULLS: i32 = 0;
const JDBC_COLUMN_NULLABLE: i32 = 1;
const JDBC_COLUMN_NULLABLE_UNKNOWN: i32 = 2;
const JDBC_IMPORTED_KEY_CASCADE: i32 = 0;
const JDBC_IMPORTED_KEY_RESTRICT: i32 = 1;
const JDBC_IMPORTED_KEY_SET_NULL: i32 = 2;
const JDBC_IMPORTED_KEY_NO_ACTION: i32 = 3;
const JDBC_IMPORTED_KEY_SET_DEFAULT: i32 = 4;
const JDBC_IMPORTED_KEY_INITIALLY_DEFERRED: i32 = 5;

/// SQLite `pragma_foreign_key_list` 的规范化单行。
struct SqliteForeignKey {
    sequence: i64,
    primary_table: String,
    foreign_column: String,
    primary_column: Option<String>,
    update_rule: i32,
    delete_rule: i32,
    name: Option<String>,
}

/// SQLx metadata 对当前未池化连接的借用。
pub(super) enum SqlxDatabaseMetaDataBackend<'connection> {
    /// SQLx Any（MySQL/PostgreSQL 等）。
    Any(&'connection mut AnyConnection),
    /// 原生 SQLite。
    Sqlite(&'connection mut SqliteConnection),
}

/// SQLx 的物理数据库元数据实现。
///
/// 对应 Java 平台职责：JDBC driver 的 `DatabaseMetaData` 实现。只报告 SQLx
/// 和真实后端能够证明的能力；未暴露的 JDBC 能力沿用 trait 的精确
/// `UnsupportedOperation`，不会以默认 false/空字符串冒充驱动结果。
pub struct SqlxDatabaseMetaData<'connection> {
    backend: SqlxDatabaseMetaDataBackend<'connection>,
    url: &'connection str,
}

impl<'connection> SqlxDatabaseMetaData<'connection> {
    /// 创建 SQLx Any metadata。
    pub(super) fn any(connection: &'connection mut AnyConnection, url: &'connection str) -> Self {
        Self {
            backend: SqlxDatabaseMetaDataBackend::Any(connection),
            url,
        }
    }

    /// 创建 SQLite metadata。
    pub(super) fn sqlite(
        connection: &'connection mut SqliteConnection,
        url: &'connection str,
    ) -> Self {
        Self {
            backend: SqlxDatabaseMetaDataBackend::Sqlite(connection),
            url,
        }
    }

    fn is_mysql(&self) -> bool {
        self.url
            .get(..6)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("mysql:"))
    }

    fn is_postgresql(&self) -> bool {
        self.url
            .get(..9)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("postgres:"))
            || self
                .url
                .get(..11)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("postgresql:"))
    }

    async fn product_version(&mut self) -> Result<String, DruidError> {
        let is_postgresql = self.is_postgresql();
        let is_mysql = self.is_mysql();
        match &mut self.backend {
            SqlxDatabaseMetaDataBackend::Sqlite(connection) => {
                sqlx::query_scalar::<Sqlite, String>("select sqlite_version()")
                    .fetch_one(&mut **connection)
                    .await
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)
            }
            SqlxDatabaseMetaDataBackend::Any(connection) if is_postgresql => {
                sqlx::query_scalar::<Any, String>("show server_version")
                    .fetch_one(&mut **connection)
                    .await
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)
            }
            SqlxDatabaseMetaDataBackend::Any(connection) if is_mysql => {
                sqlx::query_scalar::<Any, String>("select version()")
                    .fetch_one(&mut **connection)
                    .await
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)
            }
            SqlxDatabaseMetaDataBackend::Any(_) => Err(DruidError::UnsupportedOperation {
                operation: "sqlx_database_metadata_product_version",
            }),
        }
    }

    async fn version_component(&mut self, index: usize) -> Result<i32, DruidError> {
        let version = self.product_version().await?;
        version
            .split(|character: char| !character.is_ascii_digit())
            .filter(|component| !component.is_empty())
            .nth(index)
            .and_then(|component| component.parse::<i32>().ok())
            .ok_or_else(|| {
                DruidError::DriverError(format!(
                    "database version `{version}` has no numeric component {index}"
                ))
            })
    }

    fn labels(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|label| (*label).to_owned()).collect()
    }

    fn result_set(rows: Vec<Row>, labels: &[&str]) -> Arc<dyn PhysicalResultSet> {
        Arc::new(RowSetResultSet::with_column_labels(
            rows,
            Self::labels(labels),
        ))
    }

    fn sqlite_empty_result_set(
        &self,
        operation: &'static str,
        labels: &[&str],
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.require_sqlite(operation)?;
        Ok(Self::result_set(Vec::new(), labels))
    }

    fn sqlite_connection(
        &mut self,
        operation: &'static str,
    ) -> Result<&mut SqliteConnection, DruidError> {
        match &mut self.backend {
            SqlxDatabaseMetaDataBackend::Sqlite(connection) => Ok(&mut **connection),
            SqlxDatabaseMetaDataBackend::Any(_) => {
                Err(DruidError::UnsupportedOperation { operation })
            }
        }
    }

    fn require_sqlite(&self, operation: &'static str) -> Result<(), DruidError> {
        match self.backend {
            SqlxDatabaseMetaDataBackend::Sqlite(_) => Ok(()),
            SqlxDatabaseMetaDataBackend::Any(_) => {
                Err(DruidError::UnsupportedOperation { operation })
            }
        }
    }

    async fn sqlite_tables(
        &mut self,
        table_name_pattern: Option<&str>,
        types: Option<&[String]>,
    ) -> Result<Vec<(String, String)>, DruidError> {
        let pattern = match table_name_pattern {
            None | Some("") => "%",
            Some(pattern) => pattern,
        };
        let rows = sqlx::query(
            "SELECT NAME, TYPE FROM (\
             SELECT 'sqlite_schema' AS NAME, 'SYSTEM TABLE' AS TYPE \
             UNION ALL \
             SELECT NAME, UPPER(TYPE) AS TYPE FROM sqlite_schema \
             WHERE NAME NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
             AND UPPER(TYPE) IN ('TABLE', 'VIEW') \
             UNION ALL \
             SELECT NAME, 'GLOBAL TEMPORARY' AS TYPE FROM sqlite_temp_master \
             UNION ALL \
             SELECT NAME, 'SYSTEM TABLE' AS TYPE FROM sqlite_schema \
             WHERE NAME LIKE 'sqlite\\_%' ESCAPE '\\'\
             ) WHERE NAME LIKE ? ESCAPE '\\' ORDER BY TYPE, NAME",
        )
        .bind(pattern)
        .fetch_all(self.sqlite_connection("sqlx_database_metadata_sqlite_get_tables")?)
        .await
        .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;

        let accepted_types = types.map(|types| {
            types
                .iter()
                .map(|table_type| table_type.to_uppercase())
                .collect::<Vec<_>>()
        });
        rows.into_iter()
            .map(|row| {
                let name = row
                    .try_get::<String, _>(0)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let table_type = row
                    .try_get::<String, _>(1)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                Ok((name, table_type))
            })
            .filter_map(|row: Result<(String, String), DruidError>| match row {
                Ok((name, table_type))
                    if accepted_types
                        .as_ref()
                        .is_none_or(|types| types.contains(&table_type)) =>
                {
                    Some(Ok((name, table_type)))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    fn sqlite_declared_type(declared_type: Option<&str>) -> (i32, String, i64, i64) {
        let mut type_name = declared_type.unwrap_or("TEXT").to_uppercase();
        let mut column_size = 2_000_000_000_i64;
        let mut decimal_digits = 10_i64;
        let data_type = if type_name.contains("INT") || type_name.contains("BOOL") {
            decimal_digits = 0;
            JDBC_TYPE_INTEGER
        } else if ["CHAR", "CLOB", "TEXT", "BLOB"]
            .iter()
            .any(|marker| type_name.contains(marker))
        {
            decimal_digits = 0;
            JDBC_TYPE_VARCHAR
        } else if ["REAL", "FLOA", "DOUB", "DEC", "NUM"]
            .iter()
            .any(|marker| type_name.contains(marker))
        {
            JDBC_TYPE_FLOAT
        } else {
            JDBC_TYPE_VARCHAR
        };

        if let Some(open) = type_name.find('(').filter(|open| *open > 0) {
            if let Some(close_offset) = type_name[open + 1..].find(')') {
                let close = open + 1 + close_offset;
                let dimensions = &type_name[open + 1..close];
                let mut parts = dimensions.splitn(2, ',');
                if let Some(integer_part) = parts.next() {
                    if let Ok(integer_digits) = integer_part.trim().parse::<u64>() {
                        if let Some(decimal_part) = parts.next() {
                            if let Ok(parsed_decimal_digits) = decimal_part.trim().parse::<u64>() {
                                decimal_digits = parsed_decimal_digits as i64;
                                column_size = integer_digits
                                    .saturating_add(parsed_decimal_digits)
                                    .min(i64::MAX as u64)
                                    as i64;
                            }
                        } else {
                            decimal_digits = 0;
                            column_size = integer_digits.min(i64::MAX as u64) as i64;
                        }
                    }
                }
            }
            type_name.truncate(open);
            type_name = type_name.trim().to_owned();
        }

        (data_type, type_name, column_size, decimal_digits)
    }

    fn unquote_identifier(identifier: &str) -> String {
        let identifier = identifier.trim();
        let bytes = identifier.as_bytes();
        if bytes.len() >= 2 {
            let paired = matches!(
                (bytes[0], bytes[bytes.len() - 1]),
                (b'"', b'"') | (b'`', b'`') | (b'\'', b'\'') | (b'[', b']')
            );
            if paired {
                return identifier[1..identifier.len() - 1].trim().to_owned();
            }
        }
        identifier.to_owned()
    }

    fn primary_key_definition(create_sql: &str) -> Option<(Option<String>, Vec<String>)> {
        let lower = create_sql.to_ascii_lowercase();
        let primary = lower.rfind("primary")?;
        let after_primary = &lower[primary + "primary".len()..];
        let key_offset = after_primary.find("key")?;
        let after_key = primary + "primary".len() + key_offset + "key".len();
        let open_offset = lower[after_key..].find('(')?;
        let open = after_key + open_offset;
        let close = lower[open + 1..].find(')')? + open + 1;
        let columns = create_sql[open + 1..close]
            .split(',')
            .map(Self::unquote_identifier)
            .collect::<Vec<_>>();
        if columns.is_empty() {
            return None;
        }

        let prefix = &lower[..primary];
        let name = prefix.rfind("constraint").and_then(|constraint| {
            let candidate = create_sql[constraint + "constraint".len()..primary].trim();
            (!candidate.is_empty()).then(|| Self::unquote_identifier(candidate))
        });
        Some((name, columns))
    }

    fn foreign_key_names(create_sql: &str) -> Vec<Option<String>> {
        let lower = create_sql.to_ascii_lowercase();
        let mut names = Vec::new();
        let mut offset = 0;
        while let Some(foreign_offset) = lower[offset..].find("foreign") {
            let foreign = offset + foreign_offset;
            let prefix = &lower[..foreign];
            let name = prefix.rfind("constraint").and_then(|constraint| {
                let candidate = create_sql[constraint + "constraint".len()..foreign].trim();
                let candidate = Self::unquote_identifier(candidate);
                let valid = !candidate.is_empty()
                    && candidate.bytes().enumerate().all(|(index, byte)| {
                        byte == b'_'
                            || byte.is_ascii_alphabetic()
                            || (index > 0 && byte.is_ascii_digit())
                    });
                valid.then_some(candidate)
            });
            names.push(name);
            offset = foreign + "foreign".len();
        }
        // SQLite PRAGMA 的外键 id 顺序与 CREATE TABLE 中的声明顺序相反。
        names.reverse();
        names
    }

    fn imported_key_rule(rule: &str) -> i32 {
        match rule {
            "CASCADE" => JDBC_IMPORTED_KEY_CASCADE,
            "RESTRICT" => JDBC_IMPORTED_KEY_RESTRICT,
            "SET NULL" => JDBC_IMPORTED_KEY_SET_NULL,
            "SET DEFAULT" => JDBC_IMPORTED_KEY_SET_DEFAULT,
            _ => JDBC_IMPORTED_KEY_NO_ACTION,
        }
    }

    fn optional_string(value: Option<&str>) -> Value {
        value.map_or(Value::Null, |value| Value::String(value.to_owned()))
    }

    fn foreign_key_labels() -> &'static [&'static str] {
        &[
            "PKTABLE_CAT",
            "PKTABLE_SCHEM",
            "PKTABLE_NAME",
            "PKCOLUMN_NAME",
            "FKTABLE_CAT",
            "FKTABLE_SCHEM",
            "FKTABLE_NAME",
            "FKCOLUMN_NAME",
            "KEY_SEQ",
            "UPDATE_RULE",
            "DELETE_RULE",
            "FK_NAME",
            "PK_NAME",
            "DEFERRABILITY",
        ]
    }

    async fn sqlite_primary_key_info(
        &mut self,
        table: &str,
        operation: &'static str,
    ) -> Result<(String, Option<String>, Vec<String>), DruidError> {
        if table.trim().is_empty() {
            return Err(DruidError::DriverError(format!(
                "Invalid table name: '{table}'"
            )));
        }
        if table == "sqlite_schema" || table == "sqlite_master" {
            return Ok((table.to_owned(), None, Vec::new()));
        }
        let schema_row = sqlx::query(
            "SELECT name, sql FROM sqlite_schema \
             WHERE lower(name) = lower(?) AND type IN ('table', 'view')",
        )
        .bind(table)
        .fetch_optional(self.sqlite_connection(operation)?)
        .await
        .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?
        .ok_or_else(|| DruidError::DriverError(format!("Table not found: '{table}'")))?;
        let canonical_name = schema_row
            .try_get::<String, _>(0)
            .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
        let create_sql = schema_row
            .try_get::<Option<String>, _>(1)
            .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
        if let Some(definition) = create_sql.as_deref().and_then(Self::primary_key_definition) {
            return Ok((canonical_name, definition.0, definition.1));
        }

        let pragma_rows =
            sqlx::query("SELECT name, pk FROM pragma_table_info(?) WHERE pk > 0 ORDER BY pk")
                .bind(table)
                .fetch_all(self.sqlite_connection(operation)?)
                .await
                .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
        let columns = pragma_rows
            .into_iter()
            .map(|row| {
                row.try_get::<String, _>(0)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((canonical_name, None, columns))
    }

    async fn sqlite_foreign_keys(
        &mut self,
        table: &str,
        operation: &'static str,
    ) -> Result<Vec<SqliteForeignKey>, DruidError> {
        let create_sql = sqlx::query_scalar::<Sqlite, Option<String>>(
            "SELECT sql FROM sqlite_schema WHERE lower(name) = lower(?)",
        )
        .bind(table)
        .fetch_optional(self.sqlite_connection(operation)?)
        .await
        .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?
        .flatten();
        let names = create_sql
            .as_deref()
            .map(Self::foreign_key_names)
            .unwrap_or_default();
        let rows = sqlx::query(
            "SELECT id, seq, \"table\", \"from\", \"to\", on_update, on_delete \
             FROM pragma_foreign_key_list(?) ORDER BY id, seq",
        )
        .bind(table)
        .fetch_all(self.sqlite_connection(operation)?)
        .await
        .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
        rows.into_iter()
            .map(|row| {
                let id = row
                    .try_get::<i64, _>(0)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?
                    as usize;
                let update_rule = row
                    .try_get::<String, _>(5)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let delete_rule = row
                    .try_get::<String, _>(6)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                Ok(SqliteForeignKey {
                    sequence: row.try_get::<i64, _>(1).map_err(
                        super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error,
                    )? + 1,
                    primary_table: row.try_get::<String, _>(2).map_err(
                        super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error,
                    )?,
                    foreign_column: row
                        .try_get::<Option<String>, _>(3)
                        .map_err(
                            super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error,
                        )?
                        .unwrap_or_default(),
                    primary_column: row.try_get::<Option<String>, _>(4).map_err(
                        super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error,
                    )?,
                    update_rule: Self::imported_key_rule(&update_rule),
                    delete_rule: Self::imported_key_rule(&delete_rule),
                    name: names.get(id).cloned().flatten(),
                })
            })
            .collect()
    }

    fn sqlite_table_labels() -> &'static [&'static str] {
        &[
            "TABLE_CAT",
            "TABLE_SCHEM",
            "TABLE_NAME",
            "TABLE_TYPE",
            "REMARKS",
            "TYPE_CAT",
            "TYPE_SCHEM",
            "TYPE_NAME",
            "SELF_REFERENCING_COL_NAME",
            "REF_GENERATION",
        ]
    }

    fn sqlite_column_labels() -> &'static [&'static str] {
        &[
            "TABLE_CAT",
            "TABLE_SCHEM",
            "TABLE_NAME",
            "COLUMN_NAME",
            "DATA_TYPE",
            "TYPE_NAME",
            "COLUMN_SIZE",
            "BUFFER_LENGTH",
            "DECIMAL_DIGITS",
            "NUM_PREC_RADIX",
            "NULLABLE",
            "REMARKS",
            "COLUMN_DEF",
            "SQL_DATA_TYPE",
            "SQL_DATETIME_SUB",
            "CHAR_OCTET_LENGTH",
            "ORDINAL_POSITION",
            "IS_NULLABLE",
            "SCOPE_CATALOG",
            "SCOPE_SCHEMA",
            "SCOPE_TABLE",
            "SOURCE_DATA_TYPE",
            "IS_AUTOINCREMENT",
            "IS_GENERATEDCOLUMN",
        ]
    }
}

#[async_trait::async_trait]
impl PhysicalDatabaseMetaData for SqlxDatabaseMetaData<'_> {
    async fn all_procedures_are_callable(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_all_procedures_are_callable")?;
        Ok(false)
    }

    async fn all_tables_are_selectable(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_all_tables_are_selectable")?;
        Ok(true)
    }

    async fn nulls_are_sorted_high(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_nulls_are_sorted_high")?;
        Ok(true)
    }

    async fn nulls_are_sorted_low(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_nulls_are_sorted_low")?;
        Ok(false)
    }

    async fn nulls_are_sorted_at_start(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_nulls_are_sorted_at_start")?;
        Ok(true)
    }

    async fn nulls_are_sorted_at_end(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_nulls_are_sorted_at_end")?;
        Ok(false)
    }

    async fn uses_local_files(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_uses_local_files")?;
        Ok(true)
    }

    async fn uses_local_file_per_table(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_uses_local_file_per_table")?;
        Ok(false)
    }

    async fn supports_mixed_case_identifiers(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_mixed_case_identifiers")?;
        Ok(true)
    }

    async fn stores_upper_case_identifiers(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_stores_upper_case_identifiers")?;
        Ok(false)
    }

    async fn stores_lower_case_identifiers(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_stores_lower_case_identifiers")?;
        Ok(false)
    }

    async fn stores_mixed_case_identifiers(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_stores_mixed_case_identifiers")?;
        Ok(true)
    }

    async fn supports_mixed_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_mixed_case_quoted_identifiers",
        )?;
        Ok(false)
    }

    async fn stores_upper_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_stores_upper_case_quoted_identifiers")?;
        Ok(false)
    }

    async fn stores_lower_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_stores_lower_case_quoted_identifiers")?;
        Ok(false)
    }

    async fn stores_mixed_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_stores_mixed_case_quoted_identifiers")?;
        Ok(false)
    }

    async fn supports_alter_table_with_add_column(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_alter_table_with_add_column")?;
        Ok(true)
    }

    async fn supports_alter_table_with_drop_column(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_alter_table_with_drop_column")?;
        Ok(true)
    }

    async fn supports_column_aliasing(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_column_aliasing")?;
        Ok(true)
    }

    async fn null_plus_non_null_is_null(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_null_plus_non_null_is_null")?;
        Ok(true)
    }

    async fn supports_convert(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_convert")?;
        Ok(false)
    }

    async fn supports_table_correlation_names(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_table_correlation_names")?;
        Ok(false)
    }

    async fn supports_different_table_correlation_names(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_different_table_correlation_names",
        )?;
        Ok(false)
    }

    async fn supports_expressions_in_order_by(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_expressions_in_order_by")?;
        Ok(true)
    }

    async fn supports_order_by_unrelated(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_order_by_unrelated")?;
        Ok(false)
    }

    async fn supports_group_by(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_group_by")?;
        Ok(true)
    }

    async fn supports_group_by_unrelated(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_group_by_unrelated")?;
        Ok(false)
    }

    async fn supports_group_by_beyond_select(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_group_by_beyond_select")?;
        Ok(false)
    }

    async fn supports_like_escape_clause(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_like_escape_clause")?;
        Ok(false)
    }

    async fn supports_multiple_result_sets(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_multiple_result_sets")?;
        Ok(false)
    }

    async fn supports_multiple_transactions(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_multiple_transactions")?;
        Ok(true)
    }

    async fn supports_non_nullable_columns(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_non_nullable_columns")?;
        Ok(true)
    }

    async fn supports_minimum_sql_grammar(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_minimum_sql_grammar")?;
        Ok(true)
    }

    async fn supports_core_sql_grammar(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_core_sql_grammar")?;
        Ok(true)
    }

    async fn supports_extended_sql_grammar(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_extended_sql_grammar")?;
        Ok(false)
    }

    async fn supports_ansi92_entry_level_sql(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_ansi92_entry_level_sql")?;
        Ok(false)
    }

    async fn supports_ansi92_intermediate_sql(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_ansi92_intermediate_sql")?;
        Ok(false)
    }

    async fn supports_ansi92_full_sql(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_ansi92_full_sql")?;
        Ok(false)
    }

    async fn supports_integrity_enhancement_facility(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_integrity_enhancement_facility",
        )?;
        Ok(false)
    }

    async fn supports_outer_joins(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_outer_joins")?;
        Ok(true)
    }

    async fn supports_limited_outer_joins(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_limited_outer_joins")?;
        Ok(true)
    }

    async fn is_catalog_at_start(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_is_catalog_at_start")?;
        Ok(true)
    }

    async fn supports_schemas_in_data_manipulation(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_schemas_in_data_manipulation")?;
        Ok(false)
    }

    async fn supports_schemas_in_procedure_calls(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_schemas_in_procedure_calls")?;
        Ok(false)
    }

    async fn supports_schemas_in_table_definitions(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_schemas_in_table_definitions")?;
        Ok(false)
    }

    async fn supports_schemas_in_index_definitions(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_schemas_in_index_definitions")?;
        Ok(false)
    }

    async fn supports_schemas_in_privilege_definitions(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_schemas_in_privilege_definitions",
        )?;
        Ok(false)
    }

    async fn supports_catalogs_in_data_manipulation(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_catalogs_in_data_manipulation",
        )?;
        Ok(false)
    }

    async fn supports_catalogs_in_procedure_calls(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_catalogs_in_procedure_calls")?;
        Ok(false)
    }

    async fn supports_catalogs_in_table_definitions(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_catalogs_in_table_definitions",
        )?;
        Ok(false)
    }

    async fn supports_catalogs_in_index_definitions(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_catalogs_in_index_definitions",
        )?;
        Ok(false)
    }

    async fn supports_catalogs_in_privilege_definitions(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_catalogs_in_privilege_definitions",
        )?;
        Ok(false)
    }

    async fn supports_positioned_delete(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_positioned_delete")?;
        Ok(false)
    }

    async fn supports_positioned_update(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_positioned_update")?;
        Ok(false)
    }

    async fn supports_select_for_update(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_select_for_update")?;
        Ok(false)
    }

    async fn supports_stored_procedures(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_stored_procedures")?;
        Ok(false)
    }

    async fn supports_subqueries_in_comparisons(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_subqueries_in_comparisons")?;
        Ok(false)
    }

    async fn supports_subqueries_in_exists(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_subqueries_in_exists")?;
        Ok(true)
    }

    async fn supports_subqueries_in_ins(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_subqueries_in_ins")?;
        Ok(true)
    }

    async fn supports_subqueries_in_quantifieds(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_subqueries_in_quantifieds")?;
        Ok(false)
    }

    async fn supports_correlated_subqueries(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_correlated_subqueries")?;
        Ok(false)
    }

    async fn supports_union(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_union")?;
        Ok(true)
    }

    async fn supports_union_all(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_union_all")?;
        Ok(true)
    }

    async fn supports_open_cursors_across_commit(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_open_cursors_across_commit")?;
        Ok(false)
    }

    async fn supports_open_cursors_across_rollback(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_open_cursors_across_rollback")?;
        Ok(false)
    }

    async fn supports_open_statements_across_commit(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_open_statements_across_commit",
        )?;
        Ok(false)
    }

    async fn supports_open_statements_across_rollback(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_open_statements_across_rollback",
        )?;
        Ok(false)
    }

    async fn does_max_row_size_include_blobs(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_does_max_row_size_include_blobs")?;
        Ok(false)
    }

    async fn supports_data_definition_and_data_manipulation_transactions(
        &mut self,
    ) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_data_definition_and_data_manipulation_transactions")?;
        Ok(true)
    }

    async fn supports_data_manipulation_transactions_only(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_supports_data_manipulation_transactions_only",
        )?;
        Ok(false)
    }

    async fn data_definition_causes_transaction_commit(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_data_definition_causes_transaction_commit",
        )?;
        Ok(false)
    }

    async fn data_definition_ignored_in_transactions(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite(
            "sqlx_database_metadata_sqlite_data_definition_ignored_in_transactions",
        )?;
        Ok(false)
    }

    async fn supports_multiple_open_results(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_multiple_open_results")?;
        Ok(false)
    }

    async fn supports_get_generated_keys(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_get_generated_keys")?;
        Ok(true)
    }

    async fn locators_update_copy(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_locators_update_copy")?;
        Ok(false)
    }

    async fn supports_statement_pooling(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_statement_pooling")?;
        Ok(false)
    }

    async fn get_max_binary_literal_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_binary_literal_length")?;
        Ok(0)
    }

    async fn get_max_char_literal_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_char_literal_length")?;
        Ok(0)
    }

    async fn get_max_column_name_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_column_name_length")?;
        Ok(0)
    }

    async fn get_max_columns_in_group_by(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_columns_in_group_by")?;
        Ok(0)
    }

    async fn get_max_columns_in_index(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_columns_in_index")?;
        Ok(0)
    }

    async fn get_max_columns_in_order_by(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_columns_in_order_by")?;
        Ok(0)
    }

    async fn get_max_columns_in_select(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_columns_in_select")?;
        Ok(0)
    }

    async fn get_max_columns_in_table(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_columns_in_table")?;
        Ok(0)
    }

    async fn get_max_cursor_name_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_cursor_name_length")?;
        Ok(0)
    }

    async fn get_max_index_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_index_length")?;
        Ok(0)
    }

    async fn get_max_schema_name_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_schema_name_length")?;
        Ok(0)
    }

    async fn get_max_procedure_name_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_procedure_name_length")?;
        Ok(0)
    }

    async fn get_max_catalog_name_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_catalog_name_length")?;
        Ok(0)
    }

    async fn get_max_row_size(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_row_size")?;
        Ok(0)
    }

    async fn get_max_statement_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_statement_length")?;
        Ok(0)
    }

    async fn get_max_statements(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_statements")?;
        Ok(0)
    }

    async fn get_max_table_name_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_table_name_length")?;
        Ok(0)
    }

    async fn get_max_tables_in_select(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_tables_in_select")?;
        Ok(0)
    }

    async fn get_max_user_name_length(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_max_user_name_length")?;
        Ok(0)
    }

    async fn get_url(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some(self.url.to_owned()))
    }

    async fn get_user_name(&mut self) -> Result<Option<String>, DruidError> {
        // SQLx does not expose the authenticated username after connection setup. Returning
        // None preserves the absent value without reparsing and leaking URL credentials.
        Ok(None)
    }

    async fn is_read_only(&mut self) -> Result<bool, DruidError> {
        let query_only = sqlx::query_scalar::<Sqlite, i64>("PRAGMA query_only")
            .fetch_one(self.sqlite_connection("sqlx_database_metadata_sqlite_is_read_only")?)
            .await
            .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
        Ok(query_only != 0)
    }

    async fn get_database_product_name(&mut self) -> Result<Option<String>, DruidError> {
        let product = match &self.backend {
            SqlxDatabaseMetaDataBackend::Sqlite(_) => "SQLite",
            SqlxDatabaseMetaDataBackend::Any(_) if self.is_postgresql() => "PostgreSQL",
            SqlxDatabaseMetaDataBackend::Any(_) if self.is_mysql() => "MySQL",
            SqlxDatabaseMetaDataBackend::Any(_) => {
                return Err(DruidError::UnsupportedOperation {
                    operation: "sqlx_database_metadata_product_name",
                });
            }
        };
        Ok(Some(product.to_owned()))
    }

    async fn get_database_product_version(&mut self) -> Result<Option<String>, DruidError> {
        self.product_version().await.map(Some)
    }

    async fn get_driver_name(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some("sqlx".to_owned()))
    }

    async fn get_driver_version(&mut self) -> Result<Option<String>, DruidError> {
        // sqlx does not expose its crate version as a runtime driver metadata field.
        Ok(None)
    }

    async fn get_identifier_quote_string(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some(if self.is_mysql() { "`" } else { "\"" }.to_owned()))
    }

    async fn get_search_string_escape(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some("\\".to_owned()))
    }

    async fn get_sql_keywords(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_sql_keywords")?;
        Ok(Some(
            "ABORT,ACTION,AFTER,ANALYZE,ATTACH,AUTOINCREMENT,BEFORE,\
             CASCADE,CONFLICT,DATABASE,DEFERRABLE,DEFERRED,DESC,DETACH,\
             EXCLUSIVE,EXPLAIN,FAIL,GLOB,IGNORE,INDEX,INDEXED,INITIALLY,INSTEAD,ISNULL,\
             KEY,LIMIT,NOTNULL,OFFSET,PLAN,PRAGMA,QUERY,\
             RAISE,REGEXP,REINDEX,RENAME,REPLACE,RESTRICT,\
             TEMP,TEMPORARY,TRANSACTION,VACUUM,VIEW,VIRTUAL"
                .replace(' ', ""),
        ))
    }

    async fn get_numeric_functions(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_numeric_functions")?;
        Ok(Some(String::new()))
    }

    async fn get_string_functions(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_string_functions")?;
        Ok(Some(String::new()))
    }

    async fn get_system_functions(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_system_functions")?;
        Ok(Some(String::new()))
    }

    async fn get_time_date_functions(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_time_date_functions")?;
        Ok(Some("DATE,TIME,DATETIME,JULIANDAY,STRFTIME".to_owned()))
    }

    async fn get_extra_name_characters(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_extra_name_characters")?;
        Ok(Some(String::new()))
    }

    async fn get_schema_term(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_schema_term")?;
        Ok(Some("schema".to_owned()))
    }

    async fn get_procedure_term(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_procedure_term")?;
        Ok(Some("not_implemented".to_owned()))
    }

    async fn get_catalog_term(&mut self) -> Result<Option<String>, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_catalog_term")?;
        Ok(Some("catalog".to_owned()))
    }

    async fn get_catalog_separator(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some(".".to_owned()))
    }

    async fn get_max_connections(&mut self) -> Result<i32, DruidError> {
        // JDBC 规定 0 表示未知或无限制；SQLx Connection 不暴露服务器上限。
        Ok(0)
    }

    async fn get_default_transaction_isolation(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_default_transaction_isolation")?;
        Ok(8)
    }

    async fn supports_transactions(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn supports_transaction_isolation_level(
        &mut self,
        level: i32,
    ) -> Result<bool, DruidError> {
        // Xerial SQLite 只报告 TRANSACTION_SERIALIZABLE（JDBC 常量 8）。
        Ok(match &self.backend {
            SqlxDatabaseMetaDataBackend::Sqlite(_) => level == 8,
            SqlxDatabaseMetaDataBackend::Any(_) => level == 2,
        })
    }

    async fn supports_batch_updates(&mut self) -> Result<bool, DruidError> {
        // Adapter 实现有序 batch，并在部分失败时保留已完成计数。
        Ok(true)
    }

    async fn supports_savepoints(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn supports_named_parameters(&mut self) -> Result<bool, DruidError> {
        // SQLite SQL 语法支持 :name/@name/$name；SQLx Any 未提供统一保证。
        Ok(matches!(
            self.backend,
            SqlxDatabaseMetaDataBackend::Sqlite(_)
        ))
    }

    async fn supports_convert_between(
        &mut self,
        _from_type: i32,
        _to_type: i32,
    ) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_convert_between")?;
        Ok(false)
    }

    async fn supports_full_outer_joins(&mut self) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_full_outer_joins")?;
        let major = self.version_component(0).await?;
        let minor = self.version_component(1).await?;
        Ok(major >= 3 && minor >= 39)
    }

    async fn supports_result_set_type(&mut self, result_set_type: i32) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_result_set_type")?;
        Ok(result_set_type == 1003)
    }

    async fn supports_result_set_concurrency(
        &mut self,
        result_set_type: i32,
        concurrency: i32,
    ) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_result_set_concurrency")?;
        Ok(result_set_type == 1003 && concurrency == 1007)
    }

    async fn supports_result_set_holdability(
        &mut self,
        holdability: i32,
    ) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_supports_result_set_holdability")?;
        Ok(holdability == 2)
    }

    async fn get_result_set_holdability(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_result_set_holdability")?;
        Ok(2)
    }

    async fn get_sql_state_type(&mut self) -> Result<i32, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_get_sql_state_type")?;
        Ok(2)
    }

    async fn own_updates_are_visible(&mut self, _result_set_type: i32) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_own_updates_are_visible")?;
        Ok(false)
    }

    async fn own_deletes_are_visible(&mut self, _result_set_type: i32) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_own_deletes_are_visible")?;
        Ok(false)
    }

    async fn own_inserts_are_visible(&mut self, _result_set_type: i32) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_own_inserts_are_visible")?;
        Ok(false)
    }

    async fn others_updates_are_visible(
        &mut self,
        _result_set_type: i32,
    ) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_others_updates_are_visible")?;
        Ok(false)
    }

    async fn others_deletes_are_visible(
        &mut self,
        _result_set_type: i32,
    ) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_others_deletes_are_visible")?;
        Ok(false)
    }

    async fn others_inserts_are_visible(
        &mut self,
        _result_set_type: i32,
    ) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_others_inserts_are_visible")?;
        Ok(false)
    }

    async fn updates_are_detected(&mut self, _result_set_type: i32) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_updates_are_detected")?;
        Ok(false)
    }

    async fn deletes_are_detected(&mut self, _result_set_type: i32) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_deletes_are_detected")?;
        Ok(false)
    }

    async fn inserts_are_detected(&mut self, _result_set_type: i32) -> Result<bool, DruidError> {
        self.require_sqlite("sqlx_database_metadata_sqlite_inserts_are_detected")?;
        Ok(false)
    }

    async fn get_database_major_version(&mut self) -> Result<i32, DruidError> {
        self.version_component(0).await
    }

    async fn get_database_minor_version(&mut self) -> Result<i32, DruidError> {
        self.version_component(1).await
    }

    async fn get_tables(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
        types: Option<&[String]>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let rows = self
            .sqlite_tables(table_name_pattern, types)
            .await?
            .into_iter()
            .map(|(name, table_type)| {
                Row::new(vec![
                    Value::Null,
                    Value::Null,
                    Value::String(name),
                    Value::String(table_type),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                ])
            })
            .collect();
        Ok(Self::result_set(rows, Self::sqlite_table_labels()))
    }

    async fn get_schemas(&mut self) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_connection("sqlx_database_metadata_sqlite_get_schemas")?;
        Ok(Self::result_set(
            Vec::new(),
            &["TABLE_SCHEM", "TABLE_CATALOG"],
        ))
    }

    async fn get_catalogs(&mut self) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_connection("sqlx_database_metadata_sqlite_get_catalogs")?;
        Ok(Self::result_set(Vec::new(), &["TABLE_CAT"]))
    }

    async fn get_table_types(&mut self) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_connection("sqlx_database_metadata_sqlite_get_table_types")?;
        let rows = ["GLOBAL TEMPORARY", "SYSTEM TABLE", "TABLE", "VIEW"]
            .into_iter()
            .map(|table_type| Row::new(vec![Value::String(table_type.to_owned())]))
            .collect();
        Ok(Self::result_set(rows, &["TABLE_TYPE"]))
    }

    async fn get_columns(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let tables = self.sqlite_tables(table_name_pattern, None).await?;
        let column_pattern = column_name_pattern.unwrap_or("%");
        let mut output = Vec::new();

        for (table_name, _) in tables {
            if table_name == "sqlite_schema" {
                continue;
            }
            let create_sql = sqlx::query_scalar::<Sqlite, Option<String>>(
                "SELECT sql FROM sqlite_schema \
                 WHERE lower(name) = lower(?) AND type IN ('table', 'view')",
            )
            .bind(&table_name)
            .fetch_optional(self.sqlite_connection("sqlx_database_metadata_sqlite_get_columns")?)
            .await
            .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?
            .flatten();
            let has_auto_increment = create_sql
                .as_deref()
                .is_some_and(|sql| sql.to_ascii_lowercase().contains("autoincrement"));

            let columns = sqlx::query(
                "SELECT cid, name, type, \"notnull\", dflt_value, pk, hidden \
                 FROM pragma_table_xinfo(?) \
                 WHERE upper(name) LIKE upper(?) ESCAPE '\\' ORDER BY cid",
            )
            .bind(&table_name)
            .bind(column_pattern)
            .fetch_all(self.sqlite_connection("sqlx_database_metadata_sqlite_get_columns")?)
            .await
            .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;

            for (ordinal, column) in columns.into_iter().enumerate() {
                let column_name = column
                    .try_get::<String, _>(1)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let declared_type = column
                    .try_get::<Option<String>, _>(2)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let not_null = column
                    .try_get::<Option<i64>, _>(3)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let default_value = column
                    .try_get::<Option<String>, _>(4)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let primary_key_order = column
                    .try_get::<i64, _>(5)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let hidden = column
                    .try_get::<i64, _>(6)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let nullable = match not_null {
                    Some(0) => JDBC_COLUMN_NULLABLE,
                    Some(_) => JDBC_COLUMN_NO_NULLS,
                    None => JDBC_COLUMN_NULLABLE_UNKNOWN,
                };
                let nullable_name = match nullable {
                    JDBC_COLUMN_NO_NULLS => "NO",
                    JDBC_COLUMN_NULLABLE => "YES",
                    _ => "",
                };
                let (data_type, type_name, column_size, decimal_digits) =
                    Self::sqlite_declared_type(declared_type.as_deref());
                output.push(Row::new(vec![
                    Value::Null,
                    Value::Null,
                    Value::String(table_name.clone()),
                    Value::String(column_name),
                    Value::Int(data_type.into()),
                    Value::String(type_name),
                    Value::Int(column_size),
                    Value::Int(2_000_000_000),
                    Value::Int(decimal_digits),
                    Value::Int(10),
                    Value::Int(nullable.into()),
                    Value::Null,
                    default_value.map_or(Value::Null, Value::String),
                    Value::Int(0),
                    Value::Int(0),
                    Value::Int(2_000_000_000),
                    Value::Int((ordinal + 1) as i64),
                    Value::String(nullable_name.to_owned()),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::String(
                        if primary_key_order == 1 && has_auto_increment {
                            "YES"
                        } else {
                            "NO"
                        }
                        .to_owned(),
                    ),
                    Value::String(
                        if hidden == 2 || hidden == 3 {
                            "YES"
                        } else {
                            "NO"
                        }
                        .to_owned(),
                    ),
                ]));
            }
        }

        Ok(Self::result_set(output, Self::sqlite_column_labels()))
    }

    async fn get_procedures(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        _procedure_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_procedures",
            &[
                "PROCEDURE_CAT",
                "PROCEDURE_SCHEM",
                "PROCEDURE_NAME",
                "UNDEF1",
                "UNDEF2",
                "UNDEF3",
                "REMARKS",
                "PROCEDURE_TYPE",
            ],
        )
    }

    async fn get_procedure_columns(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        _procedure_name_pattern: Option<&str>,
        _column_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_procedure_columns",
            &[
                "PROCEDURE_CAT",
                "PROCEDURE_SCHEM",
                "PROCEDURE_NAME",
                "COLUMN_NAME",
                "COLUMN_TYPE",
                "DATA_TYPE",
                "TYPE_NAME",
                "PRECISION",
                "LENGTH",
                "SCALE",
                "RADIX",
                "NULLABLE",
                "REMARKS",
            ],
        )
    }

    async fn get_column_privileges(
        &mut self,
        _catalog: Option<&str>,
        _schema: Option<&str>,
        _table: Option<&str>,
        _column_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_column_privileges",
            &[
                "TABLE_CAT",
                "TABLE_SCHEM",
                "TABLE_NAME",
                "COLUMN_NAME",
                "GRANTOR",
                "GRANTEE",
                "PRIVILEGE",
                "IS_GRANTABLE",
            ],
        )
    }

    async fn get_table_privileges(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        _table_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_table_privileges",
            &[
                "TABLE_CAT",
                "TABLE_SCHEM",
                "TABLE_NAME",
                "GRANTOR",
                "GRANTEE",
                "PRIVILEGE",
                "IS_GRANTABLE",
            ],
        )
    }

    async fn get_best_row_identifier(
        &mut self,
        _catalog: Option<&str>,
        _schema: Option<&str>,
        _table: Option<&str>,
        _scope: i32,
        _nullable: bool,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_best_row_identifier",
            &[
                "SCOPE",
                "COLUMN_NAME",
                "DATA_TYPE",
                "TYPE_NAME",
                "COLUMN_SIZE",
                "BUFFER_LENGTH",
                "DECIMAL_DIGITS",
                "PSEUDO_COLUMN",
            ],
        )
    }

    async fn get_version_columns(
        &mut self,
        _catalog: Option<&str>,
        _schema: Option<&str>,
        _table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_version_columns",
            &[
                "SCOPE",
                "COLUMN_NAME",
                "DATA_TYPE",
                "TYPE_NAME",
                "COLUMN_SIZE",
                "BUFFER_LENGTH",
                "DECIMAL_DIGITS",
                "PSEUDO_COLUMN",
            ],
        )
    }

    async fn get_primary_keys(
        &mut self,
        _catalog: Option<&str>,
        _schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let table = table
            .ok_or_else(|| DruidError::DriverError("Invalid table name: 'null'".to_owned()))?;
        let labels = &[
            "TABLE_CAT",
            "TABLE_SCHEM",
            "TABLE_NAME",
            "COLUMN_NAME",
            "KEY_SEQ",
            "PK_NAME",
        ];
        let (_, primary_key_name, columns) = self
            .sqlite_primary_key_info(table, "sqlx_database_metadata_sqlite_get_primary_keys")
            .await?;

        let mut rows = columns
            .into_iter()
            .enumerate()
            .map(|(sequence, column)| {
                Row::new(vec![
                    Value::Null,
                    Value::Null,
                    Value::String(table.to_owned()),
                    Value::String(column),
                    Value::Int((sequence + 1) as i64),
                    primary_key_name.clone().map_or(Value::Null, Value::String),
                ])
            })
            .collect::<Vec<_>>();
        // Xerial 最终按 COLUMN_NAME 排序，同时保留 KEY_SEQ 的声明顺序。
        rows.sort_by(|left, right| match (&left.values[3], &right.values[3]) {
            (Value::String(left), Value::String(right)) => left.cmp(right),
            _ => std::cmp::Ordering::Equal,
        });
        Ok(Self::result_set(rows, labels))
    }

    async fn get_imported_keys(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let table = table
            .ok_or_else(|| DruidError::DriverError("Invalid table name: 'null'".to_owned()))?;
        let keys = self
            .sqlite_foreign_keys(table, "sqlx_database_metadata_sqlite_get_imported_keys")
            .await?;
        let mut rows = Vec::with_capacity(keys.len());
        for key in keys {
            let primary_key = self
                .sqlite_primary_key_info(
                    &key.primary_table,
                    "sqlx_database_metadata_sqlite_get_imported_keys",
                )
                .await
                .ok();
            let primary_column = key.primary_column.or_else(|| {
                primary_key
                    .as_ref()
                    .and_then(|(_, _, columns)| columns.first().cloned())
            });
            let primary_key_name = primary_key
                .and_then(|(_, name, _)| name)
                .unwrap_or_default();
            rows.push(Row::new(vec![
                Self::optional_string(catalog),
                Self::optional_string(schema),
                Value::String(key.primary_table),
                Value::String(primary_column.unwrap_or_default()),
                Self::optional_string(catalog),
                Self::optional_string(schema),
                Value::String(table.to_owned()),
                Value::String(key.foreign_column),
                Value::Int(key.sequence),
                Value::Int(key.update_rule.into()),
                Value::Int(key.delete_rule.into()),
                Value::String(key.name.unwrap_or_default()),
                Value::String(primary_key_name),
                Value::Int(JDBC_IMPORTED_KEY_INITIALLY_DEFERRED.into()),
            ]));
        }
        // Xerial 按主表和外键列序排列。
        rows.sort_by(|left, right| {
            let primary_table = match (&left.values[2], &right.values[2]) {
                (Value::String(left), Value::String(right)) => left.cmp(right),
                _ => std::cmp::Ordering::Equal,
            };
            primary_table.then_with(|| match (&left.values[8], &right.values[8]) {
                (Value::Int(left), Value::Int(right)) => left.cmp(right),
                _ => std::cmp::Ordering::Equal,
            })
        });
        Ok(Self::result_set(rows, Self::foreign_key_labels()))
    }

    async fn get_exported_keys(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let table = table
            .ok_or_else(|| DruidError::DriverError("Invalid table name: 'null'".to_owned()))?;
        let (canonical_table, primary_key_name, primary_columns) = self
            .sqlite_primary_key_info(table, "sqlx_database_metadata_sqlite_get_exported_keys")
            .await?;
        if primary_columns.is_empty() {
            return Ok(Self::result_set(Vec::new(), Self::foreign_key_labels()));
        }
        let table_names = sqlx::query_scalar::<Sqlite, String>(
            "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name",
        )
        .fetch_all(self.sqlite_connection("sqlx_database_metadata_sqlite_get_exported_keys")?)
        .await
        .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
        let mut rows = Vec::new();
        for foreign_table in table_names {
            let keys = self
                .sqlite_foreign_keys(
                    &foreign_table,
                    "sqlx_database_metadata_sqlite_get_exported_keys",
                )
                .await?;
            for key in keys
                .into_iter()
                .filter(|key| key.primary_table.eq_ignore_ascii_case(&canonical_table))
            {
                let primary_column = key.primary_column.unwrap_or_default();
                let use_primary_key_name = primary_columns
                    .iter()
                    .any(|column| column.eq_ignore_ascii_case(&primary_column));
                rows.push(Row::new(vec![
                    Self::optional_string(catalog),
                    Self::optional_string(schema),
                    Value::String(canonical_table.clone()),
                    Value::String(primary_column),
                    Self::optional_string(catalog),
                    Self::optional_string(schema),
                    Value::String(foreign_table.clone()),
                    Value::String(key.foreign_column),
                    Value::Int(key.sequence),
                    Value::Int(key.update_rule.into()),
                    Value::Int(key.delete_rule.into()),
                    Value::String(key.name.unwrap_or_default()),
                    Value::String(
                        if use_primary_key_name {
                            primary_key_name.clone()
                        } else {
                            None
                        }
                        .unwrap_or_default(),
                    ),
                    Value::Int(JDBC_IMPORTED_KEY_INITIALLY_DEFERRED.into()),
                ]));
            }
        }
        rows.sort_by(|left, right| {
            let foreign_table = match (&left.values[6], &right.values[6]) {
                (Value::String(left), Value::String(right)) => left.cmp(right),
                _ => std::cmp::Ordering::Equal,
            };
            foreign_table.then_with(|| match (&left.values[8], &right.values[8]) {
                (Value::Int(left), Value::Int(right)) => left.cmp(right),
                _ => std::cmp::Ordering::Equal,
            })
        });
        Ok(Self::result_set(rows, Self::foreign_key_labels()))
    }

    async fn get_cross_reference(
        &mut self,
        parent_catalog: Option<&str>,
        parent_schema: Option<&str>,
        parent_table: Option<&str>,
        foreign_catalog: Option<&str>,
        foreign_schema: Option<&str>,
        foreign_table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        if parent_table.is_none() {
            return self
                .get_exported_keys(foreign_catalog, foreign_schema, foreign_table)
                .await;
        }
        if foreign_table.is_none() {
            return self
                .get_imported_keys(parent_catalog, parent_schema, parent_table)
                .await;
        }
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_cross_reference",
            Self::foreign_key_labels(),
        )
    }

    async fn get_type_info(&mut self) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_connection("sqlx_database_metadata_sqlite_get_type_info")?;
        let labels = &[
            "TYPE_NAME",
            "DATA_TYPE",
            "PRECISION",
            "LITERAL_PREFIX",
            "LITERAL_SUFFIX",
            "CREATE_PARAMS",
            "NULLABLE",
            "CASE_SENSITIVE",
            "SEARCHABLE",
            "UNSIGNED_ATTRIBUTE",
            "FIXED_PREC_SCALE",
            "AUTO_INCREMENT",
            "LOCAL_TYPE_NAME",
            "MINIMUM_SCALE",
            "MAXIMUM_SCALE",
            "SQL_DATA_TYPE",
            "SQL_DATETIME_SUB",
            "NUM_PREC_RADIX",
        ];
        let type_row = |name: &str,
                        data_type: i32,
                        case_sensitive: bool,
                        unsigned: bool,
                        auto_increment: bool| {
            Row::new(vec![
                Value::String(name.to_owned()),
                Value::Int(data_type.into()),
                Value::Int(0),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Int(JDBC_TYPE_NULLABLE.into()),
                Value::Int(i64::from(case_sensitive)),
                Value::Int(JDBC_TYPE_SEARCHABLE.into()),
                Value::Int(i64::from(unsigned)),
                Value::Int(0),
                Value::Int(i64::from(auto_increment)),
                Value::Null,
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(10),
            ])
        };
        let rows = vec![
            type_row("NULL", JDBC_TYPE_NULL, false, true, false),
            type_row("INTEGER", JDBC_TYPE_INTEGER, false, false, true),
            type_row("REAL", JDBC_TYPE_REAL, false, false, false),
            type_row("TEXT", JDBC_TYPE_VARCHAR, true, true, false),
            type_row("BLOB", JDBC_TYPE_BLOB, false, true, false),
        ];
        Ok(Self::result_set(rows, labels))
    }

    async fn get_ud_ts(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        _type_name_pattern: Option<&str>,
        _types: Option<&[i32]>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_ud_ts",
            &[
                "TYPE_CAT",
                "TYPE_SCHEM",
                "TYPE_NAME",
                "CLASS_NAME",
                "DATA_TYPE",
                "REMARKS",
                "BASE_TYPE",
            ],
        )
    }

    async fn get_super_types(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        _type_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_super_types",
            &[
                "TYPE_CAT",
                "TYPE_SCHEM",
                "TYPE_NAME",
                "SUPERTYPE_CAT",
                "SUPERTYPE_SCHEM",
                "SUPERTYPE_NAME",
            ],
        )
    }

    async fn get_super_tables(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        _table_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_super_tables",
            &["TABLE_CAT", "TABLE_SCHEM", "TABLE_NAME", "SUPERTABLE_NAME"],
        )
    }

    async fn get_attributes(
        &mut self,
        _catalog: Option<&str>,
        _schema_pattern: Option<&str>,
        _type_name_pattern: Option<&str>,
        _attribute_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.sqlite_empty_result_set(
            "sqlx_database_metadata_sqlite_get_attributes",
            &[
                "TYPE_CAT",
                "TYPE_SCHEM",
                "TYPE_NAME",
                "ATTR_NAME",
                "DATA_TYPE",
                "ATTR_TYPE_NAME",
                "ATTR_SIZE",
                "DECIMAL_DIGITS",
                "NUM_PREC_RADIX",
                "NULLABLE",
                "REMARKS",
                "ATTR_DEF",
                "SQL_DATA_TYPE",
                "SQL_DATETIME_SUB",
                "CHAR_OCTET_LENGTH",
                "ORDINAL_POSITION",
                "IS_NULLABLE",
                "SCOPE_CATALOG",
                "SCOPE_SCHEMA",
                "SCOPE_TABLE",
                "SOURCE_DATA_TYPE",
            ],
        )
    }

    async fn get_index_info(
        &mut self,
        _catalog: Option<&str>,
        _schema: Option<&str>,
        table: Option<&str>,
        _unique: bool,
        _approximate: bool,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let table = table
            .ok_or_else(|| DruidError::DriverError("Invalid table name: 'null'".to_owned()))?;
        let labels = &[
            "TABLE_CAT",
            "TABLE_SCHEM",
            "TABLE_NAME",
            "NON_UNIQUE",
            "INDEX_QUALIFIER",
            "INDEX_NAME",
            "TYPE",
            "ORDINAL_POSITION",
            "COLUMN_NAME",
            "ASC_OR_DESC",
            "CARDINALITY",
            "PAGES",
            "FILTER_CONDITION",
        ];
        let indexes = sqlx::query("SELECT name, \"unique\" FROM pragma_index_list(?) ORDER BY seq")
            .bind(table)
            .fetch_all(self.sqlite_connection("sqlx_database_metadata_sqlite_get_index_info")?)
            .await
            .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
        let mut rows = Vec::new();
        for index in indexes {
            let index_name = index
                .try_get::<String, _>(0)
                .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
            let unique = index
                .try_get::<i64, _>(1)
                .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
            let columns =
                sqlx::query("SELECT seqno, name FROM pragma_index_info(?) ORDER BY seqno")
                    .bind(&index_name)
                    .fetch_all(
                        self.sqlite_connection("sqlx_database_metadata_sqlite_get_index_info")?,
                    )
                    .await
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
            for column in columns {
                let sequence = column
                    .try_get::<i64, _>(0)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                let column_name = column
                    .try_get::<Option<String>, _>(1)
                    .map_err(super::sqlx_connection_adapter::SqlxConnectionAdapter::driver_error)?;
                rows.push(Row::new(vec![
                    Value::Null,
                    Value::Null,
                    Value::String(table.to_owned()),
                    Value::Bool(unique == 0),
                    Value::Null,
                    Value::String(index_name.clone()),
                    Value::Int(JDBC_TABLE_INDEX_OTHER.into()),
                    Value::Int(sequence + 1),
                    column_name.map_or(Value::Null, Value::String),
                    Value::Null,
                    Value::Int(0),
                    Value::Int(0),
                    Value::Null,
                ]));
            }
        }
        Ok(Self::result_set(rows, labels))
    }
}
