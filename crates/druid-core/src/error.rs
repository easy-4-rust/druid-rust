//! 对应 Java 类：com.alibaba.druid.pool.GetConnectionTimeoutException 等
//!
//! druid-rust 统一错误枚举，替代 SQLException 层级。

use std::fmt;
use std::time::Duration;

/// druid-rust 统一错误类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DruidError {
    PoolClosed,
    AcquireTimeout,
    PoolExhausted,
    ValidationFailed(String),
    ConnectionLeaked { id: u64, held_for: Duration },
    ConnectionDiscarded,
    DriverError(String),
    SqlParseError(String),
    WallViolation(String),
    DataSourceNotFound(String),
    UnsupportedOperation { operation: &'static str },
    Other(String),
}

impl fmt::Display for DruidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PoolClosed => write!(f, "connection pool is closed"),
            Self::AcquireTimeout => write!(f, "acquire connection timed out"),
            Self::PoolExhausted => write!(f, "connection pool exhausted"),
            Self::ValidationFailed(msg) => write!(f, "connection validation failed: {msg}"),
            Self::ConnectionLeaked { id, held_for } => write!(f, "connection {id} leaked, held for {held_for:?}"),
            Self::ConnectionDiscarded => write!(f, "connection has been discarded"),
            Self::DriverError(msg) => write!(f, "driver error: {msg}"),
            Self::SqlParseError(msg) => write!(f, "SQL parse error: {msg}"),
            Self::WallViolation(msg) => write!(f, "wall violation: {msg}"),
            Self::DataSourceNotFound(name) => write!(f, "datasource not found: {name}"),
            Self::UnsupportedOperation { operation } => {
                write!(f, "operation is not supported by the physical connection: {operation}")
            }
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for DruidError {}

impl From<String> for DruidError {
    fn from(s: String) -> Self { Self::Other(s) }
}
impl From<&str> for DruidError {
    fn from(s: &str) -> Self { Self::Other(s.to_string()) }
}
