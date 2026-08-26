use crate::error::RdbcError;
use crate::result_set::{ColumnIndex, ResultSet};
use crate::types::Value;
use crate::wrapper::Wrapper;

/// ResultSet cursor type descriptor。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultSetType {
    /// Forward-only cursor。
    #[default]
    ForwardOnly,
    /// Scroll-insensitive cursor。
    ScrollInsensitive,
    /// Scroll-sensitive cursor。
    ScrollSensitive,
}

/// ResultSet concurrency descriptor。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultSetConcurrency {
    /// Read-only cursor。
    #[default]
    ReadOnly,
    /// Updatable cursor。
    Updatable,
}

/// ResultSet transaction-boundary holdability descriptor。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultSetHoldability {
    /// Commit 时关闭 cursor。
    #[default]
    CloseAtCommit,
    /// Commit 后保留 cursor。
    HoldOverCommit,
}

/// `Connection#createStatement` 的 typed options。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatementOptions {
    result_set_type: ResultSetType,
    concurrency: ResultSetConcurrency,
    holdability: ResultSetHoldability,
}

impl StatementOptions {
    /// 创建 statement options。
    pub const fn new(
        result_set_type: ResultSetType,
        concurrency: ResultSetConcurrency,
        holdability: ResultSetHoldability,
    ) -> Self {
        Self {
            result_set_type,
            concurrency,
            holdability,
        }
    }

    /// 返回 result-set type。
    pub const fn result_set_type(self) -> ResultSetType {
        self.result_set_type
    }

    /// 返回 concurrency。
    pub const fn concurrency(self) -> ResultSetConcurrency {
        self.concurrency
    }

    /// 返回 holdability。
    pub const fn holdability(self) -> ResultSetHoldability {
        self.holdability
    }
}

/// Generated keys request descriptor。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum GeneratedKeys {
    /// 不请求 generated keys。
    #[default]
    None,
    /// 请求驱动默认 generated keys。
    ReturnGeneratedKeys,
    /// 按 1-based column indexes 请求。
    ColumnIndexes(Vec<ColumnIndex>),
    /// 按 column names 请求。
    ColumnNames(Vec<String>),
}

/// 一个 update 执行摘要。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExecResult {
    /// 受影响行数。
    pub rows_affected: u64,
    /// 最后插入的 ID；驱动未返回时保持 `None`。
    pub last_insert_id: Option<i64>,
    /// 查询返回行数；流式或非查询结果保持 `None`。
    pub row_count: Option<u64>,
}

/// `Statement#execute` 的有序结果。
pub enum ExecutionResult {
    /// Owned result-set handle。
    ResultSet(Box<dyn ResultSet>),
    /// Update count；`-1` 等 JDBC sentinel 保持有符号值。
    UpdateCount(i64),
}

/// Canonical JDBC-like name for one generic execution result。
pub type StatementExecuteResult = ExecutionResult;

impl std::fmt::Debug for ExecutionResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResultSet(_) => formatter.write_str("ExecutionResult::ResultSet(..)"),
            Self::UpdateCount(count) => formatter
                .debug_tuple("ExecutionResult::UpdateCount")
                .field(count)
                .finish(),
        }
    }
}

/// JDBC `Statement` 的 owned executor/native-handle 标准。
///
/// 实现不得借用创建它的 connection；query 返回唯一 owned [`ResultSet`]。
#[async_trait::async_trait]
pub trait Statement: Wrapper + Send + 'static {
    /// 返回当前query timeout秒数。
    fn query_timeout(&self) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_query_timeout",
        })
    }

    /// 设置query timeout秒数。
    fn set_query_timeout(&mut self, _seconds: i32) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_set_query_timeout",
        })
    }

    /// 执行 query 并返回 owned result set。
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_execute_query",
        })
    }

    /// 执行 update 并返回 64-bit count。
    async fn execute_update(
        &mut self,
        _sql: &str,
        _generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_execute_update",
        })
    }

    /// 执行 generic statement 并保留结果种类。
    async fn execute(
        &mut self,
        _sql: &str,
        _generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_execute",
        })
    }

    /// 加入一条 SQL batch entry。
    fn add_batch(&mut self, _sql: String) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_add_batch",
        })
    }

    /// 清空当前 batch entries。
    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    /// 执行 batch 并返回 JDBC long-count shape。
    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_execute_batch",
        })
    }

    /// 返回本 statement 最近一次执行产生的 owned generated-keys result set。
    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported("statement_generated_keys"))
    }

    /// 关闭 statement handle。
    async fn close(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("statement_close"))
    }

    /// 返回 statement 是否关闭。
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::feature_not_supported("statement_is_closed"))
    }

    /// 返回当前 statement warnings 的标准值视图。
    fn warning_messages(&self) -> Result<Vec<String>, RdbcError> {
        Ok(Vec::new())
    }

    /// 清理 statement warnings。
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    /// 兼容 eager scalar adapters 的 typed parameter helper。
    async fn execute_values(
        &mut self,
        sql: &str,
        _values: Vec<Value>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.execute_query(sql).await
    }
}
