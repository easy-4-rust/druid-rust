use crate::error::{RdbcError, SqlWarning};
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

/// JDBC cursor fetch direction。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum FetchDirection {
    /// 驱动自行选择最有效方向。
    #[default]
    Unknown,
    /// 正向读取。
    Forward,
    /// 反向读取。
    Reverse,
}

/// 推进到下一个 statement result 前如何处置当前 ResultSet。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CurrentResultDisposition {
    /// 采用 JDBC 默认处置。
    #[default]
    Default,
    /// 关闭当前 ResultSet。
    CloseCurrent,
    /// 保留当前 ResultSet。
    KeepCurrent,
    /// 关闭所有先前 ResultSet。
    CloseAll,
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
    /// 返回创建 statement 时的 typed options。
    fn options(&self) -> Result<StatementOptions, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_options",
        })
    }

    /// 返回 max-field-size 限制；`0` 表示无限制。
    fn max_field_size(&self) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_max_field_size",
        })
    }

    /// 设置 max-field-size 限制。
    fn set_max_field_size(&mut self, _max: i32) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_set_max_field_size",
        })
    }

    /// 返回 max-rows 限制；`0` 表示无限制。
    fn max_rows(&self) -> Result<i64, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_max_rows",
        })
    }

    /// 设置 max-rows 限制。
    fn set_max_rows(&mut self, _max: i64) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_set_max_rows",
        })
    }

    /// 设置 driver escape processing。
    fn set_escape_processing(&mut self, _enabled: bool) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_set_escape_processing",
        })
    }

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

    /// 请求取消当前 execution。
    fn cancel(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_cancel",
        })
    }

    /// 设置 cursor name。
    fn set_cursor_name(&mut self, _name: &str) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_set_cursor_name",
        })
    }

    /// 设置 fetch direction hint。
    fn set_fetch_direction(&mut self, _direction: FetchDirection) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_set_fetch_direction",
        })
    }

    /// 返回 fetch direction hint。
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_fetch_direction",
        })
    }

    /// 设置 fetch-size hint。
    fn set_fetch_size(&mut self, _rows: i32) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_set_fetch_size",
        })
    }

    /// 返回 fetch-size hint。
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_fetch_size",
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

    /// 返回当前 statement batch snapshot。
    fn batch(&self) -> Result<Vec<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_batch",
        })
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

    /// JDBC-compatible generated-keys convenience name。
    async fn get_generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.generated_keys().await
    }

    /// 取走当前 execution 的 ResultSet；没有 ResultSet 时返回 `None`。
    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_get_result_set",
        })
    }

    /// 返回当前 update count；没有 update result 时返回 `None`。
    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_get_update_count",
        })
    }

    /// 推进到下一个 execution result。
    async fn get_more_results(
        &mut self,
        _disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_get_more_results",
        })
    }

    /// 关闭 statement handle。
    async fn close(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("statement_close"))
    }

    /// 返回 statement 是否关闭。
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::feature_not_supported("statement_is_closed"))
    }

    /// 返回 statement 是否允许池化。
    fn is_poolable(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_is_poolable",
        })
    }

    /// 设置 statement poolable hint。
    fn set_poolable(&mut self, _poolable: bool) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_set_poolable",
        })
    }

    /// 标记所有 dependent ResultSet 关闭后自动关闭 statement。
    fn close_on_completion(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_close_on_completion",
        })
    }

    /// 返回 close-on-completion 状态。
    fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_is_close_on_completion",
        })
    }

    /// 返回 statement warning chain。
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_warnings",
        })
    }

    /// 返回当前 statement warnings 的标准值视图。
    fn warning_messages(&self) -> Result<Vec<String>, RdbcError> {
        let mut messages = Vec::new();
        let mut warning = self.warnings()?;
        while let Some(current) = warning {
            messages.push(current.message().to_owned());
            warning = current.next_warning().cloned();
        }
        Ok(messages)
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
