//! JDBC-like 语句标准与类型化描述符。

mod callable_statement;
mod prepared_statement;
#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod statement;

pub use callable_statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
};
pub use prepared_statement::{
    ParameterIndex, PreparedInputParameter, PreparedParameter, PreparedStatement,
    PreparedTypeNameArgument,
};
pub use statement::{
    ExecResult, ExecutionResult, GeneratedKeys, ResultSetConcurrency, ResultSetHoldability,
    ResultSetType, Statement, StatementExecuteResult, StatementOptions,
};
