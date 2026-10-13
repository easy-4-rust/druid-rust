use crate::error::{RdbcError, SqlWarning};
use crate::result_set::{ColumnIndex, ResultSet};
use crate::statement::StatementCancellation;
use crate::types::Value;
use crate::wrapper::Wrapper;
use std::sync::Arc;

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
    /// 无损保存驱动或过滤器报告的原整数，不据此推断游标能力。
    DriverValue(i32),
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
    /// 无损保存驱动或过滤器报告的原整数，不据此声明可更新能力。
    DriverValue(i32),
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
    /// 无损保存驱动返回的整数状态；不推断其游标能力。
    /// 对应 Java: java.sql.Connection#getHoldability。
    DriverValue(i32),
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
    /// 无损保存已类型化的驱动或过滤器整数；整数输入的 TryFrom 校验不变。
    DriverValue(i32),
}

impl ResultSetType {
    /// 返回结果集类型的 JDBC 整数代码；无参数，返回对应 TYPE 常量。
    /// 对应 Java: java.sql.ResultSet#TYPE_FORWARD_ONLY/TYPE_SCROLL_INSENSITIVE/TYPE_SCROLL_SENSITIVE。
    pub const fn rdbc_code(self) -> i32 {
        match self {
            Self::ForwardOnly => 1003,
            Self::ScrollInsensitive => 1004,
            Self::ScrollSensitive => 1005,
            Self::DriverValue(code) => code,
        }
    }
}

impl ResultSetConcurrency {
    /// 返回并发模式的 JDBC 整数代码；无参数，返回只读或可更新常量。
    /// 对应 Java: java.sql.ResultSet#CONCUR_READ_ONLY/CONCUR_UPDATABLE。
    pub const fn rdbc_code(self) -> i32 {
        match self {
            Self::ReadOnly => 1007,
            Self::Updatable => 1008,
            Self::DriverValue(code) => code,
        }
    }
}

impl ResultSetHoldability {
    /// 返回提交后的游标保留代码；无参数，返回 HOLD 或 CLOSE 常量。
    /// 对应 Java: java.sql.ResultSet#HOLD_CURSORS_OVER_COMMIT/CLOSE_CURSORS_AT_COMMIT。
    pub const fn rdbc_code(self) -> i32 {
        match self {
            Self::HoldOverCommit => 1,
            Self::CloseAtCommit => 2,
            Self::DriverValue(code) => code,
        }
    }
}

impl FetchDirection {
    /// 返回提取方向的 JDBC 整数代码；无参数，返回 FETCH 常量。
    /// 对应 Java: java.sql.ResultSet#FETCH_FORWARD/FETCH_REVERSE/FETCH_UNKNOWN。
    pub const fn rdbc_code(self) -> i32 {
        match self {
            Self::Forward => 1000,
            Self::Reverse => 1001,
            Self::Unknown => 1002,
            Self::DriverValue(code) => code,
        }
    }
}

impl TryFrom<i32> for FetchDirection {
    type Error = RdbcError;

    /// 校验整数方向并转换为标准描述符；参数 direction 仅接受 1000/1001/1002。
    /// 返回对应方向，其他输入返回参数错误且不修改结果集状态。
    /// 对应 Java: java.sql.ResultSet#setFetchDirection(int)。
    fn try_from(direction: i32) -> Result<Self, Self::Error> {
        match direction {
            1000 => Ok(Self::Forward),
            1001 => Ok(Self::Reverse),
            1002 => Ok(Self::Unknown),
            _ => Err(RdbcError::InvalidArgument(format!(
                "invalid fetch direction: {direction}"
            ))),
        }
    }
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

impl TryFrom<i32> for CurrentResultDisposition {
    type Error = RdbcError;

    /// 将 JDBC 整数处置常量转换为标准处置。对应 Java：Statement#getMoreResults(int)。
    /// 参数 current 仅接受 1/2/3；返回对应处置，其他值返回参数错误且无资源副作用。
    fn try_from(current: i32) -> Result<Self, Self::Error> {
        match current {
            1 => Ok(Self::CloseCurrent),
            2 => Ok(Self::KeepCurrent),
            3 => Ok(Self::CloseAll),
            _ => Err(RdbcError::InvalidArgument(format!(
                "invalid JDBC current result disposition: {current}"
            ))),
        }
    }
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

    /// 返回可独立持有的取消控制对象；默认以 SQLState `0A000` 报告不支持。
    /// 调用方应在开始执行前取得句柄，且本方法不要求 Statement 实现 `Sync`。
    /// 旧的 `&mut self` 取消入口不能与占用同一独占借用的执行 Future 同时调用：
    ///
    /// ```compile_fail
    /// use druid_rdbc::statement::Statement;
    /// fn cannot_cancel_while_execute_borrows_statement<S: Statement>(statement: &mut S) {
    ///     let execution = statement.execute_query("SELECT 1");
    ///     let _ = statement.cancel();
    ///     drop(execution);
    /// }
    /// ```
    fn cancellation_control(&self) -> Result<Arc<dyn StatementCancellation>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "statement_cancellation_control",
        ))
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

    /// 按 Java 整数处置推进结果。对应 Java：Statement#getMoreResults(int)。
    /// 参数 current 为 JDBC 处置常量；返回下个结果是否为游标，非法值返回参数错误。
    async fn get_more_results_current(&mut self, current: i32) -> Result<bool, RdbcError> {
        // 先完成整数边界校验，再交给同一 typed 实现，失败不能影响当前游标。
        let disposition = CurrentResultDisposition::try_from(current)?;
        self.get_more_results(disposition).await
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
