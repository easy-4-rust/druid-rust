//! 对应 Java：
//! `com.alibaba.druid.filter.mysql8datetime.MySQL8DateTimeSqlTypeFilter`。

use super::MySQL8DateTimeResultSetMetaData;
use crate::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecResult, ResultSetFilter,
    ResultSetFilterChain, ResultSetMetaData, Value,
};
use std::time::Duration;

/// `MySQL` Connector/J 8.0.23+ DATETIME 兼容 Filter。
///
/// Java 驱动把 `getObject` 的 DATETIME 从 `Timestamp` 改为
/// `LocalDateTime`。Rust RDBC 值模型从一开始就把无时区 SQL
/// TIMESTAMP/DATETIME 表示为 `Value::Timestamp`，因此值替换是恒等操作；
/// metadata 的 Java 类名仍需显式恢复。
#[derive(Debug, Default, Clone, Copy)]
pub struct MySQL8DateTimeSqlTypeFilter;

impl MySQL8DateTimeSqlTypeFilter {
    /// 创建兼容 Filter。
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// 把驱动对象恢复为旧版 Timestamp 语义。
    ///
    /// Rust 中 `Value::Timestamp` 已是该 canonical 表示，其他值原样返回。
    #[must_use]
    pub fn get_object_replace_local_date_time(object: Value) -> Value {
        object
    }
}

#[async_trait::async_trait]
impl BeforeFilter for MySQL8DateTimeSqlTypeFilter {
    fn name(&self) -> &str {
        "mysql8DateTime"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for MySQL8DateTimeSqlTypeFilter {
    fn name(&self) -> &str {
        "mysql8DateTime"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }
}

impl ResultSetFilter for MySQL8DateTimeSqlTypeFilter {
    fn result_set_get_object(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        chain
            .result_set_get_object(column_index)
            .map(Self::get_object_replace_local_date_time)
    }

    fn result_set_get_object_by_label(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        chain
            .result_set_get_object_by_label(column_label)
            .map(Self::get_object_replace_local_date_time)
    }

    fn result_set_get_meta_data(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<ResultSetMetaData, DruidError> {
        chain.result_set_get_meta_data().map(|metadata| {
            MySQL8DateTimeResultSetMetaData::new(metadata).into_result_set_meta_data()
        })
    }
}
