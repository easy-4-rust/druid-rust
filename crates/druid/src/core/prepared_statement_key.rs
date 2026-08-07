//! 预编译语句缓存键。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidPooledPreparedStatement.PreparedStatementKey`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/DruidPooledPreparedStatement.java`。

use super::{DruidError, StatementGeneratedKeys};

/// 创建预编译语句所调用的 RDBC 重载。
///
/// 对应 Java：`PreparedStatementPool.MethodType`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreparedStatementMethodType {
    /// `prepareStatement(String)`。
    M1,
    /// `prepareStatement(String, int, int)`。
    M2,
    /// `prepareStatement(String, int, int, int)`。
    M3,
    /// `prepareStatement(String, int[])`。
    M4,
    /// `prepareStatement(String, String[])`。
    M5,
    /// `prepareStatement(String, int)`。
    M6,
    /// `prepareCall(String)`。
    Precall1,
    /// `prepareCall(String, int, int, int)`。
    Precall2,
    /// `prepareCall(String, int, int)`。
    Precall3,
}

/// 完整区分 RDBC PreparedStatement/CallableStatement 重载的缓存键。
///
/// Java 的相等性同时比较 SQL、catalog、方法重载、结果集属性、自动生成键、
/// 列序号和列名；Rust 保留同一字段集合，不能仅按 SQL 文本命中缓存。
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

impl PreparedStatementKey {
    /// 创建无额外结果集参数的缓存键。
    ///
    /// 对应 Java：
    /// `PreparedStatementKey(String, String, MethodType)`。
    ///
    /// # 参数
    /// - `sql`：Java 可空参数 `sql`；`None` 对应 Java `null`。
    /// - `catalog`：连接当前 catalog。
    /// - `method_type`：调用的 RDBC 重载。
    ///
    /// # 错误
    /// `sql` 为 `None` 时返回与 Java `SQLException("sql is null")` 同类别的参数错误。
    pub fn new(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
    ) -> Result<Self, DruidError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, None, None)
    }

    /// 创建带结果集类型和并发模式的缓存键。
    ///
    /// 对应 Java：
    /// `PreparedStatementKey(String, String, MethodType, int, int)`。
    pub fn with_result_set(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        result_set_type: i32,
        result_set_concurrency: i32,
    ) -> Result<Self, DruidError> {
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

    /// 创建带完整结果集属性的缓存键。
    ///
    /// 对应 Java：
    /// `PreparedStatementKey(String, String, MethodType, int, int, int)`。
    pub fn with_result_set_holdability(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        result_set_type: i32,
        result_set_concurrency: i32,
        result_set_holdability: i32,
    ) -> Result<Self, DruidError> {
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

    /// 创建带自动生成键模式的缓存键。
    ///
    /// 对应 Java：
    /// `PreparedStatementKey(String, String, MethodType, int)`。
    pub fn with_auto_generated_keys(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        auto_generated_keys: i32,
    ) -> Result<Self, DruidError> {
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

    /// 创建带生成键列序号的缓存键。
    ///
    /// 对应 Java：
    /// `PreparedStatementKey(String, String, MethodType, int[])`。
    pub fn with_column_indexes(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        column_indexes: Option<Vec<i32>>,
    ) -> Result<Self, DruidError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, column_indexes, None)
    }

    /// 创建带生成键列名的缓存键。
    ///
    /// 对应 Java：
    /// `PreparedStatementKey(String, String, MethodType, String[])`。
    pub fn with_column_names(
        sql: Option<String>,
        catalog: Option<String>,
        method_type: PreparedStatementMethodType,
        column_names: Option<Vec<String>>,
    ) -> Result<Self, DruidError> {
        Self::full(sql, catalog, method_type, 0, 0, 0, 0, None, column_names)
    }

    /// 创建包含全部 Java 字段的缓存键。
    ///
    /// 对应 Java 九参数构造器。
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
    ) -> Result<Self, DruidError> {
        let sql = sql.ok_or_else(|| DruidError::InvalidArgument("sql is null".to_string()))?;
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

    /// 替换 SQL，同时保留 catalog、prepare 重载和 generated-key 参数。
    ///
    /// 对应 Java Filter 在 `connection_prepareStatement` 下游调用前替换 SQL；
    /// 改写后的文本必须参与 PreparedStatement cache equality。
    #[must_use]
    pub fn with_sql(mut self, sql: impl Into<String>) -> Self {
        self.sql = sql.into();
        self
    }

    /// 返回 catalog。
    pub fn catalog(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    /// 返回创建语句所使用的 RDBC 重载。
    pub fn method_type(&self) -> PreparedStatementMethodType {
        self.method_type
    }

    /// 返回结果集类型。
    pub fn result_set_type(&self) -> i32 {
        self.result_set_type
    }

    /// 返回结果集并发模式。
    pub fn result_set_concurrency(&self) -> i32 {
        self.result_set_concurrency
    }

    /// 返回结果集保持性。
    pub fn result_set_holdability(&self) -> i32 {
        self.result_set_holdability
    }

    /// 返回自动生成键模式。
    pub fn auto_generated_keys(&self) -> i32 {
        self.auto_generated_keys
    }

    /// 返回生成键列序号。
    pub fn column_indexes(&self) -> Option<&[i32]> {
        self.column_indexes.as_deref()
    }

    /// 返回生成键列名。
    pub fn column_names(&self) -> Option<&[String]> {
        self.column_names.as_deref()
    }

    /// 返回本缓存键对应的 RDBC generated-keys 执行参数。
    ///
    /// 对应 Java：`Connection#prepareStatement` 的 `int`、`int[]` 与
    /// `String[]` 重载。参数必须在后续 `PreparedStatement#execute()` 时继续
    /// 交给物理 Adapter，不能因为 prepare 已完成就丢失重载语义。
    pub fn statement_generated_keys(&self) -> StatementGeneratedKeys {
        match self.method_type {
            PreparedStatementMethodType::M4 => StatementGeneratedKeys::ColumnIndexes(
                self.column_indexes.clone().unwrap_or_default(),
            ),
            PreparedStatementMethodType::M5 => {
                StatementGeneratedKeys::ColumnNames(self.column_names.clone().unwrap_or_default())
            }
            PreparedStatementMethodType::M6 => {
                StatementGeneratedKeys::AutoGeneratedKeys(self.auto_generated_keys)
            }
            PreparedStatementMethodType::M1
            | PreparedStatementMethodType::M2
            | PreparedStatementMethodType::M3
            | PreparedStatementMethodType::Precall1
            | PreparedStatementMethodType::Precall2
            | PreparedStatementMethodType::Precall3 => StatementGeneratedKeys::None,
        }
    }
}
