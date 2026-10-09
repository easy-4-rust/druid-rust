pub use crate::error::SqlExceptionKind;

/// Base exception for database access failures and other SQL errors.
///
/// Corresponds to Java: `java.sql.SQLException`. It preserves reason, `SQLState`, vendor code,
/// cause information, and the independent next-exception chain.
pub type SqlException = crate::error::SqlException;
/// Chain of database access warnings that do not necessarily stop an operation.
///
/// Corresponds to Java: `java.sql.SQLWarning`. Warnings may be attached to connections,
/// statements, and result sets. Reading a warning does not clear it.
pub type SqlWarning = crate::error::SqlWarning;
/// Transient failure that may succeed on retry without application intervention.
pub type SqlTransientException = crate::error::SqlException;
/// Non-transient failure that requires correction before retry.
pub type SqlNonTransientException = crate::error::SqlException;
/// Failure that may succeed after connection or transaction recovery.
pub type SqlRecoverableException = crate::error::SqlException;
/// Transient connection failure in `SQLState` class `08`.
pub type SqlTransientConnectionException = crate::error::SqlException;
/// Non-transient connection failure in `SQLState` class `08`.
pub type SqlNonTransientConnectionException = crate::error::SqlException;
/// Data failure in `SQLState` class `22`.
pub type SqlDataException = crate::error::SqlException;
/// Integrity-constraint failure in `SQLState` class `23`.
pub type SqlIntegrityConstraintViolationException = crate::error::SqlException;
/// Invalid authorization specification in `SQLState` class `28`.
pub type SqlInvalidAuthorizationSpecException = crate::error::SqlException;
/// SQL syntax or access-rule failure in `SQLState` class `42`.
pub type SqlSyntaxErrorException = crate::error::SqlException;
/// Transaction rollback failure in `SQLState` class `40`.
pub type SqlTransactionRollbackException = crate::error::SqlException;
/// Unsupported feature in `SQLState` class `0A`.
pub type SqlFeatureNotSupportedException = crate::error::SqlException;
/// RDBC query or login timeout.
pub type SqlTimeoutException = crate::error::SqlException;

// 历史兼容入口重导出唯一标准对象；真实定义和逻辑归标准 error 层所有。
pub use crate::error::{BatchUpdateException, DataTruncation, SqlClientInfoException};
