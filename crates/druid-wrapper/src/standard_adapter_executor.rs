//! Wrapper-private owned executor shared by canonical adapter handles.

use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Driver update outcome retained inside wrapper adapters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AdapterUpdateResult {
    pub(crate) rows_affected: u64,
    pub(crate) last_insert_id: Option<i64>,
    pub(crate) row_count: Option<u64>,
}

/// Concrete adapters expose one private driver core to canonical callers.
#[async_trait::async_trait]
pub(crate) trait AdapterExecutionCore: Send + 'static {
    async fn standard_execute_update_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<i64, RdbcError>;

    async fn standard_execute_query_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError>;

    async fn standard_statement_returns_rows_core(&mut self, sql: &str) -> Result<bool, RdbcError> {
        Ok(statement_returns_rows(sql))
    }

    async fn standard_execute_update_result_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        let rows_affected = self.standard_execute_update_core(sql, values).await?;
        Ok(AdapterUpdateResult {
            rows_affected: u64::try_from(rows_affected).map_err(|_| {
                RdbcError::InvalidArgument(
                    "negative update count cannot be represented by AdapterUpdateResult".to_owned(),
                )
            })?,
            last_insert_id: None,
            row_count: None,
        })
    }

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError>;

    /// 参数为SQL及值；默认保留驱动单键，支持完整键的驱动覆写此执行通道。
    async fn standard_execute_update_keys_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<(i64, Vec<i64>), RdbcError> {
        let result = self
            .standard_execute_update_result_core(sql, values)
            .await?;
        let count = i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".into())
        })?;
        Ok((count, result.last_insert_id.into_iter().collect()))
    }

    async fn standard_parameter_count_core(
        &mut self,
        sql: &str,
    ) -> Result<Option<usize>, RdbcError> {
        Ok(Some(parameter_count(sql)))
    }
}

/// Type-erased executor owned by canonical Statement/Prepared/ResultSet handles.
#[async_trait::async_trait]
pub(crate) trait StandardExecutor: Send + Sync {
    /// 参数为SQL及值；返回更新计数与当前执行的完整生成键，不提前发布句柄状态。
    async fn execute_update_keys(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<(i64, Vec<i64>), RdbcError>;
    async fn execute_update(&self, sql: &str, values: Vec<Value>) -> Result<i64, RdbcError>;
    async fn execute_update_result(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError>;
    async fn execute_query(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError>;
    async fn statement_returns_rows(&self, sql: &str) -> Result<bool, RdbcError>;
    async fn parameter_count(&self, sql: &str) -> Result<Option<usize>, RdbcError>;
    async fn close(&self) -> Result<(), RdbcError>;
    fn is_closed(&self) -> bool;

    /// 中断当前驱动执行；不支持中断的适配器保持原有的成功空操作语义。
    fn cancel_running(&self) -> Result<(), RdbcError> {
        Ok(())
    }

    #[track_caller]
    fn task4_trace_statement_create(&self);
    #[track_caller]
    fn task4_trace_statement_update(&self);
    #[track_caller]
    fn task4_trace_prepared_create(&self);
    #[track_caller]
    fn task4_trace_parameter_set(&self);
    #[track_caller]
    fn task4_trace_prepared_query(&self);
    #[track_caller]
    fn task4_trace_result_next(&self);
    #[track_caller]
    fn task4_trace_result_set_metadata(&self);
    #[track_caller]
    fn task4_trace_database_metadata(&self);

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }
}

struct OwnedAdapterExecutor<C> {
    connection: Mutex<Option<C>>,
    closed: AtomicBool,
    runtime_trace: Option<RuntimeTraceEmitter>,
    cancel: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl<C> OwnedAdapterExecutor<C> {
    fn new(
        connection: C,
        runtime_trace: Option<RuntimeTraceEmitter>,
        cancel: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Self {
        Self {
            connection: Mutex::new(Some(connection)),
            closed: AtomicBool::new(false),
            runtime_trace,
            cancel,
        }
    }
}

#[async_trait::async_trait]
impl<C> StandardExecutor for OwnedAdapterExecutor<C>
where
    C: AdapterExecutionCore,
{
    async fn execute_update_keys(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<(i64, Vec<i64>), RdbcError> {
        let mut guard = self.connection.lock().await;
        let connection = guard.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        connection
            .standard_execute_update_keys_core(sql, values)
            .await
    }
    async fn execute_update(&self, sql: &str, values: Vec<Value>) -> Result<i64, RdbcError> {
        let result = self.execute_update_result(sql, values).await?;
        i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".to_owned())
        })
    }

    async fn execute_update_result(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        let mut guard = self.connection.lock().await;
        let connection = guard.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        connection
            .standard_execute_update_result_core(sql, values)
            .await
    }

    async fn execute_query(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError> {
        let mut guard = self.connection.lock().await;
        let connection = guard.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        connection.standard_execute_query_core(sql, values).await
    }

    async fn statement_returns_rows(&self, sql: &str) -> Result<bool, RdbcError> {
        let mut guard = self.connection.lock().await;
        let connection = guard.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        connection.standard_statement_returns_rows_core(sql).await
    }

    async fn parameter_count(&self, sql: &str) -> Result<Option<usize>, RdbcError> {
        let mut guard = self.connection.lock().await;
        let connection = guard.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        connection.standard_parameter_count_core(sql).await
    }

    async fn close(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut guard = self.connection.lock().await;
        let Some(connection) = guard.as_mut() else {
            self.closed.store(true, Ordering::Release);
            return Ok(());
        };
        connection.standard_close_connection_core().await?;
        guard.take();
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_connection_close();
        }
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn cancel_running(&self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        if let Some(cancel) = &self.cancel {
            cancel();
        }
        Ok(())
    }

    #[track_caller]
    fn task4_trace_statement_create(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_statement_create();
        }
    }

    #[track_caller]
    fn task4_trace_statement_update(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_statement_update();
        }
    }

    #[track_caller]
    fn task4_trace_prepared_create(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_prepared_create();
        }
    }

    #[track_caller]
    fn task4_trace_parameter_set(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_parameter_set();
        }
    }

    #[track_caller]
    fn task4_trace_prepared_query(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_prepared_query();
        }
    }

    #[track_caller]
    fn task4_trace_result_next(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_result_next();
        }
    }

    #[track_caller]
    fn task4_trace_result_set_metadata(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_result_set_metadata();
        }
    }

    #[track_caller]
    fn task4_trace_database_metadata(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_database_metadata();
        }
    }
}

/// 根据已有首关键字规则判断普通语句是否返回行，供驱动专有 AST 判断回退使用。
pub(crate) fn statement_returns_rows(sql: &str) -> bool {
    let sql = sql.trim_start();
    let keyword_end = sql
        .find(|character: char| !character.is_ascii_alphabetic())
        .unwrap_or(sql.len());
    matches!(
        sql[..keyword_end].to_ascii_uppercase().as_str(),
        "SELECT" | "WITH" | "VALUES" | "PRAGMA" | "SHOW" | "DESCRIBE" | "DESC" | "EXPLAIN"
    )
}

fn parameter_count(sql: &str) -> usize {
    let bytes = sql.as_bytes();
    let mut index = 0;
    let mut anonymous = 0_usize;
    let mut numbered = 0_usize;
    let mut single_quoted = false;
    let mut double_quoted = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' if !double_quoted => {
                if single_quoted && bytes.get(index + 1) == Some(&b'\'') {
                    index += 2;
                    continue;
                }
                single_quoted = !single_quoted;
            }
            b'"' if !single_quoted => {
                if double_quoted && bytes.get(index + 1) == Some(&b'"') {
                    index += 2;
                    continue;
                }
                double_quoted = !double_quoted;
            }
            b'?' if !single_quoted && !double_quoted => {
                let mut cursor = index + 1;
                while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                    cursor += 1;
                }
                if cursor == index + 1 {
                    anonymous = anonymous.saturating_add(1);
                } else if let Ok(value) = sql[index + 1..cursor].parse::<usize>() {
                    numbered = numbered.max(value);
                }
                index = cursor;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    anonymous.max(numbered)
}

pub(crate) fn owned_executor<C>(
    connection: C,
    runtime_trace: Option<RuntimeTraceEmitter>,
) -> Arc<dyn StandardExecutor>
where
    C: AdapterExecutionCore,
{
    Arc::new(OwnedAdapterExecutor::new(connection, runtime_trace, None))
}

/// 为提供独立线程安全中断句柄的驱动构造执行器。
#[cfg(feature = "duckdb-native")]
pub(crate) fn owned_executor_with_cancel<C>(
    connection: C,
    runtime_trace: Option<RuntimeTraceEmitter>,
    cancel: Arc<dyn Fn() + Send + Sync>,
) -> Arc<dyn StandardExecutor>
where
    C: AdapterExecutionCore,
{
    Arc::new(OwnedAdapterExecutor::new(
        connection,
        runtime_trace,
        Some(cancel),
    ))
}

#[cfg(test)]
mod tests {
    use super::{owned_executor, AdapterExecutionCore};
    use crate::standard_query_result::StandardQueryResult;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::types::Value;

    struct CloseFailsOnce {
        closes: usize,
    }

    #[async_trait::async_trait]
    impl AdapterExecutionCore for CloseFailsOnce {
        async fn standard_execute_update_core(
            &mut self,
            _sql: &str,
            _values: Vec<Value>,
        ) -> Result<i64, RdbcError> {
            Ok(0)
        }

        async fn standard_execute_query_core(
            &mut self,
            _sql: &str,
            _values: Vec<Value>,
        ) -> Result<StandardQueryResult, RdbcError> {
            Ok(StandardQueryResult::rows_only(Vec::new()))
        }

        async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
            self.closes += 1;
            if self.closes == 1 {
                Err(RdbcError::DriverError("close failed once".to_owned()))
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn close_error_propagates_and_a_retry_reaches_exactly_once_success() {
        let executor = owned_executor(CloseFailsOnce { closes: 0 }, None);
        assert!(executor.close().await.is_err());
        assert!(!executor.is_closed());
        assert!(executor.close().await.is_ok());
        assert!(executor.is_closed());
        assert!(executor.close().await.is_ok());
    }

    struct SerializedCore {
        started: std::sync::Arc<tokio::sync::Notify>,
        release: std::sync::Arc<tokio::sync::Notify>,
    }

    #[async_trait::async_trait]
    impl AdapterExecutionCore for SerializedCore {
        async fn standard_execute_update_core(
            &mut self,
            _sql: &str,
            _values: Vec<Value>,
        ) -> Result<i64, RdbcError> {
            self.started.notify_one();
            self.release.notified().await;
            Ok(1)
        }

        async fn standard_execute_query_core(
            &mut self,
            _sql: &str,
            _values: Vec<Value>,
        ) -> Result<StandardQueryResult, RdbcError> {
            Ok(StandardQueryResult::rows_only(Vec::new()))
        }

        async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn close_serializes_with_in_flight_execute_and_closes_all_owned_handles() {
        let started = std::sync::Arc::new(tokio::sync::Notify::new());
        let release = std::sync::Arc::new(tokio::sync::Notify::new());
        let executor = owned_executor(
            SerializedCore {
                started: std::sync::Arc::clone(&started),
                release: std::sync::Arc::clone(&release),
            },
            None,
        );
        let execute = {
            let executor = std::sync::Arc::clone(&executor);
            tokio::spawn(async move { executor.execute_update("UPDATE", Vec::new()).await })
        };
        started.notified().await;
        let close = {
            let executor = std::sync::Arc::clone(&executor);
            tokio::spawn(async move { executor.close().await })
        };
        tokio::task::yield_now().await;
        assert!(!close.is_finished());
        release.notify_one();
        assert_eq!(execute.await.unwrap().unwrap(), 1);
        close.await.unwrap().unwrap();
        assert!(executor.execute_update("UPDATE", Vec::new()).await.is_err());
    }
}
