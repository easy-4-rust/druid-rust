//! RDBC 错误与 SQL 异常合同的声明入口。

mod sql_exception;
mod sql_warning;

pub use sql_exception::SqlException;
pub use sql_exception::SqlExceptionCause;
pub use sql_exception::SqlExceptionKind;
pub use sql_warning::SqlWarning;
