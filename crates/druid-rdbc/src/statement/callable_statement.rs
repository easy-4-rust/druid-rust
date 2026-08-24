use crate::error::RdbcError;
use crate::statement::{ParameterIndex, PreparedStatement};
use crate::types::Value;

/// Callable out-parameter descriptor。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableOutParameter {
    /// 1-based parameter index。
    pub index: ParameterIndex,
    /// 驱动 SQL type name。
    pub type_name: String,
}

/// Callable input parameter descriptor。
#[derive(Debug, Clone, PartialEq)]
pub enum CallableInputParameter {
    /// 输入 value snapshot。
    Value(crate::statement::PreparedParameter),
}

/// Callable parameter direction descriptor。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CallableParameter {
    /// Input-only parameter。
    In(CallableInputParameter),
    /// Output-only parameter。
    Out(CallableOutParameter),
    /// Input/output parameter。
    InOut {
        /// Input snapshot。
        input: CallableInputParameter,
        /// Output descriptor。
        output: CallableOutParameter,
    },
}

/// JDBC `CallableStatement` 标准。
#[async_trait::async_trait]
pub trait CallableStatement: PreparedStatement {
    /// 注册一个 typed out parameter。
    fn register_out_parameter(
        &mut self,
        _parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "callable_statement_register_out_parameter",
        })
    }

    /// 读取执行后的 out value。
    fn out_value(&self, _index: ParameterIndex) -> Result<Value, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "callable_statement_out_value",
        })
    }
}
