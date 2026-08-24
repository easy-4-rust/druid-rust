//! Java 语义兼容合同的声明入口。

mod exceptions;

pub use exceptions::BatchUpdateException;
pub use exceptions::DataTruncation;
pub use exceptions::SqlClientInfoException;
pub use exceptions::SqlDataException;
pub use exceptions::SqlException;
pub use exceptions::SqlExceptionKind;
pub use exceptions::SqlFeatureNotSupportedException;
pub use exceptions::SqlIntegrityConstraintViolationException;
pub use exceptions::SqlInvalidAuthorizationSpecException;
pub use exceptions::SqlNonTransientConnectionException;
pub use exceptions::SqlNonTransientException;
pub use exceptions::SqlRecoverableException;
pub use exceptions::SqlSyntaxErrorException;
pub use exceptions::SqlTimeoutException;
pub use exceptions::SqlTransactionRollbackException;
pub use exceptions::SqlTransientConnectionException;
pub use exceptions::SqlTransientException;
pub use exceptions::SqlWarning;
