//! druid-rust 内部物理连接 SPI。

use crate::error::DruidError;
use crate::exec_result::ExecResult;
use crate::physical_connection_capabilities::PhysicalConnectionCapabilities;
use crate::physical_prepared_statement::PhysicalPreparedStatement;
use crate::prepared_statement_key::PreparedStatementKey;
use crate::row::Row;
use crate::savepoint::Savepoint;
use crate::value::Value;
use std::sync::Arc;

/// druid-rust 内部最小物理连接 SPI。
///
/// 对应 Java 平台依赖: `java.sql.Connection`。它不是 Druid 领域对象，
/// 仅作为 `DruidPooledConnection` 与 SQLx、RBDC 等驱动之间的稳定边界。
/// 所有驱动 Adapter 必须实现本 trait，Adapter 不得再次持有连接池。
#[async_trait::async_trait]
pub trait PhysicalConnection: Send {
    /// 执行更新类 SQL。
    ///
    /// 参数 `sql` 为 SQL 文本，`params` 为绑定参数；返回执行结果。
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError>;

    /// 执行查询类 SQL。
    ///
    /// 参数 `sql` 为 SQL 文本，`params` 为绑定参数；返回结果行。
    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError>;

    /// 按完整 JDBC 重载键创建物理预编译语句。
    ///
    /// 对应 Java：`Connection#prepareStatement(...)` 和 `prepareCall(...)`。
    /// 不支持的 Adapter 必须返回明确错误。
    async fn prepare_physical_statement(
        &mut self,
        _key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepare_physical_statement",
        })
    }

    /// 按完整 `prepareCall` 缓存键创建物理 CallableStatement。
    ///
    /// 不支持存储过程调用的 Adapter 必须返回明确错误。
    async fn prepare_physical_call(
        &mut self,
        _key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepare_physical_call",
        })
    }

    /// 执行已经 prepare 的更新类语句。
    ///
    /// 默认实现把 statement 的 SQL 交回驱动执行入口；这适用于 RBDC 等在
    /// `exec` 内部完成 prepare/cache 的驱动。持有独立 server handle 的 Adapter
    /// 可以覆盖本方法。
    async fn exec_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        if statement.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }
        self.exec(statement.sql(), params).await
    }

    /// 执行已经 prepare 的查询类语句。
    async fn fetch_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        if statement.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }
        self.fetch(statement.sql(), params).await
    }

    /// 关闭物理预编译语句。
    async fn close_prepared_statement(
        &mut self,
        statement: Arc<dyn PhysicalPreparedStatement>,
    ) -> Result<(), DruidError> {
        statement.close()
    }

    /// 开始事务。
    async fn begin(&mut self) -> Result<(), DruidError>;

    /// 提交事务。
    async fn commit(&mut self) -> Result<(), DruidError>;

    /// 回滚事务。
    async fn rollback(&mut self) -> Result<(), DruidError>;

    /// 回滚到指定保存点。
    async fn rollback_to(&mut self, _savepoint: &Savepoint) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "rollback_to",
        })
    }

    /// 创建匿名保存点。
    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_savepoint",
        })
    }

    /// 创建命名保存点。
    async fn set_savepoint_named(&mut self, _name: &str) -> Result<Savepoint, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_savepoint_named",
        })
    }

    /// 释放指定保存点。
    async fn release_savepoint(&mut self, _savepoint: &Savepoint) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "release_savepoint",
        })
    }

    /// 强制中止物理连接。
    async fn abort(&mut self) -> Result<(), DruidError> {
        self.close().await
    }

    /// 验证物理连接是否存活。
    async fn ping(&mut self) -> Result<(), DruidError>;

    /// 关闭物理连接。
    async fn close(&mut self) -> Result<(), DruidError>;

    /// 返回物理连接是否已关闭。
    fn is_closed(&self) -> bool {
        false
    }

    /// 返回 Adapter 明确支持的能力。
    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        PhysicalConnectionCapabilities::default()
    }

    /// 返回自动提交状态。
    fn auto_commit(&self) -> bool {
        true
    }

    /// 设置自动提交状态。
    async fn set_auto_commit(&mut self, _auto_commit: bool) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_auto_commit",
        })
    }

    /// 返回只读状态。
    fn read_only(&self) -> bool {
        false
    }

    /// 设置只读状态。
    async fn set_read_only(&mut self, _read_only: bool) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_read_only",
        })
    }

    /// 返回事务隔离级别。
    fn transaction_isolation(&self) -> u8 {
        2
    }

    /// 设置事务隔离级别。
    async fn set_transaction_isolation(&mut self, _level: u8) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_transaction_isolation",
        })
    }

    /// 返回 `ResultSet` 保持性。
    fn holdability(&self) -> i32 {
        0
    }

    /// 设置 `ResultSet` 保持性。
    async fn set_holdability(&mut self, _holdability: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_holdability",
        })
    }

    /// 清理连接上的 `SQLWarning`。
    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "clear_warnings",
        })
    }

    /// 标记连接不得重新进入外部池。
    ///
    /// Native Pool 通过回收处置枚举直接丢弃连接；bb8/deadpool 等外部池
    /// Adapter 必须覆盖该方法，使其 manager 能在同步 Drop 路径识别脏连接。
    fn mark_discarded(&mut self) {}

    /// 返回连接是否已被标记为不得复用。
    fn is_discarded(&self) -> bool {
        false
    }

    /// 返回 catalog。
    fn catalog(&self) -> Option<&str> {
        None
    }

    /// 设置 catalog。
    async fn set_catalog(&mut self, _catalog: &str) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_catalog",
        })
    }

    /// 返回 schema。
    fn schema(&self) -> Option<&str> {
        None
    }

    /// 设置 schema。
    async fn set_schema(&mut self, _schema: &str) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_schema",
        })
    }

    /// 返回驱动名称。
    fn driver_name(&self) -> &str {
        ""
    }
}
