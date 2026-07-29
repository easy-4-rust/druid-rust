//! 对应 Java 类：com.alibaba.druid.pool.GetConnectionTimeoutException 等
//!
//! druid-rust 统一错误枚举，替代 SQLException 层级。

use std::fmt;
use std::time::Duration;

use super::SqlException;

/// druid-rust 统一错误类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DruidError {
    PoolClosed,
    AcquireTimeout,
    PoolExhausted,
    /// 等待获取连接的任务数超过数据源上限。
    ///
    /// 对应 Java `DruidDataSource#getConnectionInternal` 的
    /// `maxWaitThreadCount` 分支。
    MaxWaitThreadCountExceeded {
        max: usize,
        current: usize,
    },
    ValidationFailed(String),
    ConnectionLeaked {
        id: u64,
        held_for: Duration,
    },
    ConnectionDiscarded,
    DriverError(String),
    /// 保留 JDBC/驱动异常分类字段的 SQL 执行错误。
    ///
    /// 对应 Java `SQLException`；池化连接会把该对象交给
    /// `ExceptionSorter`，但仍把原始错误返回调用者。
    SqlException(Box<SqlException>),
    /// JDBC 批处理异常及已经完成项的更新计数。
    ///
    /// 对应 Java `java.sql.BatchUpdateException#getUpdateCounts()`。计数允许
    /// `Statement::SUCCESS_NO_INFO(-2)` 与 `Statement::EXECUTE_FAILED(-3)`；
    /// `cause` 保留原始驱动错误，供异常分类器判断连接是否应丢弃。
    BatchUpdateException {
        update_counts: Vec<i32>,
        cause: Box<DruidError>,
    },
    SqlParseError(String),
    WallViolation(String),
    DataSourceNotFound(String),
    /// 数据源管理开关已禁用。
    ///
    /// 对应 Java `DataSourceDisableException`。
    DataSourceDisabled,
    /// 高可用数据源当前没有可用节点。
    ///
    /// 对应 Java `DataSourceNotAvailableException`，不能与名称不存在混同。
    DataSourceNotAvailable(String),
    /// 公共 API 参数不满足 Java 合约。
    InvalidArgument(String),
    UnsupportedOperation {
        operation: &'static str,
    },
    Other(String),
}

impl fmt::Display for DruidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PoolClosed => write!(f, "connection pool is closed"),
            Self::AcquireTimeout => write!(f, "acquire connection timed out"),
            Self::PoolExhausted => write!(f, "connection pool exhausted"),
            Self::MaxWaitThreadCountExceeded { max, current } => write!(
                f,
                "maxWaitThreadCount {max}, current wait task count {current}"
            ),
            Self::ValidationFailed(msg) => write!(f, "connection validation failed: {msg}"),
            Self::ConnectionLeaked { id, held_for } => {
                write!(f, "connection {id} leaked, held for {held_for:?}")
            }
            Self::ConnectionDiscarded => write!(f, "connection has been discarded"),
            Self::DriverError(msg) => write!(f, "driver error: {msg}"),
            Self::SqlException(exception) => write!(
                f,
                "SQL exception (code={}, state={}): {}",
                exception.error_code(),
                exception.sql_state().unwrap_or("null"),
                exception.message().unwrap_or("null")
            ),
            Self::BatchUpdateException {
                update_counts,
                cause,
            } => write!(
                f,
                "batch update failed after {} result(s): {cause}",
                update_counts.len()
            ),
            Self::SqlParseError(msg) => write!(f, "SQL parse error: {msg}"),
            Self::WallViolation(msg) => write!(f, "wall violation: {msg}"),
            Self::DataSourceNotFound(name) => write!(f, "datasource not found: {name}"),
            Self::DataSourceDisabled => write!(f, "datasource is disabled"),
            Self::DataSourceNotAvailable(name) => {
                write!(f, "datasource is not available: {name}")
            }
            Self::InvalidArgument(message) => write!(f, "invalid argument: {message}"),
            Self::UnsupportedOperation { operation } => {
                write!(
                    f,
                    "operation is not supported by the physical connection: {operation}"
                )
            }
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for DruidError {}

impl DruidError {
    /// 返回当前错误或批处理 cause 中保留的 JDBC `SQLException`。
    ///
    /// 批处理异常仍必须进入与普通 SQL 异常相同的 vendor fatal sorter。
    pub fn sql_exception(&self) -> Option<&SqlException> {
        match self {
            Self::SqlException(exception) => Some(exception),
            Self::BatchUpdateException { cause, .. } => cause.sql_exception(),
            _ => None,
        }
    }

    /// 返回批处理失败前已经得到的 JDBC 更新计数。
    pub fn batch_update_counts(&self) -> Option<&[i32]> {
        match self {
            Self::BatchUpdateException { update_counts, .. } => Some(update_counts),
            _ => None,
        }
    }
}

impl From<String> for DruidError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}
impl From<&str> for DruidError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}
