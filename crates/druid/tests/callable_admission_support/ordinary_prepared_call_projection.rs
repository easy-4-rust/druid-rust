use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::metadata::{ParameterMetaData, ResultSetMetaData};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    CurrentResultDisposition, ExecutionResult, FetchDirection, GeneratedKeys, ParameterIndex,
    PreparedInputParameter, PreparedParameter, PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::Arc;

/// 普通 Prepared 后端的静态 Callable 返回容器，仅用于原错误准入合同控制。
/// 对应 Java: DruidPooledConnection#prepareCall 的非 Callable 句柄拒绝路径。
/// 本对象不声明动态 Callable 角色；所有普通句柄行为委托 inner，OUT 明确拒绝。
pub(super) struct OrdinaryPreparedCallProjection {
    inner: Box<dyn PreparedStatement>,
}
impl OrdinaryPreparedCallProjection {
    /// 接管实际创建的普通 Prepared 句柄；返回持有同一后端的标准投影。
    pub(super) fn new(inner: Box<dyn PreparedStatement>) -> Self {
        Self { inner }
    }
}
druid_rdbc::impl_standard_wrapper!(OrdinaryPreparedCallProjection => PreparedStatement);

#[async_trait::async_trait]
impl Statement for OrdinaryPreparedCallProjection {
    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn options(&self) -> Result<StatementOptions, RdbcError> {
        self.inner.options()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn max_field_size(&self) -> Result<i32, RdbcError> {
        self.inner.max_field_size()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_max_field_size(&mut self, _max: i32) -> Result<(), RdbcError> {
        self.inner.set_max_field_size(_max)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn max_rows(&self) -> Result<i64, RdbcError> {
        self.inner.max_rows()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_max_rows(&mut self, _max: i64) -> Result<(), RdbcError> {
        self.inner.set_max_rows(_max)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_escape_processing(&mut self, _enabled: bool) -> Result<(), RdbcError> {
        self.inner.set_escape_processing(_enabled)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.inner.query_timeout()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_query_timeout(&mut self, _seconds: i32) -> Result<(), RdbcError> {
        self.inner.set_query_timeout(_seconds)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn cancel(&mut self) -> Result<(), RdbcError> {
        self.inner.cancel()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_cursor_name(&mut self, _name: &str) -> Result<(), RdbcError> {
        self.inner.set_cursor_name(_name)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_fetch_direction(&mut self, _direction: FetchDirection) -> Result<(), RdbcError> {
        self.inner.set_fetch_direction(_direction)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.inner.fetch_direction()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_fetch_size(&mut self, _rows: i32) -> Result<(), RdbcError> {
        self.inner.set_fetch_size(_rows)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.inner.fetch_size()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.execute_query(_sql).await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute_update(
        &mut self,
        _sql: &str,
        _generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.inner.execute_update(_sql, _generated_keys).await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute(
        &mut self,
        _sql: &str,
        _generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.inner.execute(_sql, _generated_keys).await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn add_batch(&mut self, _sql: String) -> Result<(), RdbcError> {
        self.inner.add_batch(_sql)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.inner.clear_batch()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn batch(&self) -> Result<Vec<String>, RdbcError> {
        self.inner.batch()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.inner.execute_batch().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.generated_keys().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn get_generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.get_generated_keys().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        self.inner.get_result_set().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.inner.get_update_count()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn get_more_results(
        &mut self,
        _disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.inner.get_more_results(_disposition).await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn get_more_results_current(&mut self, current: i32) -> Result<bool, RdbcError> {
        self.inner.get_more_results_current(current).await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.inner.close().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.inner.is_closed()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn is_poolable(&self) -> Result<bool, RdbcError> {
        self.inner.is_poolable()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_poolable(&mut self, _poolable: bool) -> Result<(), RdbcError> {
        self.inner.set_poolable(_poolable)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn close_on_completion(&mut self) -> Result<(), RdbcError> {
        self.inner.close_on_completion()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        self.inner.is_close_on_completion()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.inner.warnings()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn warning_messages(&self) -> Result<Vec<String>, RdbcError> {
        self.inner.warning_messages()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.inner.clear_warnings()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute_values(
        &mut self,
        sql: &str,
        _values: Vec<Value>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.execute_values(sql, _values).await
    }
}

#[async_trait::async_trait]
impl PreparedStatement for OrdinaryPreparedCallProjection {
    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn sql(&self) -> Result<&str, RdbcError> {
        self.inner.sql()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_parameter(&mut self, _parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.inner.set_parameter(_parameter)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.inner.set_input_parameter(index, parameter)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.inner.clear_parameters()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn add_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.inner.add_prepared_batch()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn add_parameter_batch(
        &mut self,
        _parameters: Vec<PreparedParameter>,
    ) -> Result<(), RdbcError> {
        self.inner.add_parameter_batch(_parameters)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn add_input_parameter_batch(
        &mut self,
        parameters: Vec<(ParameterIndex, PreparedInputParameter)>,
    ) -> Result<(), RdbcError> {
        self.inner.add_input_parameter_batch(parameters)
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.inner.clear_prepared_batch()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.execute_prepared_query().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.inner.execute_prepared_update().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.inner.execute_prepared().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.inner.execute_prepared_batch().await
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn result_set_meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.inner.result_set_meta_data()
    }

    /// 原样委托真实普通 Prepared 后端，保留结果及错误。
    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        self.inner.parameter_meta_data()
    }
}
#[async_trait::async_trait]
impl CallableStatement for OrdinaryPreparedCallProjection {
    fn register_out_parameter(
        &mut self,
        _parameter: CallableParameter,
        _out: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "ordinary_prepared_register_out",
        })
    }
    fn set_named_parameter(
        &mut self,
        _name: &str,
        _parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "ordinary_prepared_named_parameter",
        })
    }
    fn out_parameter(&self, _parameter: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "ordinary_prepared_out_parameter",
        })
    }
}
