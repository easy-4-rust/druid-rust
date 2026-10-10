use super::PooledConnection;
use crate::error::SqlException;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 池化连接的共享所有权句柄；保留事件来源的真实对象身份及可变操作。
/// 对应 Java: javax.sql.ConnectionEvent#getSource 返回的 PooledConnection。
pub type PooledConnectionSource = Arc<Mutex<Box<dyn PooledConnection>>>;

/// 池化连接事件，持有实际来源和可选 SQL 异常，不以数字 ID 替代对象。
/// 对应 Java: javax.sql.ConnectionEvent。
#[derive(Clone)]
pub struct ConnectionEvent {
    source: PooledConnectionSource,
    exception: Option<Arc<SqlException>>,
}

impl ConnectionEvent {
    /// 从实际池化连接创建无异常事件。参数为非空来源句柄，返回持有该来源的事件。
    /// 对应 Java: ConnectionEvent#ConnectionEvent(PooledConnection)。
    pub fn new(source: PooledConnectionSource) -> Self {
        Self::with_sql_exception(source, None)
    }

    /// 创建可含 SQL 异常的事件。参数保留共享引用身份，返回独立持有引用的事件。
    /// 对应 Java: ConnectionEvent#ConnectionEvent(PooledConnection, SQLException)。
    pub fn with_sql_exception(
        source: PooledConnectionSource,
        exception: Option<Arc<SqlException>>,
    ) -> Self {
        Self { source, exception }
    }

    /// 返回原池化连接的共享句柄引用；无参数，不复制底层连接。
    /// 对应 Java: java.util.EventObject#getSource。
    pub fn source(&self) -> &PooledConnectionSource {
        &self.source
    }

    /// 返回原 SQL 异常的共享引用，未提供异常时返回 None。
    /// 对应 Java: ConnectionEvent#getSQLException。
    pub fn sql_exception(&self) -> Option<&Arc<SqlException>> {
        self.exception.as_ref()
    }
}

impl std::fmt::Debug for ConnectionEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectionEvent")
            .field("source", &Arc::as_ptr(&self.source))
            .field("exception", &self.exception)
            .finish()
    }
}
