//! Druid 私有 PreparedStatement cache key。
//!
//! 对应 Java：`DruidPooledPreparedStatement.PreparedStatementKey`。

use super::prepared_statement_method_type::PreparedStatementMethodType;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{GeneratedKeys, StatementOptions};

/// 完整区分 SQL、catalog、重载与结果集选项的 Druid 私有缓存键。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreparedStatementKey {
    sql: String,
    catalog: Option<String>,
    method_type: PreparedStatementMethodType,
    result_set_type: i32,
    result_set_concurrency: i32,
    result_set_holdability: i32,
    auto_generated_keys: i32,
    column_indexes: Option<Vec<i32>>,
    column_names: Option<Vec<String>>,
}

#[allow(dead_code)] // Task 5 freezes the full Java overload key; later overloads consume accessors.
impl PreparedStatementKey {
    /// 创建 canonical standard PreparedStatement 缓存键。
    pub fn standard(
        sql: &str,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        options: StatementOptions,
        generated_keys: GeneratedKeys,
    ) -> Result<Self, RdbcError> {
        if sql.is_empty() {
            return Err(RdbcError::InvalidArgument("sql is empty".to_string()));
        }
        let result_set_type = match options.result_set_type() {
            druid_rdbc::statement::ResultSetType::ForwardOnly => 1003,
            druid_rdbc::statement::ResultSetType::ScrollInsensitive => 1004,
            druid_rdbc::statement::ResultSetType::ScrollSensitive => 1005,
            _ => 0,
        };
        let result_set_concurrency = match options.concurrency() {
            druid_rdbc::statement::ResultSetConcurrency::ReadOnly => 1007,
            druid_rdbc::statement::ResultSetConcurrency::Updatable => 1008,
            _ => 0,
        };
        let result_set_holdability = match options.holdability() {
            druid_rdbc::statement::ResultSetHoldability::CloseAtCommit => 2,
            druid_rdbc::statement::ResultSetHoldability::HoldOverCommit => 1,
            _ => 0,
        };
        let (auto_generated_keys, column_indexes, column_names) = match generated_keys {
            GeneratedKeys::None => (0, None, None),
            GeneratedKeys::ReturnGeneratedKeys => (1, None, None),
            GeneratedKeys::ColumnIndexes(indexes) => (
                0,
                Some(
                    indexes
                        .into_iter()
                        .map(|index| i32::try_from(index.get()))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| {
                            RdbcError::InvalidArgument(
                                "generated key column index exceeds i32".to_string(),
                            )
                        })?,
                ),
                None,
            ),
            GeneratedKeys::ColumnNames(names) => (0, None, Some(names)),
            _ => {
                return Err(RdbcError::UnsupportedOperation {
                    operation: "prepared_cache_generated_keys_variant",
                });
            }
        };
        Ok(Self {
            sql: sql.to_string(),
            catalog,
            method_type,
            result_set_type,
            result_set_concurrency,
            result_set_holdability,
            auto_generated_keys,
            column_indexes,
            column_names,
        })
    }

    pub fn new(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
    ) -> Result<Self, RdbcError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, None, None)
    }

    pub fn with_result_set(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        result_set_type: i32,
        result_set_concurrency: i32,
    ) -> Result<Self, RdbcError> {
        Self::full(
            sql,
            catalog,
            method_type,
            result_set_type,
            result_set_concurrency,
            0,
            0,
            None,
            None,
        )
    }

    pub fn with_result_set_holdability(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        result_set_type: i32,
        result_set_concurrency: i32,
        result_set_holdability: i32,
    ) -> Result<Self, RdbcError> {
        Self::full(
            sql,
            catalog,
            method_type,
            result_set_type,
            result_set_concurrency,
            result_set_holdability,
            0,
            None,
            None,
        )
    }

    pub fn with_auto_generated_keys(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        auto_generated_keys: i32,
    ) -> Result<Self, RdbcError> {
        Self::full(
            sql,
            catalog,
            method_type,
            0,
            0,
            0,
            auto_generated_keys,
            None,
            None,
        )
    }

    pub fn with_column_indexes(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        column_indexes: Option<Vec<i32>>,
    ) -> Result<Self, RdbcError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, column_indexes, None)
    }

    pub fn with_column_names(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        column_names: Option<Vec<String>>,
    ) -> Result<Self, RdbcError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, None, column_names)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn full(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        result_set_type: i32,
        result_set_concurrency: i32,
        result_set_holdability: i32,
        auto_generated_keys: i32,
        column_indexes: Option<Vec<i32>>,
        column_names: Option<Vec<String>>,
    ) -> Result<Self, RdbcError> {
        let sql = sql.ok_or_else(|| RdbcError::InvalidArgument("sql is null".to_string()))?;
        Ok(Self {
            sql,
            catalog,
            method_type,
            result_set_type,
            result_set_concurrency,
            result_set_holdability,
            auto_generated_keys,
            column_indexes,
            column_names,
        })
    }

    /// 返回 SQL 文本。
    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn with_sql(mut self, sql: impl Into<String>) -> Self {
        self.sql = sql.into();
        self
    }

    /// 返回 catalog。
    pub fn catalog(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    /// 返回 prepare 重载种类。
    pub const fn method_type(&self) -> PreparedStatementMethodType {
        self.method_type
    }

    pub const fn result_set_type(&self) -> i32 {
        self.result_set_type
    }

    pub const fn result_set_concurrency(&self) -> i32 {
        self.result_set_concurrency
    }

    pub const fn result_set_holdability(&self) -> i32 {
        self.result_set_holdability
    }

    pub const fn auto_generated_keys(&self) -> i32 {
        self.auto_generated_keys
    }

    pub fn column_indexes(&self) -> Option<&[i32]> {
        self.column_indexes.as_deref()
    }

    pub fn column_names(&self) -> Option<&[String]> {
        self.column_names.as_deref()
    }

    pub fn statement_generated_keys(&self) -> druid_rdbc::spi::StatementGeneratedKeys {
        if let Some(indexes) = &self.column_indexes {
            druid_rdbc::spi::StatementGeneratedKeys::ColumnIndexes(indexes.clone())
        } else if let Some(names) = &self.column_names {
            druid_rdbc::spi::StatementGeneratedKeys::ColumnNames(names.clone())
        } else if self.auto_generated_keys != 0 {
            druid_rdbc::spi::StatementGeneratedKeys::AutoGeneratedKeys(self.auto_generated_keys)
        } else {
            druid_rdbc::spi::StatementGeneratedKeys::None
        }
    }
}
