use super::PooledConnectionSource;
use crate::error::SqlException;
use crate::statement::PreparedStatement;
use std::sync::Arc;
use tokio::sync::Mutex;

/// PreparedStatement 的共享所有权句柄，保留事件中的原语句身份。
/// 对应 Java: javax.sql.StatementEvent#getStatement。
pub type StatementSource = Arc<Mutex<Box<dyn PreparedStatement>>>;

/// PreparedStatement 生命周期事件，保留来源、语句和可选异常的引用身份。
/// 对应 Java: javax.sql.StatementEvent。
#[derive(Clone)]
pub struct StatementEvent {
    source: PooledConnectionSource,
    statement: Option<StatementSource>,
    exception: Option<Arc<SqlException>>,
}

impl StatementEvent {
    /// 创建无异常语句事件。来源非空；statement 为 None 时保留 Java 的 null 语义。
    /// 对应 Java: StatementEvent#StatementEvent(PooledConnection, PreparedStatement)。
    pub fn new(source: PooledConnectionSource, statement: Option<StatementSource>) -> Self {
        Self::with_sql_exception(source, statement, None)
    }

    /// 创建含可选异常的事件，返回持有原来源、语句及异常共享引用的事件。
    /// 对应 Java: StatementEvent#StatementEvent(PooledConnection, PreparedStatement, SQLException)。
    pub fn with_sql_exception(
        source: PooledConnectionSource,
        statement: Option<StatementSource>,
        exception: Option<Arc<SqlException>>,
    ) -> Self {
        Self {
            source,
            statement,
            exception,
        }
    }

    /// 返回原池化连接共享句柄引用；无参数，不重新创建连接。
    /// 对应 Java: java.util.EventObject#getSource。
    pub fn source(&self) -> &PooledConnectionSource {
        &self.source
    }

    /// 返回原 PreparedStatement 共享句柄引用，Java null 返回 None。
    /// 对应 Java: StatementEvent#getStatement。
    pub fn statement(&self) -> Option<&StatementSource> {
        self.statement.as_ref()
    }

    /// 返回原 SQL 异常共享引用，Java null 返回 None。
    /// 对应 Java: StatementEvent#getSQLException。
    pub fn sql_exception(&self) -> Option<&Arc<SqlException>> {
        self.exception.as_ref()
    }
}

impl std::fmt::Debug for StatementEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StatementEvent")
            .field("source", &Arc::as_ptr(&self.source))
            .field("statement", &self.statement.as_ref().map(Arc::as_ptr))
            .field("exception", &self.exception)
            .finish()
    }
}
