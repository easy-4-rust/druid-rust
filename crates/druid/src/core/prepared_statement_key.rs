//! Druid 私有 PreparedStatement cache key。
//!
//! 对应 Java：`DruidPooledPreparedStatement.PreparedStatementKey`。

use super::prepared_statement_method_type::PreparedStatementMethodType;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{GeneratedKeys, StatementOptions};

/// 完整区分 SQL、catalog、重载与结果集选项的 Druid 私有缓存键。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PreparedStatementKey {
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
    pub(crate) fn standard(
        sql: &str,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        options: StatementOptions,
        generated_keys: GeneratedKeys,
    ) -> Result<Self, RdbcError> {
        if sql.is_empty() {
            return Err(RdbcError::InvalidArgument("sql is empty".to_string()));
        }
        // 游标类型和并发模式都参与缓存身份，保留标准及驱动原数字。
        let result_set_type = options.result_set_type().rdbc_code();
        let result_set_concurrency = options.concurrency().rdbc_code();
        // 原始整数是 prepare 重载身份的一部分，未知值也不能被合并为0。
        let result_set_holdability = options.holdability().rdbc_code();
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

    pub(crate) fn new(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
    ) -> Result<Self, RdbcError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, None, None)
    }

    pub(crate) fn with_result_set(
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

    pub(crate) fn with_result_set_holdability(
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

    pub(crate) fn with_auto_generated_keys(
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

    pub(crate) fn with_column_indexes(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        column_indexes: Option<Vec<i32>>,
    ) -> Result<Self, RdbcError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, column_indexes, None)
    }

    pub(crate) fn with_column_names(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        column_names: Option<Vec<String>>,
    ) -> Result<Self, RdbcError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, None, column_names)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn full(
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
    pub(crate) fn sql(&self) -> &str {
        &self.sql
    }

    pub(crate) fn with_sql(mut self, sql: impl Into<String>) -> Self {
        self.sql = sql.into();
        self
    }

    /// 返回 catalog。
    pub(crate) fn catalog(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    /// 返回 prepare 重载种类。
    pub(crate) const fn method_type(&self) -> PreparedStatementMethodType {
        self.method_type
    }

    pub(crate) const fn result_set_type(&self) -> i32 {
        self.result_set_type
    }

    pub(crate) const fn result_set_concurrency(&self) -> i32 {
        self.result_set_concurrency
    }

    pub(crate) const fn result_set_holdability(&self) -> i32 {
        self.result_set_holdability
    }

    pub(crate) const fn auto_generated_keys(&self) -> i32 {
        self.auto_generated_keys
    }

    pub(crate) fn column_indexes(&self) -> Option<&[i32]> {
        self.column_indexes.as_deref()
    }

    pub(crate) fn column_names(&self) -> Option<&[String]> {
        self.column_names.as_deref()
    }

    pub(crate) fn statement_generated_keys(&self) -> GeneratedKeys {
        if let Some(indexes) = &self.column_indexes {
            GeneratedKeys::ColumnIndexes(
                indexes
                    .iter()
                    .filter_map(|index| usize::try_from(*index).ok())
                    .filter_map(|index| druid_rdbc::result_set::ColumnIndex::new(index).ok())
                    .collect(),
            )
        } else if let Some(names) = &self.column_names {
            GeneratedKeys::ColumnNames(names.clone())
        } else if self.auto_generated_keys != 0 {
            GeneratedKeys::ReturnGeneratedKeys
        } else {
            GeneratedKeys::None
        }
    }
}

#[cfg(test)]
mod holdability_tests;
