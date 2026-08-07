use crate::core::DruidError;
use std::io::Write;
use std::sync::Arc;

/// Thread-safe Rust mapping of an RDBC diagnostic writer.
///
/// Driver and data-source diagnostics may contain SQL, table names, or values. Callers must
/// protect and redact this output with the same care as credentials.
pub type RdbcLogWriter = Arc<parking_lot::Mutex<Box<dyn Write + Send>>>;

/// Configuration shared by data sources, pooled data sources, and XA data sources.
///
/// Corresponds to Java: `javax.sql.CommonDataSource`. The login timeout limits physical
/// connection establishment; it is not a statement query timeout.
pub trait CommonDataSource: Send + Sync {
    /// Returns the maximum physical-login wait in seconds; zero means no limit.
    ///
    /// Corresponds to Java: `CommonDataSource#getLoginTimeout`.
    fn login_timeout(&self) -> u64 {
        0
    }
    /// Returns the maximum physical-login wait in seconds.
    ///
    /// This is the `snake_case` form of Java `getLoginTimeout`.
    fn get_login_timeout(&self) -> u64 {
        self.login_timeout()
    }

    /// Changes the physical-login timeout in seconds.
    ///
    /// `seconds` is the maximum wait; zero means no limit. An immutable, already-built data
    /// source returns `UnsupportedOperation` instead of changing only a cosmetic value.
    ///
    /// Druid pool configuration is immutable after initialization.
    fn set_login_timeout(&self, _seconds: u64) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_login_timeout_after_build",
        })
    }

    /// Returns the RDBC log writer, or `None` when diagnostics are disabled.
    ///
    /// Corresponds to Java: `CommonDataSource#getLogWriter`. The returned handle shares the
    /// same output destination.
    fn get_log_writer(&self) -> Option<RdbcLogWriter> {
        None
    }

    /// Sets the RDBC log writer; `None` disables diagnostic output.
    ///
    /// Corresponds to Java: `CommonDataSource#setLogWriter`. Immutable implementations return
    /// `UnsupportedOperation` for runtime changes.
    fn set_log_writer(&self, _writer: Option<RdbcLogWriter>) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_log_writer_after_build",
        })
    }

    /// Returns the parent logger name used by the implementation.
    ///
    /// Corresponds to Java: `CommonDataSource#getParentLogger`.
    fn parent_logger(&self) -> &'static str {
        "druid::rdbc"
    }

    /// Returns the parent logger name used by the implementation.
    ///
    /// This is the `snake_case` form of Java `getParentLogger`.
    fn get_parent_logger(&self) -> &'static str {
        self.parent_logger()
    }
}
