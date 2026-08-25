use crate::error::RdbcError;
use crate::error::SqlWarning;
use crate::extensions::ConnectionCapabilities;
use crate::metadata::DatabaseMetaData;
use crate::statement::{CallableStatement, PreparedStatement, Statement, StatementOptions};
use crate::types::Savepoint;
use crate::wrapper::Wrapper;

/// JDBC `Connection` 的唯一 standard trait。
///
/// 对应 Java：`java.sql.Connection`。所有 mutable operations 使用 exclusive `&mut self`；
/// 创建的 statement 与 live metadata 拥有独立 actor/native handle，不借用 caller。
#[async_trait::async_trait]
pub trait Connection: Wrapper + Send + 'static {
    /// 创建 owned statement。
    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError>;

    /// 创建 owned prepared statement。
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_prepare_statement",
        })
    }

    /// 创建 owned callable statement。
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_prepare_call",
        })
    }

    /// 返回 owned live database metadata object。
    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_database_meta_data",
        })
    }

    /// 提交当前 transaction。
    async fn commit(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_commit",
        })
    }

    /// 回滚当前 transaction。
    async fn rollback(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_rollback",
        })
    }

    /// 回滚到保存点。
    async fn rollback_to(&mut self, _savepoint: &Savepoint) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_rollback_to",
        })
    }

    /// 设置 auto-commit 状态。
    async fn set_auto_commit(&mut self, _enabled: bool) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_auto_commit",
        })
    }

    /// 返回 auto-commit 状态。
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_auto_commit",
        })
    }

    /// 返回当前只读状态。
    fn read_only(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_read_only",
        })
    }

    /// 设置当前只读状态。
    async fn set_read_only(&mut self, _read_only: bool) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_read_only",
        })
    }

    /// 返回当前 catalog；`None` 保留 JDBC `null` 语义。
    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_catalog",
        })
    }

    /// 设置当前 catalog；`None` 保留 JDBC `null` 语义。
    async fn set_catalog(&mut self, _catalog: Option<&str>) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_catalog",
        })
    }

    /// 返回当前 schema；`None` 保留 JDBC `null` 语义。
    fn schema(&self) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_schema",
        })
    }

    /// 设置当前 schema；`None` 保留 JDBC `null` 语义。
    async fn set_schema(&mut self, _schema: Option<&str>) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_schema",
        })
    }

    /// 返回当前 JDBC warning 链。
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_warnings",
        })
    }

    /// 清理当前 JDBC warning 链。
    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_clear_warnings",
        })
    }

    /// 返回 immutable capability descriptor。
    fn capabilities(&self) -> ConnectionCapabilities {
        ConnectionCapabilities::default()
    }

    /// 以 bounded timeout 验证连接。
    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_is_valid",
        })
    }

    /// 关闭 raw 或 logical connection handle。
    async fn close(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("connection_close"))
    }

    /// 返回 connection 是否已经关闭。
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::feature_not_supported("connection_is_closed"))
    }
}
