//! 对应 Java 类：com.alibaba.druid.pool.GetConnectionTimeoutException 等
//!
//! druid-rust 统一错误枚举，替代 SQLException 层级。

use std::fmt;
use std::time::Duration;

use super::{JavaString, SqlException};

/// druid-rust 统一错误类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DruidError {
    /// Rust 外部池已关闭，但没有 Druid 数据源的关闭时间元数据。
    PoolClosed,
    /// 数据源已经关闭，并保留 Java 异常消息使用的关闭时刻。
    ///
    /// 对应 Java `DataSourceClosedException`。Rust 不复制 `SQLException`
    /// 继承层级，但错误分类、关闭时间与可观察消息保持一致。
    DataSourceClosed {
        close_time_millis: u64,
    },
    /// Rust 外部池或驱动层的通用获取超时。
    AcquireTimeout,
    /// Java `GetConnectionTimeoutException` 的 Druid 原生池诊断。
    GetConnectionTimeout {
        wait_millis: u64,
        active_count: usize,
        max_active: usize,
        creating_count: usize,
        create_elapsed_millis: Option<u64>,
        create_error_count: u64,
        running_sql: Vec<(u64, String)>,
        cause: Option<Box<DruidError>>,
    },
    /// 物理驱动建连超过数据源 `loginTimeout`。
    LoginTimeout,
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
    /// 保留 RDBC/驱动异常分类字段的 SQL 执行错误。
    ///
    /// 对应 Java `SQLException`；池化连接会把该对象交给
    /// `ExceptionSorter`，但仍把原始错误返回调用者。
    SqlException(Box<SqlException>),
    /// RDBC 批处理异常及已经完成项的更新计数。
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
    /// 数据源仍有借出连接，不能执行重启。
    ///
    /// 对应 Java `DruidDataSource#restart(Properties)` 在 `activeCount > 0`
    /// 时抛出的 `SQLException`。独立变体保留可观察的失败原因与活动数。
    ActiveConnectionsPreventRestart {
        active_count: usize,
    },
    /// 数据源处于 Java `onFatalError` 并达到活动连接门限。
    ///
    /// `last_sql` 使用无损 UTF-16 值对象，保留 Java
    /// `String#substring(0, 1024)` 可能截断在 surrogate 中间的行为。
    OnFatalError {
        active_count: usize,
        max_active: i32,
        last_error_time_millis: u64,
        last_sql: Option<JavaString>,
        cause: Option<Box<DruidError>>,
    },
    /// 高可用数据源当前没有可用节点。
    ///
    /// 对应 Java `DataSourceNotAvailableException`，不能与名称不存在混同。
    DataSourceNotAvailable {
        cause: Option<Box<DruidError>>,
    },
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
            Self::DataSourceClosed { close_time_millis } => {
                use chrono::{Local, TimeZone};
                write!(f, "dataSource already closed at ")?;
                match i64::try_from(*close_time_millis)
                    .ok()
                    .and_then(|millis| Local.timestamp_millis_opt(millis).single())
                {
                    Some(time) => write!(f, "{}", time.format("%a %b %d %H:%M:%S %Z %Y")),
                    None => write!(f, "{close_time_millis}"),
                }
            }
            Self::AcquireTimeout => write!(f, "acquire connection timed out"),
            Self::GetConnectionTimeout {
                wait_millis,
                active_count,
                max_active,
                creating_count,
                create_elapsed_millis,
                create_error_count,
                running_sql,
                ..
            } => {
                write!(
                    f,
                    "wait millis {wait_millis}, active {active_count}, maxActive {max_active}, creating {creating_count}"
                )?;
                if let Some(create_elapsed_millis) =
                    create_elapsed_millis.filter(|value| *value > 0)
                {
                    write!(f, ", createElapseMillis {create_elapsed_millis}")?;
                }
                if *create_error_count > 0 {
                    write!(f, ", createErrorCount {create_error_count}")?;
                }
                for (index, (running_count, sql)) in running_sql.iter().enumerate() {
                    if index == 0 {
                        write!(f, ", ")?;
                    } else {
                        writeln!(f)?;
                    }
                    write!(f, "runningSqlCount {running_count} : {sql}")?;
                }
                Ok(())
            }
            Self::LoginTimeout => write!(f, "physical connection login timed out"),
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
            Self::ActiveConnectionsPreventRestart { active_count } => {
                write!(
                    f,
                    "can not restart, active connection count not zero: {active_count}"
                )
            }
            Self::OnFatalError {
                active_count,
                max_active,
                last_error_time_millis,
                last_sql,
                ..
            } => {
                write!(
                    f,
                    "onFatalError, activeCount {active_count}, onFatalErrorMaxActive {max_active}"
                )?;
                if *last_error_time_millis > 0 {
                    use chrono::{Local, TimeZone};
                    if let Some(time) = Local
                        .timestamp_millis_opt(*last_error_time_millis as i64)
                        .single()
                    {
                        write!(f, ", time '{}'", time.format("%Y-%m-%d %H:%M:%S"))?;
                    }
                }
                if let Some(last_sql) = last_sql {
                    write!(
                        f,
                        ", sql \n{}",
                        String::from_utf16_lossy(last_sql.as_utf16())
                    )?;
                }
                Ok(())
            }
            Self::DataSourceNotAvailable { cause: Some(cause) } => write!(f, "{cause}"),
            Self::DataSourceNotAvailable { cause: None } => {
                write!(f, "datasource is not available")
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

impl std::error::Error for DruidError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BatchUpdateException { cause, .. } => Some(cause.as_ref()),
            Self::OnFatalError {
                cause: Some(cause), ..
            } => Some(cause.as_ref()),
            Self::DataSourceNotAvailable { cause: Some(cause) } => Some(cause.as_ref()),
            Self::GetConnectionTimeout {
                cause: Some(cause), ..
            } => Some(cause.as_ref()),
            _ => None,
        }
    }
}

impl DruidError {
    /// 返回稳定的错误类型名称，供统计管理面映射 Java `Throwable#getClass()`。
    #[must_use]
    pub fn class_name(&self) -> &str {
        match self {
            Self::PoolClosed => "druid::PoolClosed",
            Self::DataSourceClosed { .. } => "com.alibaba.druid.pool.DataSourceClosedException",
            Self::AcquireTimeout => "druid::AcquireTimeout",
            Self::GetConnectionTimeout { .. } => {
                "com.alibaba.druid.pool.GetConnectionTimeoutException"
            }
            Self::LoginTimeout => "druid::LoginTimeout",
            Self::PoolExhausted => "druid::PoolExhausted",
            Self::MaxWaitThreadCountExceeded { .. } => "druid::MaxWaitThreadCountExceeded",
            Self::ValidationFailed(_) => "druid::ValidationFailed",
            Self::ConnectionLeaked { .. } => "druid::ConnectionLeaked",
            Self::ConnectionDiscarded => "druid::ConnectionDiscarded",
            Self::DriverError(_) => "druid::DriverError",
            Self::SqlException(exception) => exception.class_name(),
            Self::BatchUpdateException { .. } => "java.sql.BatchUpdateException",
            Self::SqlParseError(_) => "druid::SqlParseError",
            Self::WallViolation(_) => "druid::WallViolation",
            Self::DataSourceNotFound(_) => "druid::DataSourceNotFound",
            Self::DataSourceDisabled => "com.alibaba.druid.pool.DataSourceDisableException",
            Self::ActiveConnectionsPreventRestart { .. } => {
                "druid::ActiveConnectionsPreventRestart"
            }
            Self::OnFatalError { .. } => "java.sql.SQLException",
            Self::DataSourceNotAvailable { .. } => "druid::DataSourceNotAvailable",
            Self::InvalidArgument(_) => "druid::InvalidArgument",
            Self::UnsupportedOperation { .. } => "druid::UnsupportedOperation",
            Self::Other(_) => "druid::Other",
        }
    }

    /// 返回当前错误或批处理 cause 中保留的 RDBC `SQLException`。
    ///
    /// 批处理异常仍必须进入与普通 SQL 异常相同的 vendor fatal sorter。
    pub fn sql_exception(&self) -> Option<&SqlException> {
        match self {
            Self::SqlException(exception) => Some(exception),
            Self::BatchUpdateException { cause, .. } => cause.sql_exception(),
            _ => None,
        }
    }

    /// 返回批处理失败前已经得到的 RDBC 更新计数。
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
