use crate::prepared_parameter_state::PreparedParameterState;
use druid_core::core::{
    DruidError, PhysicalPreparedStatement, PhysicalStatement, PhysicalStatementOptions,
    PreparedInputParameter, SqlTextStatement, Value,
};
use std::any::Any;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch;

/// HTTP SQL 请求执行期间由 Druid 控制面产生的中断结果。
pub(crate) enum HttpSqlStatementExecutionError {
    Driver(DruidError),
    TimedOut,
    Cancelled,
}

/// HTTP SQL 产品的物理 `PreparedStatement` 状态。
pub struct HttpSqlPreparedStatement {
    sql: String,
    closed: AtomicBool,
    statement: SqlTextStatement,
    parameter_state: PreparedParameterState,
    cancel_generation: watch::Sender<u64>,
}

impl HttpSqlPreparedStatement {
    /// 创建一个仅属于当前 HTTP SQL 物理连接的语句句柄。
    #[must_use]
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
            parameter_state: PreparedParameterState::new(),
            cancel_generation: watch::channel(0).0,
        }
    }

    pub(crate) async fn execute_with_controls<T, F>(
        &self,
        execution: F,
    ) -> Result<T, HttpSqlStatementExecutionError>
    where
        F: Future<Output = Result<T, DruidError>>,
    {
        let generation = *self.cancel_generation.borrow();
        let mut cancellation = self.cancel_generation.subscribe();
        let cancelled = async move {
            if *cancellation.borrow() != generation {
                return;
            }
            while cancellation.changed().await.is_ok() {
                if *cancellation.borrow() != generation {
                    return;
                }
            }
        };
        let timeout_seconds = self
            .statement
            .query_timeout()
            .map_err(HttpSqlStatementExecutionError::Driver)?;
        if timeout_seconds > 0 {
            tokio::select! {
                result = execution => result.map_err(HttpSqlStatementExecutionError::Driver),
                () = cancelled => Err(HttpSqlStatementExecutionError::Cancelled),
                () = tokio::time::sleep(std::time::Duration::from_secs(timeout_seconds as u64)) => {
                    Err(HttpSqlStatementExecutionError::TimedOut)
                }
            }
        } else {
            tokio::select! {
                result = execution => result.map_err(HttpSqlStatementExecutionError::Driver),
                () = cancelled => Err(HttpSqlStatementExecutionError::Cancelled),
            }
        }
    }
}

impl PhysicalPreparedStatement for HttpSqlPreparedStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn max_rows(&self) -> Result<i32, DruidError> {
        self.statement.max_rows()
    }

    fn set_max_rows(&self, max: i32) -> Result<(), DruidError> {
        self.statement.set_max_rows(max)
    }

    fn query_timeout(&self) -> Result<i32, DruidError> {
        self.statement.query_timeout()
    }

    fn set_query_timeout(&self, seconds: i32) -> Result<(), DruidError> {
        self.statement.set_query_timeout(seconds)
    }

    fn cancel(&self) -> Result<(), DruidError> {
        self.statement.cancel()?;
        let next = (*self.cancel_generation.borrow()).wrapping_add(1);
        self.cancel_generation.send_replace(next);
        Ok(())
    }

    fn set_parameter(
        &self,
        parameter_index: usize,
        parameter: &PreparedInputParameter,
    ) -> Result<(), DruidError> {
        self.parameter_state.set(parameter_index, parameter)
    }

    fn clear_parameters(&self) -> Result<(), DruidError> {
        self.parameter_state.clear_parameters();
        Ok(())
    }

    fn add_batch(&self, params: &[Value]) -> Result<(), DruidError> {
        self.parameter_state.add_values(params);
        Ok(())
    }

    fn add_parameter_batch(&self, params: &[PreparedInputParameter]) -> Result<(), DruidError> {
        self.parameter_state.add_parameters(params)
    }

    fn clear_batch(&self) -> Result<(), DruidError> {
        self.parameter_state.clear_batches();
        Ok(())
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        self.statement.close()
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
