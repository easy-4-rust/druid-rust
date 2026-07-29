//! 对应 Java 类：com.alibaba.druid.filter.FilterChain + FilterChainImpl

use super::connection::ExecResult;
use super::error::DruidError;
use super::filter::{AfterFilter, BatchExecContext, BeforeFilter, ConnectionEvent, ExecContext};
use super::{
    JdbcArray, JdbcBlob, JdbcCalendarArgument, JdbcClob, JdbcInputStream, JdbcNClob, JdbcObject,
    JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcTargetType, JdbcTypeMap, JdbcUrl,
    PhysicalConnection, PhysicalPreparedStatement, PhysicalResultSet, PhysicalStatement,
    ResultSetFilter, ResultSetFilterChain, ResultSetFilterContext, SqlWarning, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::sync::Arc;
use std::time::Duration;

macro_rules! scalar_result_set_filter_chain_methods {
    ($(($index:ident, $label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
            ) -> Result<$ty, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
            ) -> Result<$ty, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label)
            }
        )+
    };
}

macro_rules! temporal_result_set_filter_chain_methods {
    ($(($index:ident, $label:ident, $index_calendar:ident, $label_calendar:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, Calendar)` around-chain。")]
            pub fn $index_calendar(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                calendar: &JdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index_calendar(column_index, calendar)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, Calendar)` around-chain。")]
            pub fn $label_calendar(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                calendar: &JdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label_calendar(column_label, calendar)
            }
        )+
    };
}

macro_rules! resource_result_set_filter_chain_methods {
    ($(($index:ident, $label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label)
            }
        )+
    };
}

/// Filter 链。
pub struct FilterChain {
    before: Vec<Arc<dyn BeforeFilter>>,
    after: Vec<Arc<dyn AfterFilter>>,
    result_set: Vec<Arc<dyn ResultSetFilter>>,
}

/// 单次 Connection warning 操作使用的有位置 Filter 调用链。
///
/// 对应 Java：`FilterChainImpl#connection_getWarnings` 与
/// `connection_clearWarnings`。每次调用都从位置 0 开始，末端才进入物理连接。
pub struct ConnectionWarningFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    physical: &'a mut dyn PhysicalConnection,
}

impl<'a> ConnectionWarningFilterChain<'a> {
    fn new(filters: &'a [Arc<dyn BeforeFilter>], physical: &'a mut dyn PhysicalConnection) -> Self {
        Self {
            filters,
            position: 0,
            physical,
        }
    }

    /// 继续分派 `Connection#getWarnings()`。
    pub async fn connection_get_warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_get_warnings(self).await
        } else {
            self.physical.warnings().await
        }
    }

    /// 继续分派 `Connection#clearWarnings()`。
    pub async fn connection_clear_warnings(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_clear_warnings(self).await
        } else {
            self.physical.clear_warnings().await
        }
    }
}

/// 单次 Statement warning 操作使用的有位置 Filter 调用链。
///
/// 对应 Java：`FilterChainImpl#statement_getWarnings` 与
/// `statement_clearWarnings`。PreparedStatement 使用同一 Java 继承语义。
pub struct StatementWarningFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    physical: StatementWarningTarget<'a>,
}

enum StatementWarningTarget<'a> {
    Statement(&'a dyn PhysicalStatement),
    PreparedStatement(&'a dyn PhysicalPreparedStatement),
}

impl<'a> StatementWarningFilterChain<'a> {
    fn new_statement(
        filters: &'a [Arc<dyn BeforeFilter>],
        physical: &'a dyn PhysicalStatement,
    ) -> Self {
        Self {
            filters,
            position: 0,
            physical: StatementWarningTarget::Statement(physical),
        }
    }

    fn new_prepared_statement(
        filters: &'a [Arc<dyn BeforeFilter>],
        physical: &'a dyn PhysicalPreparedStatement,
    ) -> Self {
        Self {
            filters,
            position: 0,
            physical: StatementWarningTarget::PreparedStatement(physical),
        }
    }

    /// 继续分派 `Statement#getWarnings()`。
    pub async fn statement_get_warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.statement_get_warnings(self).await
        } else {
            match self.physical {
                StatementWarningTarget::Statement(physical) => physical.warnings(),
                StatementWarningTarget::PreparedStatement(physical) => physical.warnings(),
            }
        }
    }

    /// 继续分派 `Statement#clearWarnings()`。
    pub async fn statement_clear_warnings(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.statement_clear_warnings(self).await
        } else {
            match self.physical {
                StatementWarningTarget::Statement(physical) => physical.clear_warnings(),
                StatementWarningTarget::PreparedStatement(physical) => physical.clear_warnings(),
            }
        }
    }
}

impl FilterChain {
    pub fn new() -> Self {
        Self {
            before: Vec::new(),
            after: Vec::new(),
            result_set: Vec::new(),
        }
    }
    pub fn add_before(&mut self, filter: Arc<dyn BeforeFilter>) {
        self.before.push(filter);
    }
    pub fn add_after(&mut self, filter: Arc<dyn AfterFilter>) {
        self.after.push(filter);
    }
    /// 添加同步 `ResultSet Filter`，保持 Java `Filter` 注册顺序。
    pub fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.result_set.push(filter);
    }

    /// 把同一个 Java 风格 Filter 实例注册到全部已迁移的调用族。
    ///
    /// Java 一个 `Filter` 对象同时接收 before、after 与 ResultSet 方法；该入口
    /// 保证三个 Rust trait 视图共享同一实例与注册位置，避免调用方漏接其中一族。
    pub fn add_filter<T>(&mut self, filter: Arc<T>)
    where
        T: BeforeFilter + AfterFilter + ResultSetFilter + 'static,
    {
        self.before
            .push(Arc::clone(&filter) as Arc<dyn BeforeFilter>);
        self.after.push(Arc::clone(&filter) as Arc<dyn AfterFilter>);
        self.result_set.push(filter);
    }
    pub fn before_count(&self) -> usize {
        self.before.len()
    }
    pub fn after_count(&self) -> usize {
        self.after.len()
    }
    /// 返回 `ResultSet Filter` 数。
    pub fn result_set_count(&self) -> usize {
        self.result_set.len()
    }

    /// 在查询成功并建立结果集后按调用栈逆序执行 open-after hook。
    pub fn result_set_open_after(
        &self,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        for filter in self.result_set.iter().rev() {
            filter.result_set_open_after(context)?;
        }
        Ok(())
    }

    /// 从位置 0 执行一次完整的 ResultSet next around-chain。
    pub fn result_set_next(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_next()
    }

    /// 从位置 0 执行一次完整的 ResultSet close around-chain。
    pub fn result_set_close(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_close()
    }

    /// 从位置 0 执行一次 `ResultSet#getWarnings()` around-chain。
    pub fn result_set_warnings(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<Option<SqlWarning>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_get_warnings()
    }

    /// 从位置 0 执行一次 `ResultSet#clearWarnings()` around-chain。
    pub fn result_set_clear_warnings(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_clear_warnings()
    }

    /// 从位置 0 执行 `ResultSet#getObject(int)` around-chain。
    pub fn result_set_get_object(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object(column_index)
    }

    /// 从位置 0 执行 `ResultSet#getObject(String)` around-chain。
    pub fn result_set_get_object_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_by_label(column_label)
    }

    /// 从位置 0 执行 `ResultSet#getObject(int, Map)` around-chain。
    pub fn result_set_get_object_with_type_map(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_with_type_map(column_index, type_map)
    }

    /// 从位置 0 执行 `ResultSet#getObject(String, Map)` around-chain。
    pub fn result_set_get_object_by_label_with_type_map(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_by_label_with_type_map(column_label, type_map)
    }

    /// 从位置 0 执行 `ResultSet#getObject(int, Class<T>)` around-chain。
    pub fn result_set_get_object_typed(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_typed(column_index, target_type)
    }

    /// 从位置 0 执行 `ResultSet#getObject(String, Class<T>)` around-chain。
    pub fn result_set_get_object_typed_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_typed_by_label(column_label, target_type)
    }

    scalar_result_set_filter_chain_methods!(
        (
            result_set_get_string,
            result_set_get_string_by_label,
            Option<String>,
            "getString"
        ),
        (
            result_set_get_boolean,
            result_set_get_boolean_by_label,
            bool,
            "getBoolean"
        ),
        (
            result_set_get_byte,
            result_set_get_byte_by_label,
            i8,
            "getByte"
        ),
        (
            result_set_get_short,
            result_set_get_short_by_label,
            i16,
            "getShort"
        ),
        (
            result_set_get_int,
            result_set_get_int_by_label,
            i32,
            "getInt"
        ),
        (
            result_set_get_long,
            result_set_get_long_by_label,
            i64,
            "getLong"
        ),
        (
            result_set_get_float,
            result_set_get_float_by_label,
            f32,
            "getFloat"
        ),
        (
            result_set_get_double,
            result_set_get_double_by_label,
            f64,
            "getDouble"
        ),
        (
            result_set_get_bytes,
            result_set_get_bytes_by_label,
            Option<Vec<u8>>,
            "getBytes"
        ),
    );

    /// 从位置 0 执行 `ResultSet#getBigDecimal(int)` around-chain。
    pub fn result_set_get_big_decimal(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_big_decimal(column_index)
    }

    /// 从位置 0 执行 `ResultSet#getBigDecimal(String)` around-chain。
    pub fn result_set_get_big_decimal_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_big_decimal_by_label(column_label)
    }

    /// 从位置 0 执行 `ResultSet#getBigDecimal(int, int)` around-chain。
    pub fn result_set_get_big_decimal_with_scale(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_big_decimal_with_scale(column_index, scale)
    }

    /// 从位置 0 执行 `ResultSet#getBigDecimal(String, int)` around-chain。
    pub fn result_set_get_big_decimal_by_label_with_scale(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_big_decimal_by_label_with_scale(column_label, scale)
    }

    temporal_result_set_filter_chain_methods!(
        (
            result_set_get_date,
            result_set_get_date_by_label,
            result_set_get_date_with_calendar,
            result_set_get_date_by_label_with_calendar,
            NaiveDate,
            "getDate"
        ),
        (
            result_set_get_time,
            result_set_get_time_by_label,
            result_set_get_time_with_calendar,
            result_set_get_time_by_label_with_calendar,
            NaiveTime,
            "getTime"
        ),
        (
            result_set_get_timestamp,
            result_set_get_timestamp_by_label,
            result_set_get_timestamp_with_calendar,
            result_set_get_timestamp_by_label_with_calendar,
            NaiveDateTime,
            "getTimestamp"
        ),
    );

    resource_result_set_filter_chain_methods!(
        (
            result_set_get_ref,
            result_set_get_ref_by_label,
            JdbcRef,
            "getRef"
        ),
        (
            result_set_get_blob,
            result_set_get_blob_by_label,
            JdbcBlob,
            "getBlob"
        ),
        (
            result_set_get_clob,
            result_set_get_clob_by_label,
            JdbcClob,
            "getClob"
        ),
        (
            result_set_get_array,
            result_set_get_array_by_label,
            JdbcArray,
            "getArray"
        ),
        (
            result_set_get_url,
            result_set_get_url_by_label,
            JdbcUrl,
            "getURL"
        ),
        (
            result_set_get_row_id,
            result_set_get_row_id_by_label,
            JdbcRowId,
            "getRowId"
        ),
        (
            result_set_get_n_clob,
            result_set_get_n_clob_by_label,
            JdbcNClob,
            "getNClob"
        ),
        (
            result_set_get_sql_xml,
            result_set_get_sql_xml_by_label,
            JdbcSqlXml,
            "getSQLXML"
        ),
        (
            result_set_get_ascii_stream,
            result_set_get_ascii_stream_by_label,
            JdbcInputStream,
            "getAsciiStream"
        ),
        (
            result_set_get_unicode_stream,
            result_set_get_unicode_stream_by_label,
            JdbcInputStream,
            "getUnicodeStream"
        ),
        (
            result_set_get_binary_stream,
            result_set_get_binary_stream_by_label,
            JdbcInputStream,
            "getBinaryStream"
        ),
        (
            result_set_get_character_stream,
            result_set_get_character_stream_by_label,
            JdbcReader,
            "getCharacterStream"
        ),
        (
            result_set_get_n_character_stream,
            result_set_get_n_character_stream_by_label,
            JdbcReader,
            "getNCharacterStream"
        ),
    );

    pub async fn before_execute(&self, ctx: &mut ExecContext<'_>) -> Result<(), DruidError> {
        for f in &self.before {
            f.before(ctx).await?;
        }
        Ok(())
    }

    /// 按 Java Filter 注册顺序执行一次 batch 前置链。
    pub async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.before_batch(context).await?;
        }
        Ok(())
    }

    /// 按注册顺序执行连接事件前置过滤器。
    pub async fn before_connection_event(&self, event: &ConnectionEvent) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.on_connection_event(event).await?;
        }
        Ok(())
    }

    /// 从位置 0 执行一次 `Connection#getWarnings()` around-chain。
    pub async fn connection_warnings(
        &self,
        physical: &mut dyn PhysicalConnection,
    ) -> Result<Option<SqlWarning>, DruidError> {
        ConnectionWarningFilterChain::new(&self.before, physical)
            .connection_get_warnings()
            .await
    }

    /// 从位置 0 执行一次 `Connection#clearWarnings()` around-chain。
    pub async fn connection_clear_warnings(
        &self,
        physical: &mut dyn PhysicalConnection,
    ) -> Result<(), DruidError> {
        ConnectionWarningFilterChain::new(&self.before, physical)
            .connection_clear_warnings()
            .await
    }

    /// 从位置 0 执行一次 `Statement#getWarnings()` around-chain。
    pub async fn statement_warnings(
        &self,
        physical: &dyn PhysicalStatement,
    ) -> Result<Option<SqlWarning>, DruidError> {
        StatementWarningFilterChain::new_statement(&self.before, physical)
            .statement_get_warnings()
            .await
    }

    /// 从位置 0 执行一次 `Statement#clearWarnings()` around-chain。
    pub async fn statement_clear_warnings(
        &self,
        physical: &dyn PhysicalStatement,
    ) -> Result<(), DruidError> {
        StatementWarningFilterChain::new_statement(&self.before, physical)
            .statement_clear_warnings()
            .await
    }

    /// 从位置 0 执行 PreparedStatement 的 `Statement#getWarnings()` around-chain。
    pub async fn prepared_statement_warnings(
        &self,
        physical: &dyn PhysicalPreparedStatement,
    ) -> Result<Option<SqlWarning>, DruidError> {
        StatementWarningFilterChain::new_prepared_statement(&self.before, physical)
            .statement_get_warnings()
            .await
    }

    /// 从位置 0 执行 PreparedStatement 的 `Statement#clearWarnings()` around-chain。
    pub async fn prepared_statement_clear_warnings(
        &self,
        physical: &dyn PhysicalPreparedStatement,
    ) -> Result<(), DruidError> {
        StatementWarningFilterChain::new_prepared_statement(&self.before, physical)
            .statement_clear_warnings()
            .await
    }

    pub async fn after_execute(
        &self,
        ctx: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        for f in self.after.iter().rev() {
            f.after(ctx, result, elapsed).await?;
        }
        Ok(())
    }

    /// 按调用栈逆序执行一次 batch 后置链。
    pub async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        for filter in self.after.iter().rev() {
            filter.after_batch(context, result, elapsed).await?;
        }
        Ok(())
    }

    /// 按调用栈逆序执行物理连接事件成功后的 Filter。
    pub async fn after_connection_event(
        &self,
        event: &ConnectionEvent,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        for filter in self.after.iter().rev() {
            filter.after_connection_event(event, elapsed).await?;
        }
        Ok(())
    }

    /// 按逆序执行连接关闭后置过滤器。
    pub async fn after_connection_close(&self) -> Result<(), DruidError> {
        for filter in self.after.iter().rev() {
            filter.after_connection_close().await?;
        }
        Ok(())
    }
}

impl Default for FilterChain {
    fn default() -> Self {
        Self::new()
    }
}
