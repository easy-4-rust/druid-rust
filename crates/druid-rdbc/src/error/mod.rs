//! RDBC 错误与 SQL 异常合同的声明入口。

mod batch_update_counts;
mod rdbc_error;
mod rdbc_error_kind;
mod sql_exception;
mod sql_warning;

pub use batch_update_counts::BatchUpdateCounts;
pub use rdbc_error::{ProductErrorBoundary, RdbcError};
pub use rdbc_error_kind::RdbcErrorKind;
pub use sql_exception::SqlException;
pub use sql_exception::SqlExceptionCause;
pub use sql_exception::SqlExceptionKind;
pub use sql_warning::SqlWarning;
