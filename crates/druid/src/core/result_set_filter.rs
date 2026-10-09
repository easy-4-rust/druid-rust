//! Canonical ResultSet filter contract.

use super::standard_result_set_filter_chain::StandardResultSetFilterChain;
use super::standard_result_set_open_context::StandardResultSetOpenContext;
use super::{DruidError, ResultSetFilterContext, ResultSetObjectRead, Value};
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead, ResultSetUpdate};
use druid_rdbc::types::RdbcObject;
use std::sync::Arc;

/// Around-filter contract for the single canonical ResultSet surface.
#[allow(private_interfaces)]
#[async_trait::async_trait]
pub trait ResultSetFilter: Send + Sync {
    /// 包围原行操作；参数为上下文、独立操作身份与惰性后续调用，返回原结果。
    /// 默认继续下一Filter。对应 Java: Filter#resultSet_insertRow/updateRow/deleteRow/refreshRow/cancelRowUpdates/moveToInsertRow/moveToCurrentRow。
    fn standard_result_set_row_operation(
        &self,
        _context: &ResultSetFilterContext,
        _operation: super::ResultSetRowOperation,
        next: &mut dyn FnMut() -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        next()
    }
    /// 包围资源对象读取；参数为上下文、治理列、原标签、请求及后续调用。
    /// 返回完整资源句柄或 SQL NULL，不扁平化、不消费流。对应 Java: Filter#resultSet_getRef/getBlob/getClob/getArray/getURL/getRowId/getNClob/getSQLXML。
    fn standard_result_set_resource(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<RdbcObject, DruidError>,
    ) -> Result<RdbcObject, DruidError> {
        next()
    }

    /// 包围字节流读取；参数保留下标/标签和流类型，返回同一可空流或错误。
    /// 默认惰性委托，不预读资源。对应 Java: Filter#resultSet_getAsciiStream/getUnicodeStream/getBinaryStream。
    fn standard_result_set_input_stream(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _kind: druid_rdbc::result_set::ResultSetStreamKind,
        next: &mut dyn FnMut() -> Result<Option<druid_rdbc::resource::RdbcInputStream>, DruidError>,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, DruidError> {
        next()
    }

    /// 包围字符流读取；参数保留下标/标签及国家字符标志，返回同一Reader或错误。
    /// 默认不定位标签或读取游标。对应 Java: Filter#resultSet_getCharacterStream/getNCharacterStream。
    fn standard_result_set_reader(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _national: bool,
        next: &mut dyn FnMut() -> Result<Option<druid_rdbc::resource::RdbcReader>, DruidError>,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, DruidError> {
        next()
    }

    /// 包围原导航请求；参数为上下文、完整导航和惰性后续调用，返回原状态或错误。
    /// 对应 Java: Filter 的 ResultSet 导航方法；短路时后续 future 不被轮询。
    async fn standard_result_set_navigation<'a>(
        &'a self,
        _context: &'a ResultSetFilterContext,
        _navigation: druid_rdbc::result_set::ResultSetNavigation,
        next: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<bool, DruidError>> + Send + 'a>,
        >,
    ) -> Result<bool, DruidError> {
        next.await
    }

    /// 包围原属性调用；参数为上下文、完整描述符及后续调用，返回同类型属性值。
    /// 对应 Java: Filter 的 ResultSet 属性与 findColumn 方法。
    fn standard_result_set_property(
        &self,
        _context: &ResultSetFilterContext,
        _request: super::ResultSetPropertyOperation<'_>,
        next: &mut dyn FnMut() -> Result<super::ResultSetPropertyValue, DruidError>,
    ) -> Result<super::ResultSetPropertyValue, DruidError> {
        next()
    }
    /// 包围完整元数据句柄读取，允许外部实现者短路且不公开私有链或原生所有权。
    /// 参数为上下文和后续调用；返回同一平台句柄或精确错误。
    /// 对应 Java: Filter#resultSet_getMetaData；默认保留既有before/after观察。
    fn standard_result_set_metadata(
        &self,
        context: &ResultSetFilterContext,
        next: &mut dyn FnMut() -> Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<Arc<dyn ResultSetMetaData>, DruidError> {
        self.standard_result_set_meta_data_before(context)?;
        let mut result = next();
        self.standard_result_set_meta_data_after(context, &mut result)?;
        result
    }

    /// 包围逻辑创建语句查询，可默认委托或返回拥有型共享替换句柄。
    /// 参数为上下文与后续调用；返回可空标准语句或精确错误，不公开原生连接。
    /// 对应 Java: Filter#resultSet_getStatement。
    fn standard_result_set_statement(
        &self,
        _context: &ResultSetFilterContext,
        next: &mut dyn FnMut() -> Result<
            Option<Box<dyn druid_rdbc::statement::Statement>>,
            DruidError,
        >,
    ) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>, DruidError> {
        next()
    }

    /// 包围六种完整对象读取重载，保留可空映射、目标类型与返回句柄身份。
    /// 参数为上下文、已治理列、原列名、原请求及后续操作；返回完整对象或精确错误。
    /// 对应 Java: Filter#resultSet_getObject(int/String, Map/Class)。
    /// 默认仅委托后续操作，不定位原列名、不消费资源、不公开驱动所有权。
    fn standard_result_set_object(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetObjectRead<'_>,
        next: &mut dyn FnMut() -> Result<RdbcObject, DruidError>,
    ) -> Result<RdbcObject, DruidError> {
        next()
    }

    /// 包围完整标量读取重载，保留原列名、scale 与 Calendar 参数身份。
    /// 参数含上下文、可空已治理列、原列名、读取描述符和后续操作；返回值或原错误。
    /// 对应 Java: Filter#resultSet_getBigDecimal/getDate/getTime/getTimestamp。
    /// 列名未配置治理时允许在驱动定位前短路；默认调用后续操作，不公开驱动所有权。
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        next()
    }

    /// Observes a successfully registered result set.
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        Ok(())
    }

    /// Observes and may configure the canonical result-set proxy after registration.
    fn standard_result_set_open_after_with_proxy(
        &self,
        context: &mut StandardResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        self.result_set_open_after(context.filter_context())
    }

    /// Runs before `ResultSet::next`.
    fn standard_result_set_next_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::next`.
    fn standard_result_set_next_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &Result<bool, DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs before `ResultSet::close`.
    fn standard_result_set_close_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::close`.
    fn standard_result_set_close_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs before `ResultSet::value`.
    fn standard_result_set_value_before(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::value` and may replace the result.
    fn standard_result_set_value_after(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
        _result: &mut Result<Value, DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs before `ResultSet::meta_data`.
    fn standard_result_set_meta_data_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::meta_data` and may replace the result.
    fn standard_result_set_meta_data_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &mut Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs before `ResultSet::was_null`.
    fn standard_result_set_was_null_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::was_null` and may replace the result.
    fn standard_result_set_was_null_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &mut Result<bool, DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Around hook for canonical `ResultSet::next`.
    async fn standard_result_set_next(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<bool, DruidError> {
        self.standard_result_set_next_before(chain.context())?;
        let result = chain.result_set_next().await;
        self.standard_result_set_next_after(chain.context(), &result)?;
        result
    }

    /// Around hook for canonical `ResultSet::close`.
    async fn standard_result_set_close(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.standard_result_set_close_before(chain.context())?;
        let result = chain.result_set_close().await;
        self.standard_result_set_close_after(chain.context(), &result)?;
        result
    }

    /// Around hook for canonical `ResultSet::value`.
    fn standard_result_set_value(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
    ) -> Result<Value, DruidError> {
        self.standard_result_set_value_before(chain.context(), column)?;
        let mut result = chain.result_set_value(column);
        self.standard_result_set_value_after(chain.context(), column, &mut result)?;
        result
    }

    /// 包围按下标更新；参数为链、1-based列和原更新描述符，返回驱动或Filter结果。
    /// 对应 Java: Filter#resultSet_updateXxx(int, ...)；默认继续下一Filter。
    fn standard_result_set_update(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        chain.result_set_update(column, update)
    }

    /// 包围按原列名更新；参数为链、列名和完整描述符，返回驱动或Filter结果。
    /// 对应 Java: Filter#resultSet_updateXxx(String, ...)；默认保留列名继续链。
    fn standard_result_set_update_by_label(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        chain.result_set_update_by_label(label, update)
    }

    /// Around hook for canonical `ResultSet::meta_data`.
    fn standard_result_set_meta_data(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<Arc<dyn ResultSetMetaData>, DruidError> {
        let context = chain.context();
        self.standard_result_set_metadata(context, &mut || chain.result_set_meta_data())
    }

    /// Around hook for canonical `ResultSet::was_null`.
    fn standard_result_set_was_null(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<bool, DruidError> {
        self.standard_result_set_was_null_before(chain.context())?;
        let mut result = chain.result_set_was_null();
        self.standard_result_set_was_null_after(chain.context(), &mut result)?;
        result
    }
}
