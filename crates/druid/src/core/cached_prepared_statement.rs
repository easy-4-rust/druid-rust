use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{CallableStatement, PreparedStatement};

/// 在同一连接 LRU 中保留预编译与可调用语句的真实标准类型。
/// 对应 Java: com.alibaba.druid.pool.PreparedStatementHolder。
pub(crate) enum CachedPreparedStatement {
    Prepared(Box<dyn PreparedStatement>),
    Callable(Box<dyn CallableStatement>),
}

impl CachedPreparedStatement {
    /// 关闭底层语句；保留失败句柄供连接所有者重试释放。
    pub(crate) async fn close(&mut self) -> Result<(), RdbcError> {
        match self {
            Self::Prepared(statement) => statement.close().await,
            Self::Callable(statement) => statement.close().await,
        }
    }
}

impl From<Box<dyn PreparedStatement>> for CachedPreparedStatement {
    fn from(statement: Box<dyn PreparedStatement>) -> Self {
        Self::Prepared(statement)
    }
}

impl From<Box<dyn CallableStatement>> for CachedPreparedStatement {
    fn from(statement: Box<dyn CallableStatement>) -> Self {
        Self::Callable(statement)
    }
}
