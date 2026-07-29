//! RBDC 物理预编译语句。

use crate::prepared_parameter_state::PreparedParameterState;
use druid::core::{
    DruidError, PhysicalPreparedStatement, PhysicalStatementOptions, PreparedInputParameter,
    SqlTextPreparedStatement, SqlWarning, Value,
};
use std::any::Any;

/// 为 RBDC `Connection::exec` 保存物理参数槽的 Prepared 句柄。
///
/// RBDC 公开 SPI 没有独立 prepared handle，因此 SQL token 仍由 connection 执行；
/// 本对象负责保持 Java setter 时点、参数槽、batch 快照和继承 Statement 状态。
pub struct RbdcPreparedStatement {
    inner: SqlTextPreparedStatement,
    parameter_state: PreparedParameterState,
}

impl RbdcPreparedStatement {
    /// 创建 RBDC Prepared SQL token。
    pub(crate) fn new(sql: impl Into<String>) -> Self {
        Self {
            inner: SqlTextPreparedStatement::new(sql),
            parameter_state: PreparedParameterState::new(),
        }
    }

    /// 返回当前连续参数槽。
    pub(crate) fn materialized_parameters(
        &self,
        parameter_count: usize,
    ) -> Result<Vec<Value>, DruidError> {
        self.parameter_state.values(parameter_count)
    }

    /// 消费物理 batch 快照。
    pub(crate) fn take_batches(
        &self,
        expected_count: usize,
    ) -> Result<Option<Vec<Vec<Value>>>, DruidError> {
        self.parameter_state.take_batches(expected_count)
    }
}

impl PhysicalPreparedStatement for RbdcPreparedStatement {
    fn sql(&self) -> &str {
        self.inner.sql()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn statement_options(&self) -> PhysicalStatementOptions {
        self.inner.statement_options()
    }

    fn max_field_size(&self) -> Result<i32, DruidError> {
        self.inner.max_field_size()
    }

    fn set_max_field_size(&self, max: i32) -> Result<(), DruidError> {
        self.inner.set_max_field_size(max)
    }

    fn max_rows(&self) -> Result<i32, DruidError> {
        self.inner.max_rows()
    }

    fn set_max_rows(&self, max: i32) -> Result<(), DruidError> {
        self.inner.set_max_rows(max)
    }

    fn set_escape_processing(&self, enabled: bool) -> Result<(), DruidError> {
        self.inner.set_escape_processing(enabled)
    }

    fn query_timeout(&self) -> Result<i32, DruidError> {
        self.inner.query_timeout()
    }

    fn set_query_timeout(&self, seconds: i32) -> Result<(), DruidError> {
        self.inner.set_query_timeout(seconds)
    }

    fn cancel(&self) -> Result<(), DruidError> {
        self.inner.cancel()
    }

    fn set_cursor_name(&self, name: &str) -> Result<(), DruidError> {
        self.inner.set_cursor_name(name)
    }

    fn set_fetch_direction(&self, direction: i32) -> Result<(), DruidError> {
        self.inner.set_fetch_direction(direction)
    }

    fn fetch_direction(&self) -> Result<i32, DruidError> {
        self.inner.fetch_direction()
    }

    fn set_fetch_size(&self, rows: i32) -> Result<(), DruidError> {
        self.inner.set_fetch_size(rows)
    }

    fn fetch_size(&self) -> Result<i32, DruidError> {
        self.inner.fetch_size()
    }

    fn close_on_completion(&self) -> Result<(), DruidError> {
        self.inner.close_on_completion()
    }

    fn is_close_on_completion(&self) -> Result<bool, DruidError> {
        self.inner.is_close_on_completion()
    }

    fn set_parameter(
        &self,
        parameter_index: usize,
        parameter: &PreparedInputParameter,
    ) -> Result<(), DruidError> {
        self.parameter_state.set(parameter_index, parameter)
    }

    fn clear_parameters(&self) -> Result<(), DruidError> {
        self.parameter_state.clear_parameters();
        Ok(())
    }

    fn add_batch(&self, params: &[Value]) -> Result<(), DruidError> {
        self.parameter_state.add_values(params);
        Ok(())
    }

    fn add_parameter_batch(&self, params: &[PreparedInputParameter]) -> Result<(), DruidError> {
        self.parameter_state.add_parameters(params)
    }

    fn clear_batch(&self) -> Result<(), DruidError> {
        self.parameter_state.clear_batches();
        Ok(())
    }

    fn warnings(&self) -> Result<Option<SqlWarning>, DruidError> {
        self.inner.warnings()
    }

    fn clear_warnings(&self) -> Result<(), DruidError> {
        self.inner.clear_warnings()
    }

    fn close(&self) -> Result<(), DruidError> {
        self.inner.close()
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use super::RbdcPreparedStatement;
    use druid::core::{PhysicalPreparedStatement, PreparedInputParameter, Value};

    #[test]
    fn delegates_statement_state_and_owns_parameter_lifecycle() {
        let statement = RbdcPreparedStatement::new("select ?");
        assert_eq!(statement.sql(), "select ?");
        assert!(statement.as_any().is::<RbdcPreparedStatement>());
        assert_eq!(statement.statement_options(), Default::default());

        statement.set_max_field_size(11).unwrap();
        statement.set_max_rows(12).unwrap();
        statement.set_escape_processing(false).unwrap();
        statement.set_query_timeout(13).unwrap();
        statement.set_cursor_name("cursor").unwrap();
        statement.set_fetch_direction(1).unwrap();
        statement.set_fetch_size(14).unwrap();
        statement.close_on_completion().unwrap();
        assert_eq!(statement.max_field_size().unwrap(), 11);
        assert_eq!(statement.max_rows().unwrap(), 12);
        assert_eq!(statement.query_timeout().unwrap(), 13);
        assert_eq!(statement.fetch_direction().unwrap(), 1);
        assert_eq!(statement.fetch_size().unwrap(), 14);
        assert!(statement.is_close_on_completion().unwrap());
        assert_eq!(statement.warnings().unwrap(), None);
        statement.clear_warnings().unwrap();
        statement.cancel().unwrap();

        statement
            .set_parameter(1, &PreparedInputParameter::Int(7))
            .unwrap();
        assert_eq!(
            statement.materialized_parameters(1).unwrap(),
            vec![Value::Int(7)]
        );
        statement
            .add_parameter_batch(&[PreparedInputParameter::Int(7)])
            .unwrap();
        assert_eq!(
            statement.take_batches(1).unwrap(),
            Some(vec![vec![Value::Int(7)]])
        );
        statement.add_batch(&[Value::Int(8)]).unwrap();
        statement.clear_batch().unwrap();
        assert_eq!(statement.take_batches(0).unwrap(), None);
        statement.clear_parameters().unwrap();
        assert!(statement.materialized_parameters(1).is_err());

        statement.close().unwrap();
        assert!(statement.is_closed());
    }
}
