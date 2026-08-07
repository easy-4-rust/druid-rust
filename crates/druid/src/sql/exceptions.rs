use std::collections::HashMap;

pub use crate::core::SqlExceptionKind;

/// Base exception for database access failures and other SQL errors.
///
/// Corresponds to Java: `java.sql.SQLException`. It preserves reason, `SQLState`, vendor code,
/// cause information, and the independent next-exception chain.
pub type SqlException = crate::core::SqlException;
/// Chain of database access warnings that do not necessarily stop an operation.
///
/// Corresponds to Java: `java.sql.SQLWarning`. Warnings may be attached to connections,
/// statements, and result sets. Reading a warning does not clear it.
pub type SqlWarning = crate::core::SqlWarning;
/// Transient failure that may succeed on retry without application intervention.
pub type SqlTransientException = crate::core::SqlException;
/// Non-transient failure that requires correction before retry.
pub type SqlNonTransientException = crate::core::SqlException;
/// Failure that may succeed after connection or transaction recovery.
pub type SqlRecoverableException = crate::core::SqlException;
/// Transient connection failure in `SQLState` class `08`.
pub type SqlTransientConnectionException = crate::core::SqlException;
/// Non-transient connection failure in `SQLState` class `08`.
pub type SqlNonTransientConnectionException = crate::core::SqlException;
/// Data failure in `SQLState` class `22`.
pub type SqlDataException = crate::core::SqlException;
/// Integrity-constraint failure in `SQLState` class `23`.
pub type SqlIntegrityConstraintViolationException = crate::core::SqlException;
/// Invalid authorization specification in `SQLState` class `28`.
pub type SqlInvalidAuthorizationSpecException = crate::core::SqlException;
/// SQL syntax or access-rule failure in `SQLState` class `42`.
pub type SqlSyntaxErrorException = crate::core::SqlException;
/// Transaction rollback failure in `SQLState` class `40`.
pub type SqlTransactionRollbackException = crate::core::SqlException;
/// Unsupported feature in `SQLState` class `0A`.
pub type SqlFeatureNotSupportedException = crate::core::SqlException;
/// RDBC query or login timeout.
pub type SqlTimeoutException = crate::core::SqlException;

/// Error raised when at least one command in a batch update fails.
///
/// Corresponds to Java: `java.sql.BatchUpdateException`. Counts follow command order. A driver
/// may stop at the first failure or continue with `EXECUTE_FAILED(-3)`; unknown successful counts
/// use `SUCCESS_NO_INFO(-2)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchUpdateException {
    /// Underlying SQL exception information.
    pub exception: SqlException,
    /// Ordered update counts, including standard success-unknown and failed markers.
    pub update_counts: Vec<i64>,
}

impl BatchUpdateException {
    /// Creates a batch exception from an SQL exception and command-ordered counts.
    #[must_use]
    pub fn new(exception: SqlException, update_counts: Vec<i64>) -> Self {
        Self {
            exception,
            update_counts,
        }
    }

    /// Returns Java `int[] getUpdateCounts()` semantics; out-of-range values are saturated.
    #[must_use]
    pub fn update_counts(&self) -> Vec<i32> {
        self.update_counts
            .iter()
            .map(|value| {
                i32::try_from(*value).unwrap_or(if *value < 0 { i32::MIN } else { i32::MAX })
            })
            .collect()
    }

    /// Returns RDBC 4.2 `long[] getLargeUpdateCounts()` semantics.
    #[must_use]
    pub fn large_update_counts(&self) -> &[i64] {
        &self.update_counts
    }
}

/// Diagnostic information for unexpected truncation unrelated to `maxFieldSize`.
///
/// Corresponds to Java: `java.sql.DataTruncation`. Write truncation is an exception and read
/// truncation is a warning. `index` is a 1-based parameter or column position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataTruncation {
    /// Parameter or column index.
    pub index: usize,
    /// Whether the truncated value is a parameter.
    pub parameter: bool,
    /// Whether truncation occurred while reading; `false` means writing.
    pub read: bool,
    /// Original byte length, or `None` when unknown.
    pub data_size: Option<usize>,
    /// Transferred byte length, or `None` when unknown.
    pub transfer_size: Option<usize>,
}

/// Failure to set one or more connection client-information properties.
///
/// Corresponds to Java: `java.sql.SQLClientInfoException`. `failed_properties` contains only
/// rejected properties and their standard status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlClientInfoException {
    /// Underlying SQL exception information.
    pub exception: SqlException,
    /// Rejected property names and standard status names.
    pub failed_properties: HashMap<String, String>,
}
