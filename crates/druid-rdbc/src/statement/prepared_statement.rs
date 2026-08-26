use super::prepared_input_parameter::PreparedInputParameter;
use crate::error::RdbcError;
use crate::metadata::{ParameterMetaData, ResultSetMetaData};
use crate::result_set::ResultSet;
use crate::statement::{ExecutionResult, Statement};
use crate::types::Value;
use std::sync::Arc;

/// JDBC 1-based parameter index。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParameterIndex(usize);

impl ParameterIndex {
    /// 创建 1-based parameter index。
    pub fn new(index: usize) -> Result<Self, RdbcError> {
        if index == 0 {
            return Err(RdbcError::InvalidArgument(
                "parameter index is 1-based".to_string(),
            ));
        }
        Ok(Self(index))
    }

    /// 返回原始 1-based 数值。
    pub const fn get(self) -> usize {
        self.0
    }
}

impl TryFrom<i64> for ParameterIndex {
    type Error = RdbcError;

    fn try_from(index: i64) -> Result<Self, Self::Error> {
        Self::try_from(i128::from(index))
    }
}

impl TryFrom<i128> for ParameterIndex {
    type Error = RdbcError;

    fn try_from(index: i128) -> Result<Self, Self::Error> {
        if index <= 0 {
            return Err(RdbcError::InvalidArgument(
                "parameter index is 1-based".to_string(),
            ));
        }
        let index = usize::try_from(index).map_err(|_| {
            RdbcError::InvalidArgument("parameter index exceeds platform usize".to_string())
        })?;
        Self::new(index)
    }
}

/// 一个 prepared input parameter snapshot。
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedParameter {
    /// 1-based parameter index。
    pub index: ParameterIndex,
    /// SQL value；[`Value::Null`] 是唯一 SQL NULL 表示。
    pub value: Value,
}

/// JDBC `PreparedStatement` 标准。
#[async_trait::async_trait]
pub trait PreparedStatement: Statement {
    /// 返回 prepared SQL text。
    fn sql(&self) -> Result<&str, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_sql",
        })
    }

    /// 设置一个 typed parameter snapshot。
    fn set_parameter(&mut self, _parameter: PreparedParameter) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_set_parameter",
        })
    }

    /// 按 1-based index 绑定 rich JDBC setter descriptor。
    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.set_parameter(PreparedParameter {
            index,
            value: parameter.scalar_value()?,
        })
    }

    /// 清空当前 parameter snapshots。
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    /// 把当前 parameter snapshot 加入 prepared batch。
    fn add_prepared_batch(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_add_batch",
        })
    }

    /// 直接加入一组 typed parameter snapshots。
    fn add_parameter_batch(
        &mut self,
        _parameters: Vec<PreparedParameter>,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_add_parameter_batch",
        })
    }

    /// 直接加入一组 1-based rich setter descriptors。
    fn add_input_parameter_batch(
        &mut self,
        parameters: Vec<(ParameterIndex, PreparedInputParameter)>,
    ) -> Result<(), RdbcError> {
        let parameters = parameters
            .into_iter()
            .map(|(index, parameter)| {
                Ok(PreparedParameter {
                    index,
                    value: parameter.scalar_value()?,
                })
            })
            .collect::<Result<Vec<_>, RdbcError>>()?;
        self.add_parameter_batch(parameters)
    }

    /// 清空 prepared batch，不改变当前 parameter snapshot。
    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    /// 执行 prepared query 并返回 owned result set。
    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_execute_query",
        })
    }

    /// 执行 prepared update。
    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_execute_update",
        })
    }

    /// 执行 generic prepared statement。
    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_execute",
        })
    }

    /// 执行 prepared parameter batch，并返回无损 64-bit counts。
    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_execute_batch",
        })
    }

    /// 返回 prepared query 的 immutable ResultSet metadata snapshot。
    fn result_set_meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_result_set_meta_data",
        })
    }

    /// 返回 immutable parameter metadata snapshot。
    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_parameter_meta_data",
        })
    }
}
