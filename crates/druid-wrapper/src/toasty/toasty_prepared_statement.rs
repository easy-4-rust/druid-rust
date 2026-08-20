//! Toasty 物理预编译语句。
//!
//! 对应 Java：`java.sql.PreparedStatement` 的驱动句柄职责。

use super::toasty_connection_adapter::ToastyConnectionAdapter;
use druid_core::core::{
    DruidError, PhysicalPreparedStatement, PhysicalStatementOptions, PreparedInputParameter,
    SqlTextPreparedStatement, SqlWarning, Value,
};
use std::any::Any;
use std::sync::Mutex;

#[derive(Clone)]
enum ToastyStoredParameter {
    Materialized(Value),
    Deferred(PreparedInputParameter),
}

impl ToastyStoredParameter {
    async fn materialize(self) -> Result<Value, DruidError> {
        match self {
            Self::Materialized(value) => Ok(value),
            Self::Deferred(parameter) => {
                ToastyConnectionAdapter::prepared_parameter(&parameter).await
            }
        }
    }
}

enum ToastyStoredBatch {
    Parameters(Vec<ToastyStoredParameter>),
    Values(Vec<Value>),
}

/// 在 RDBC setter 边界保存已物化值和异步资源描述符的 Toasty 预编译语句句柄。
///
/// Toasty 的 raw SQL API 没有独立 `PreparedStatement` 参数槽，因此本对象在
/// `setXxx` 保留本地流和标量的既有时点，仅异步 RDBC 资源在 execute future 中转换。
pub struct ToastyPreparedStatement {
    inner: SqlTextPreparedStatement,
    parameters: Mutex<Vec<Option<ToastyStoredParameter>>>,
    batches: Mutex<Vec<ToastyStoredBatch>>,
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
    pub(super) async fn materialized_parameters(
        &self,
        parameter_count: usize,
    ) -> Result<Vec<Value>, DruidError> {
        let parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let parameters = (0..parameter_count)
            .map(|index| {
                parameters.get(index).and_then(Clone::clone).ok_or_else(|| {
                    DruidError::InvalidArgument(format!("parameter {} has not been set", index + 1))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut values = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            values.push(parameter.materialize().await?);
        }
        Ok(values)
    }

    /// 消费物理句柄中已由 `addBatch` 保存的有序参数批次。
    pub(super) async fn take_batches(
        &self,
        expected_count: usize,
    ) -> Result<Option<Vec<Vec<Value>>>, DruidError> {
        let batches = {
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
                    batches.len(),
                )));
            }
            std::mem::take(&mut *batches)
        };
        let mut values = Vec::with_capacity(batches.len());
        for batch in batches {
            match batch {
                ToastyStoredBatch::Values(batch) => values.push(batch),
                ToastyStoredBatch::Parameters(batch) => {
                    let mut materialized = Vec::with_capacity(batch.len());
                    for parameter in batch {
                        materialized.push(parameter.materialize().await?);
                    }
                    values.push(materialized);
                }
            }
        }
        Ok(Some(values))
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
        let parameter = ToastyConnectionAdapter::prepared_parameter_immediate(parameter)?
            .map_or_else(
                || ToastyStoredParameter::Deferred(parameter.clone()),
                ToastyStoredParameter::Materialized,
            );
        let mut parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if parameters.len() < parameter_index {
            parameters.resize(parameter_index, None);
        }
        parameters[parameter_index - 1] = Some(parameter);
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
            .push(ToastyStoredBatch::Values(params.to_vec()));
        Ok(())
    }

    fn add_parameter_batch(&self, params: &[PreparedInputParameter]) -> Result<(), DruidError> {
        let parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let snapshot = (0..params.len())
            .map(|index| {
                parameters.get(index).and_then(Clone::clone).ok_or_else(|| {
                    DruidError::InvalidArgument(format!("parameter {} has not been set", index + 1))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(ToastyStoredBatch::Parameters(snapshot));
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

    #[tokio::test]
    async fn physical_parameter_slots_and_batches_reject_inconsistent_state() {
        let statement = ToastyPreparedStatement::new("SELECT ?1");
        assert_eq!(
            statement.statement_options(),
            PhysicalStatementOptions::default()
        );
        assert!(matches!(
            statement.materialized_parameters(1).await,
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
            statement.materialized_parameters(1).await.unwrap(),
            vec![Value::Int(7)]
        );
        statement.add_batch(&[Value::Int(7)]).unwrap();
        assert!(matches!(
            statement.take_batches(2).await,
            Err(DruidError::InvalidArgument(_))
        ));
        assert_eq!(
            statement.take_batches(1).await.unwrap(),
            Some(vec![vec![Value::Int(7)]])
        );
        assert_eq!(statement.take_batches(0).await.unwrap(), None);

        statement.clear_parameters().unwrap();
        assert!(matches!(
            statement.add_parameter_batch(&[PreparedInputParameter::Int(7)]),
            Err(DruidError::InvalidArgument(_))
        ));
    }
}
