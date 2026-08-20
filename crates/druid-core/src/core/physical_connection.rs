//! druid-rust 内部物理连接 SPI。

use super::error::DruidError;
use super::exec_result::ExecResult;
use super::physical_connection_capabilities::PhysicalConnectionCapabilities;
use super::physical_database_meta_data::PhysicalDatabaseMetaData;
use super::physical_prepared_statement::PhysicalPreparedStatement;
use super::physical_statement::{
    PhysicalStatement, PhysicalStatementOptions, SqlTextStatement, StatementExecuteResult,
    StatementGeneratedKeys,
};
use super::prepared_input_parameter::PreparedInputParameter;
use super::prepared_statement_key::PreparedStatementKey;
use super::rdbc_result_set::{PhysicalResultSet, RowSetResultSet};
use super::row::Row;
use super::savepoint::Savepoint;
use super::sql_warning::SqlWarning;
use super::value::Value;
use super::{RdbcBlob, RdbcClob, RdbcNClob};
use std::any::Any;
use std::sync::Arc;

/// druid-rust 内部最小物理连接 SPI。
///
/// 对应 Java 平台依赖: `java.sql.Connection`。它不是 Druid 领域对象，
/// 仅作为 `DruidPooledConnection` 与 SQLx、RBDC 等驱动之间的稳定边界。
/// 所有驱动 Adapter 必须实现本 trait，Adapter 不得再次持有连接池。
#[async_trait::async_trait]
pub trait PhysicalConnection: Any + Send {
    /// 创建普通物理语句。
    ///
    /// 对应 Java：`Connection#createStatement(...)`。动态 SQL 驱动可使用默认
    /// 状态对象并在当前连接上执行；具有原生 Statement handle 的 Adapter 可覆盖。
    async fn create_physical_statement(
        &mut self,
        options: PhysicalStatementOptions,
    ) -> Result<Arc<dyn PhysicalStatement>, DruidError> {
        Ok(Arc::new(SqlTextStatement::new(options)))
    }

    /// 执行更新类 SQL。
    ///
    /// 参数 `sql` 为 SQL 文本，`params` 为绑定参数；返回执行结果。
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError>;

    /// 执行 RDBC `Statement#execute(...)` 并返回有序结果。
    ///
    /// 该入口不能根据 SQL 前缀猜测结果类型。支持 generic execute 的 Adapter
    /// 必须让驱动执行并报告真实结果；多结果驱动按 RDBC 顺序返回全部结果。
    async fn execute(
        &mut self,
        _sql: &str,
        _params: Vec<Value>,
        _generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "statement_execute",
        })
    }

    /// 执行一个 RDBC 更新批次并返回每项更新计数。
    ///
    /// 对应 Java：`Statement#executeBatch()`。默认实现按驱动连接顺序执行；
    /// Adapter 若有原生 batch 能力应覆盖本方法。失败时返回
    /// `BatchUpdateException` 并保留失败前已经得到的计数。
    async fn exec_batch(
        &mut self,
        batch: Vec<(String, Vec<Value>)>,
    ) -> Result<Vec<i32>, DruidError> {
        let mut update_counts = Vec::with_capacity(batch.len());
        for (sql, params) in batch {
            match self.exec(&sql, params).await {
                Ok(result) => {
                    update_counts.push(i32::try_from(result.rows_affected).unwrap_or(i32::MAX));
                }
                Err(error) => {
                    return Err(DruidError::BatchUpdateException {
                        update_counts,
                        cause: Box::new(error),
                    });
                }
            }
        }
        Ok(update_counts)
    }

    /// 执行查询类 SQL。
    ///
    /// 参数 `sql` 为 SQL 文本，`params` 为绑定参数；返回结果行。
    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError>;

    /// 执行查询并返回物理 `ResultSet` 对象。
    ///
    /// 对应 Java：`Statement#executeQuery(String)` 返回驱动 `ResultSet`，池化层
    /// 必须保留其标签、metadata、getter 重载与关闭身份。仅提供 eager 行集合的
    /// Adapter 可使用默认回退；真实驱动应覆盖本方法。
    async fn fetch_result_set(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let rows = self.fetch(sql, params).await?;
        Ok(Arc::new(RowSetResultSet::new(rows)))
    }

    /// 按完整 RDBC 重载键创建物理预编译语句。
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

    /// 使用完整 RDBC setter 描述符执行已经 prepare 的更新语句。
    ///
    /// 默认实现只接受可无损投影为 `Value` 的标量参数。LOB、Stream、Reader
    /// 及其他资源必须由具体 Adapter 覆盖本入口，并在这里读取资源；池化层
    /// 不得提前物化。
    async fn exec_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<ExecResult, DruidError> {
        let params = parameters
            .iter()
            .map(PreparedInputParameter::scalar_value)
            .collect::<Result<Vec<_>, _>>()?;
        self.exec_prepared(statement, params).await
    }

    /// generic execute 已经 prepare 的语句，并保留 query/update 结果类型。
    ///
    /// 对应 Java：`PreparedStatement#execute()`。默认实现把预编译 SQL、参数
    /// 快照及 prepare 时保存的 generated-keys 重载交给同一驱动 generic
    /// execute 边界；`Adapter` 不得按 SQL 文本前缀猜测结果类型。
    async fn execute_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        if statement.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }
        self.execute(statement.sql(), params, generated_keys).await
    }

    /// 使用完整 RDBC setter 描述符执行 generic PreparedStatement。
    async fn execute_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let params = parameters
            .iter()
            .map(PreparedInputParameter::scalar_value)
            .collect::<Result<Vec<_>, _>>()?;
        self.execute_prepared(statement, params, generated_keys)
            .await
    }

    /// 执行已经 prepare 的参数批次。
    ///
    /// 对应 Java：`PreparedStatement#executeBatch()`。默认实现保持参数快照顺序，
    /// 逐次调用同一物理 PreparedStatement；Adapter 可覆盖为原生批处理。任一项
    /// 失败时返回携带已完成更新计数的 `BatchUpdateException`。
    async fn exec_prepared_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: Vec<Vec<Value>>,
    ) -> Result<Vec<i32>, DruidError> {
        if statement.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }

        let mut update_counts = Vec::with_capacity(parameter_sets.len());
        for params in parameter_sets {
            match self.exec_prepared(statement, params).await {
                Ok(result) => {
                    update_counts.push(i32::try_from(result.rows_affected).unwrap_or(i32::MAX));
                }
                Err(error) => {
                    return Err(DruidError::BatchUpdateException {
                        update_counts,
                        cause: Box::new(error),
                    });
                }
            }
        }
        Ok(update_counts)
    }

    /// 使用完整 RDBC setter 描述符执行 PreparedStatement 参数批次。
    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i32>, DruidError> {
        let parameter_sets = parameter_sets
            .iter()
            .map(|parameters| {
                parameters
                    .iter()
                    .map(PreparedInputParameter::scalar_value)
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.exec_prepared_batch(statement, parameter_sets).await
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

    /// 使用完整 RDBC setter 描述符执行已经 prepare 的查询。
    async fn fetch_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Vec<Row>, DruidError> {
        let params = parameters
            .iter()
            .map(PreparedInputParameter::scalar_value)
            .collect::<Result<Vec<_>, _>>()?;
        self.fetch_prepared(statement, params).await
    }

    /// 执行已经 prepare 的查询并保留驱动级 `ResultSet` 语义。
    async fn fetch_prepared_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let rows = self.fetch_prepared(statement, params).await?;
        Ok(Arc::new(RowSetResultSet::new(rows)))
    }

    /// 使用完整 RDBC setter 描述符执行查询并保留驱动级 `ResultSet` 语义。
    async fn fetch_prepared_parameters_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let params = parameters
            .iter()
            .map(PreparedInputParameter::scalar_value)
            .collect::<Result<Vec<_>, _>>()?;
        self.fetch_prepared_result_set(statement, params).await
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

    /// 创建驱动拥有的 Blob 句柄。
    ///
    /// 对应 Java：`Connection#createBlob()`。默认实现明确报告驱动能力缺失；
    /// Adapter 不得用内存 `Vec<u8>` 冒充数据库 LOB。
    async fn create_blob(&mut self) -> Result<RdbcBlob, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "connection_create_blob",
        })
    }

    /// 创建驱动拥有的 Clob 句柄。
    ///
    /// 对应 Java：`Connection#createClob()`。返回 raw 句柄，由 Druid 连接
    /// FilterChain 在池化边界包装为 `ClobProxyImpl`。
    async fn create_clob(&mut self) -> Result<RdbcClob, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "connection_create_clob",
        })
    }

    /// 创建驱动拥有的 NClob 句柄。
    ///
    /// 对应 Java：`Connection#createNClob()`。NClob 必须保持独立类型身份，
    /// 不能降级成普通 Clob。
    async fn create_n_clob(&mut self) -> Result<RdbcNClob, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "connection_create_n_clob",
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

    /// 创建借用当前物理连接的数据库 metadata SPI。
    ///
    /// 对应 Java：`Connection#getMetaData()`。返回对象的生命周期受当前可变
    /// 连接借用约束，不会复制连接或创建嵌套池。Adapter 必须返回真实实现；
    /// 未支持时保持明确 capability error。
    fn database_meta_data(&mut self) -> Result<Box<dyn PhysicalDatabaseMetaData + '_>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "connection_get_meta_data",
        })
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

    /// 返回连接上的 `SQLWarning` 链。
    ///
    /// 对应 Java：`Connection#getWarnings()`。不支持的 Adapter 必须明确返回
    /// unsupported，不能用 `None` 冒充驱动没有警告。
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "connection_get_warnings",
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
