//! JDBC-like 语句标准与类型化描述符。

mod callable_statement;
mod prepared_input_parameter;
mod prepared_statement;
#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod statement;
mod statement_cancellation;

pub use callable_statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
};
pub use prepared_input_parameter::{PreparedInputParameter, PreparedTypeNameArgument};
pub use prepared_statement::{ParameterIndex, PreparedParameter, PreparedStatement};
pub use statement::{
    CurrentResultDisposition, ExecResult, ExecutionResult, FetchDirection, GeneratedKeys,
    ResultSetConcurrency, ResultSetHoldability, ResultSetType, Statement, StatementExecuteResult,
    StatementOptions,
};
pub use statement_cancellation::StatementCancellation;
