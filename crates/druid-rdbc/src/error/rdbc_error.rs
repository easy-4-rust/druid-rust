//! RDBC standard 与数据库 adapter 共享的错误合同。

use std::fmt;
use std::time::Duration;

use super::{BatchUpdateCounts, RdbcErrorKind, SqlException};

/// 上层产品错误跨越 RDBC 获取边界时的通用结构化表示。
///
/// 该对象只保存可观察 class、message 和 typed source，不在 `druid-rdbc` 中复制 Druid
/// 产品错误枚举。具体产品 taxonomy 仍由产品 crate 独占。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductErrorBoundary {
    class_name: String,
    message: String,
    source: Option<Box<RdbcError>>,
}

impl ProductErrorBoundary {
    /// 创建一个产品边界错误快照。
    #[must_use]
    pub fn new(
        class_name: impl Into<String>,
        message: impl Into<String>,
        source: Option<RdbcError>,
    ) -> Self {
        Self {
            class_name: class_name.into(),
            message: message.into(),
            source: source.map(Box::new),
        }
    }

    /// 返回产品公开的稳定错误类名。
    #[must_use]
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// 返回产品公开的错误文本。
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 返回转换后仍可 downcast 的 typed source。
    #[must_use]
    pub fn source_error(&self) -> Option<&RdbcError> {
        self.source.as_deref()
    }
}

/// RDBC standard 与 adapter 边界的统一错误类型。
///
/// 对应 Java：`java.sql.SQLException` 谱系、JDBC 默认方法的
/// `UnsupportedOperationException`，以及数据库驱动/连接获取边界。该类型不包含 Druid
/// 产品的池诊断字段；Druid 在产品边界通过自己的 `DruidError::Rdbc` 包装一次。
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdbcError {
    /// 外部连接池或 adapter 资源池已关闭。
    PoolClosed,
    /// 获取连接超时。
    AcquireTimeout,
    /// 保留池容量、创建和运行 SQL 诊断的获取超时。
    GetConnectionTimeout {
        wait_millis: u64,
        active_count: usize,
        max_active: usize,
        creating_count: usize,
        create_elapsed_millis: Option<u64>,
        create_error_count: u64,
        running_sql: Vec<(u64, String)>,
        cause: Option<Box<RdbcError>>,
    },
    /// 物理驱动登录超时。
    LoginTimeout,
    /// 数据源被禁用。
    DataSourceDisabled,
    /// 数据源已经关闭，并保留关闭时刻。
    DataSourceClosed {
        /// Unix epoch 毫秒关闭时刻。
        close_time_millis: u64,
    },
    /// 连接池容量已耗尽。
    PoolExhausted,
    /// 数据源名称未注册。
    DataSourceNotFound(String),
    /// 当前没有可用数据源节点。
    DataSourceNotAvailable { cause: Option<Box<RdbcError>> },
    /// 等待获取连接的任务数超过上限。
    MaxWaitThreadCountExceeded { max: usize, current: usize },
    /// 连接已失效或被丢弃。
    ConnectionDiscarded,
    /// logical connection 未按生命周期合同释放。
    ConnectionLeaked {
        /// logical connection 标识。
        id: u64,
        /// 被持有的持续时间。
        held_for: Duration,
    },
    /// 连接校验失败。
    ValidationFailed(String),
    /// 具体数据库驱动或 adapter 执行失败。
    DriverError(String),
    /// 保留 SQLState、vendor code、cause 分类和 next-exception 链的 SQL 异常。
    SqlException(Box<SqlException>),
    /// 保留 JDBC 4.2 大更新计数和原始 typed cause 的批处理异常。
    BatchUpdateException {
        /// 按命令顺序保存的无损 64 位更新计数。
        update_counts: BatchUpdateCounts,
        /// 原始驱动或 SQL 异常。
        cause: Box<RdbcError>,
    },
    /// 调用参数不满足标准合同。
    InvalidArgument(String),
    /// SQL 文本解析失败。
    SqlParseError(String),
    /// SQL 防火墙拒绝语句。
    WallViolation(String),
    /// 没有注册匹配的驱动扩展。
    NoDriverExtension { db_type: String },
    /// Java JDBC 默认方法的 `UnsupportedOperationException` 映射。
    UnsupportedOperation {
        /// 不受支持的 JDBC/RDBC 操作名称。
        operation: &'static str,
    },
    /// 不复制产品 taxonomy 的结构化上层错误。
    ProductBoundary(ProductErrorBoundary),
    /// 未能进一步分类的 adapter 错误。
    Other(String),
}

impl RdbcError {
    /// 创建标准 SQL feature unsupported 错误。
    ///
    /// # 参数
    /// - `operation`：不受支持的 JDBC/RDBC 操作名称。
    #[must_use]
    pub fn feature_not_supported(operation: &'static str) -> Self {
        Self::SqlException(Box::new(
            SqlException::new(
                0,
                Some("0A000".to_owned()),
                Some(format!("RDBC operation is not supported: {operation}")),
            )
            .with_class_name("java.sql.SQLFeatureNotSupportedException")
            .with_assignable_type("java.sql.SQLNonTransientException"),
        ))
    }

    /// 创建资源 free/close 后继续使用时的标准 SQL 异常。
    ///
    /// # 参数
    /// - `resource_name`：已经失效的资源类型名称。
    #[must_use]
    pub fn rdbc_resource_closed(resource_name: &'static str) -> Self {
        Self::SqlException(Box::new(SqlException::new(
            0,
            Some("HY010".to_owned()),
            Some(format!("{resource_name} has been freed or invalidated")),
        )))
    }

    /// 返回稳定错误分类。
    #[must_use]
    pub fn kind(&self) -> RdbcErrorKind {
        match self {
            Self::PoolClosed => RdbcErrorKind::PoolClosed,
            Self::AcquireTimeout => RdbcErrorKind::AcquireTimeout,
            Self::GetConnectionTimeout { .. } => RdbcErrorKind::GetConnectionTimeout,
            Self::LoginTimeout => RdbcErrorKind::LoginTimeout,
            Self::DataSourceDisabled => RdbcErrorKind::DataSourceDisabled,
            Self::DataSourceClosed { .. } => RdbcErrorKind::DataSourceClosed,
            Self::PoolExhausted => RdbcErrorKind::PoolExhausted,
            Self::DataSourceNotFound(_) => RdbcErrorKind::DataSourceNotFound,
            Self::DataSourceNotAvailable { .. } => RdbcErrorKind::DataSourceNotAvailable,
            Self::MaxWaitThreadCountExceeded { .. } => RdbcErrorKind::MaxWaitThreadCountExceeded,
            Self::ConnectionDiscarded => RdbcErrorKind::ConnectionDiscarded,
            Self::ConnectionLeaked { .. } => RdbcErrorKind::ConnectionLeaked,
            Self::ValidationFailed(_) => RdbcErrorKind::ValidationFailed,
            Self::DriverError(_) => RdbcErrorKind::Driver,
            Self::SqlException(exception)
                if exception.kind() == super::SqlExceptionKind::FeatureNotSupported =>
            {
                RdbcErrorKind::SqlFeatureNotSupported
            }
            Self::SqlException(_) => RdbcErrorKind::SqlException,
            Self::BatchUpdateException { .. } => RdbcErrorKind::BatchUpdateException,
            Self::InvalidArgument(_) => RdbcErrorKind::InvalidArgument,
            Self::SqlParseError(_) => RdbcErrorKind::SqlParse,
            Self::WallViolation(_) => RdbcErrorKind::WallViolation,
            Self::NoDriverExtension { .. } => RdbcErrorKind::NoDriverExtension,
            Self::UnsupportedOperation { .. } => RdbcErrorKind::UnsupportedOperation,
            Self::ProductBoundary(_) => RdbcErrorKind::ProductBoundary,
            Self::Other(_) => RdbcErrorKind::Other,
        }
    }

    /// 返回当前错误或批处理 cause 中保留的 SQL 异常。
    #[must_use]
    pub fn sql_exception(&self) -> Option<&SqlException> {
        match self {
            Self::SqlException(exception) => Some(exception),
            Self::BatchUpdateException { cause, .. } => cause.sql_exception(),
            Self::ProductBoundary(boundary) => {
                boundary.source_error().and_then(Self::sql_exception)
            }
            _ => None,
        }
    }

    /// 返回批处理失败前已经得到的无损更新计数。
    #[must_use]
    pub fn batch_update_counts(&self) -> Option<&BatchUpdateCounts> {
        match self {
            Self::BatchUpdateException { update_counts, .. } => Some(update_counts),
            Self::ProductBoundary(boundary) => {
                boundary.source_error().and_then(Self::batch_update_counts)
            }
            _ => None,
        }
    }

    /// 返回与 Java/JDBC taxonomy 对齐的稳定错误类名。
    #[must_use]
    pub fn class_name(&self) -> &str {
        match self {
            Self::PoolClosed => "rdbc::PoolClosed",
            Self::AcquireTimeout => "rdbc::AcquireTimeout",
            Self::GetConnectionTimeout { .. } => {
                "com.alibaba.druid.pool.GetConnectionTimeoutException"
            }
            Self::LoginTimeout => "rdbc::LoginTimeout",
            Self::DataSourceDisabled => "rdbc::DataSourceDisabled",
            Self::DataSourceClosed { .. } => "com.alibaba.druid.pool.DataSourceClosedException",
            Self::PoolExhausted => "rdbc::PoolExhausted",
            Self::DataSourceNotFound(_) => "rdbc::DataSourceNotFound",
            Self::DataSourceNotAvailable { .. } => "rdbc::DataSourceNotAvailable",
            Self::MaxWaitThreadCountExceeded { .. } => "rdbc::MaxWaitThreadCountExceeded",
            Self::ConnectionDiscarded => "rdbc::ConnectionDiscarded",
            Self::ConnectionLeaked { .. } => "rdbc::ConnectionLeaked",
            Self::ValidationFailed(_) => "rdbc::ValidationFailed",
            Self::DriverError(_) => "rdbc::DriverError",
            Self::SqlException(exception) => exception.class_name(),
            Self::BatchUpdateException { .. } => "java.sql.BatchUpdateException",
            Self::InvalidArgument(_) => "rdbc::InvalidArgument",
            Self::SqlParseError(_) => "rdbc::SqlParseError",
            Self::WallViolation(_) => "rdbc::WallViolation",
            Self::NoDriverExtension { .. } => "rdbc::NoDriverExtension",
            Self::UnsupportedOperation { .. } => "java.lang.UnsupportedOperationException",
            Self::ProductBoundary(boundary) => boundary.class_name(),
            Self::Other(_) => "rdbc::Other",
        }
    }
}

impl fmt::Display for RdbcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PoolClosed => write!(formatter, "connection pool is closed"),
            Self::AcquireTimeout => write!(formatter, "acquire connection timed out"),
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
                    formatter,
                    "wait millis {wait_millis}, active {active_count}, maxActive {max_active}, creating {creating_count}"
                )?;
                if let Some(elapsed) = create_elapsed_millis.filter(|value| *value > 0) {
                    write!(formatter, ", createElapseMillis {elapsed}")?;
                }
                if *create_error_count > 0 {
                    write!(formatter, ", createErrorCount {create_error_count}")?;
                }
                for (index, (running_count, sql)) in running_sql.iter().enumerate() {
                    if index == 0 {
                        write!(formatter, ", ")?;
                    } else {
                        writeln!(formatter)?;
                    }
                    write!(formatter, "runningSqlCount {running_count} : {sql}")?;
                }
                Ok(())
            }
            Self::LoginTimeout => write!(formatter, "physical connection login timed out"),
            Self::DataSourceDisabled => write!(formatter, "datasource is disabled"),
            Self::DataSourceClosed { close_time_millis } => {
                use chrono::{Local, TimeZone};
                write!(formatter, "dataSource already closed at ")?;
                match i64::try_from(*close_time_millis)
                    .ok()
                    .and_then(|millis| Local.timestamp_millis_opt(millis).single())
                {
                    Some(time) => write!(formatter, "{}", time.format("%a %b %d %H:%M:%S %Z %Y")),
                    None => write!(formatter, "{close_time_millis}"),
                }
            }
            Self::PoolExhausted => write!(formatter, "connection pool exhausted"),
            Self::DataSourceNotFound(name) => write!(formatter, "datasource not found: {name}"),
            Self::DataSourceNotAvailable { cause: Some(cause) } => write!(formatter, "{cause}"),
            Self::DataSourceNotAvailable { cause: None } => {
                write!(formatter, "datasource is not available")
            }
            Self::MaxWaitThreadCountExceeded { max, current } => write!(
                formatter,
                "maxWaitThreadCount {max}, current wait task count {current}"
            ),
            Self::ConnectionDiscarded => write!(formatter, "connection has been discarded"),
            Self::ConnectionLeaked { id, held_for } => {
                write!(formatter, "connection {id} leaked, held for {held_for:?}")
            }
            Self::ValidationFailed(message) => {
                write!(formatter, "connection validation failed: {message}")
            }
            Self::DriverError(message) => write!(formatter, "driver error: {message}"),
            Self::SqlException(exception) => write!(formatter, "{exception}"),
            Self::BatchUpdateException {
                update_counts,
                cause,
            } => write!(
                formatter,
                "batch update failed after {} result(s): {cause}",
                update_counts.large().len()
            ),
            Self::InvalidArgument(message) => write!(formatter, "invalid argument: {message}"),
            Self::SqlParseError(message) => write!(formatter, "SQL parse error: {message}"),
            Self::WallViolation(message) => write!(formatter, "wall violation: {message}"),
            Self::NoDriverExtension { db_type } => write!(
                formatter,
                "no driver extension registered for database type: {db_type}"
            ),
            Self::UnsupportedOperation { operation } => {
                write!(formatter, "operation is not supported: {operation}")
            }
            Self::ProductBoundary(boundary) => write!(formatter, "{}", boundary.message()),
            Self::Other(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for RdbcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SqlException(exception) => Some(exception.as_ref()),
            Self::BatchUpdateException { cause, .. } => Some(cause.as_ref()),
            Self::GetConnectionTimeout {
                cause: Some(cause), ..
            }
            | Self::DataSourceNotAvailable {
                cause: Some(cause), ..
            } => Some(cause.as_ref()),
            Self::ProductBoundary(boundary) => boundary.source_error().map(|source| source as _),
            _ => None,
        }
    }
}

impl From<String> for RdbcError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

impl From<&str> for RdbcError {
    fn from(value: &str) -> Self {
        Self::Other(value.to_owned())
    }
}
