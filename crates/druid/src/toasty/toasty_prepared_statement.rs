//! Toasty 物理预编译语句。
//!
//! 对应 Java：`java.sql.PreparedStatement` 的驱动句柄职责。

use super::toasty_connection_adapter::ToastyConnectionAdapter;
use crate::core::{
    DruidError, PhysicalPreparedStatement, PhysicalStatementOptions, PreparedInputParameter,
    SqlTextPreparedStatement, SqlWarning, Value,
};
use std::any::Any;
use std::sync::Mutex;

/// 在 RDBC setter 边界物化参数的 Toasty 预编译语句句柄。
///
/// Toasty 的 raw SQL API 没有独立 PreparedStatement 参数槽，因此本对象在
/// `setXxx` 时执行驱动转换并保存值，使负长度、提前 EOF 等错误与 Java Druid
/// 一样由 setter 返回，而不是延迟到 execute。池化层仍只保存原始描述符。
pub struct ToastyPreparedStatement {
    inner: SqlTextPreparedStatement,
    parameters: Mutex<Vec<Option<Value>>>,
    batches: Mutex<Vec<Vec<Value>>>,
}

impl ToastyPreparedStatement {
    /// 创建 Toasty 物理预编译语句。
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            inner: SqlTextPreparedStatement::new(sql),
            parameters: Mutex::new(Vec::new()),
            batches: Mutex::new(Vec::new()),
        }
    }

    /// 返回当前已物化参数；参数槽不完整时拒绝执行。
    pub(super) fn materialized_parameters(
        &self,
        parameter_count: usize,
    ) -> Result<Vec<Value>, DruidError> {
        let parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (0..parameter_count)
            .map(|index| {
                parameters.get(index).and_then(Clone::clone).ok_or_else(|| {
                    DruidError::InvalidArgument(format!("parameter {} has not been set", index + 1))
                })
            })
            .collect()
    }

    /// 消费物理句柄中已由 `addBatch` 保存的有序参数批次。
    pub(super) fn take_batches(
        &self,
        expected_count: usize,
    ) -> Result<Option<Vec<Vec<Value>>>, DruidError> {
        let mut batches = self
            .batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if batches.is_empty() {
            return Ok(None);
        }
        if batches.len() != expected_count {
            return Err(DruidError::InvalidArgument(format!(
                "physical prepared batch count {}, wrapper batch count {expected_count}",
                batches.len()
            )));
        }
        Ok(Some(std::mem::take(&mut *batches)))
    }
}

impl PhysicalPreparedStatement for ToastyPreparedStatement {
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
        if parameter_index == 0 {
            return Err(DruidError::InvalidArgument(
                "parameterIndex must be at least 1".to_string(),
            ));
        }
        let value = ToastyConnectionAdapter::prepared_parameter(parameter)?;
        let mut parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if parameters.len() < parameter_index {
            parameters.resize(parameter_index, None);
        }
        parameters[parameter_index - 1] = Some(value);
        Ok(())
    }

    fn clear_parameters(&self) -> Result<(), DruidError> {
        self.parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }

    fn add_batch(&self, params: &[Value]) -> Result<(), DruidError> {
        self.batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(params.to_vec());
        Ok(())
    }

    fn add_parameter_batch(&self, params: &[PreparedInputParameter]) -> Result<(), DruidError> {
        let values = self.materialized_parameters(params.len())?;
        self.batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(values);
        Ok(())
    }

    fn clear_batch(&self) -> Result<(), DruidError> {
        self.batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
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
    use super::*;

    #[test]
    fn physical_parameter_slots_and_batches_reject_inconsistent_state() {
        let statement = ToastyPreparedStatement::new("SELECT ?1");
        assert_eq!(
            statement.statement_options(),
            PhysicalStatementOptions::default()
        );
        assert!(matches!(
            statement.materialized_parameters(1),
            Err(DruidError::InvalidArgument(_))
        ));
        assert!(matches!(
            statement.set_parameter(0, &PreparedInputParameter::Int(1)),
            Err(DruidError::InvalidArgument(_))
        ));

        statement
            .set_parameter(1, &PreparedInputParameter::Int(7))
            .unwrap();
        assert_eq!(
            statement.materialized_parameters(1).unwrap(),
            vec![Value::Int(7)]
        );
        statement.add_batch(&[Value::Int(7)]).unwrap();
        assert!(matches!(
            statement.take_batches(2),
            Err(DruidError::InvalidArgument(_))
        ));
        assert_eq!(
            statement.take_batches(1).unwrap(),
            Some(vec![vec![Value::Int(7)]])
        );
        assert_eq!(statement.take_batches(0).unwrap(), None);

        statement.clear_parameters().unwrap();
        assert!(matches!(
            statement.add_parameter_batch(&[PreparedInputParameter::Int(7)]),
            Err(DruidError::InvalidArgument(_))
        ));
    }
}
