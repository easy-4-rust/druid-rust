use crate::error::RdbcError;
use crate::metadata::ParameterMetaData;
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

/// Canonical name for a prepared input parameter snapshot。
pub type PreparedInputParameter = PreparedParameter;

/// Optional SQL type-name argument preserved from JDBC setter overloads。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreparedTypeNameArgument {
    /// Setter overload omitted the type name。
    Omitted,
    /// Setter supplied an explicit type name。
    Named(String),
}

/// JDBC `PreparedStatement` 标准。
#[async_trait::async_trait]
pub trait PreparedStatement: Statement {
    /// 设置一个 typed parameter snapshot。
    fn set_parameter(&mut self, _parameter: PreparedParameter) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_set_parameter",
        })
    }

    /// 清空当前 parameter snapshots。
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
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

    /// 返回 immutable parameter metadata snapshot。
    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "prepared_statement_parameter_meta_data",
        })
    }
}
