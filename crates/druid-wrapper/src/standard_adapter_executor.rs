//! Wrapper-private owned executor shared by canonical adapter handles.

use crate::standard_query_result::StandardQueryResult;
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Concrete adapters expose one private driver core to Physical and canonical callers.
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

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError>;

    async fn standard_parameter_count_core(
        &mut self,
        _sql: &str,
    ) -> Result<Option<usize>, RdbcError> {
        Ok(None)
    }
}

/// Type-erased executor owned by canonical Statement/Prepared/ResultSet handles.
#[async_trait::async_trait]
pub(crate) trait StandardExecutor: Send + Sync {
    async fn execute_update(&self, sql: &str, values: Vec<Value>) -> Result<i64, RdbcError>;
    async fn execute_query(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError>;
    async fn parameter_count(&self, sql: &str) -> Result<Option<usize>, RdbcError>;
    async fn close(&self) -> Result<(), RdbcError>;
    fn is_closed(&self) -> bool;

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
}

impl<C> OwnedAdapterExecutor<C> {
    fn new(connection: C) -> Self {
        Self {
            connection: Mutex::new(Some(connection)),
            closed: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl<C> StandardExecutor for OwnedAdapterExecutor<C>
where
    C: AdapterExecutionCore,
{
    async fn execute_update(&self, sql: &str, values: Vec<Value>) -> Result<i64, RdbcError> {
        let mut guard = self.connection.lock().await;
        let connection = guard.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        connection.standard_execute_update_core(sql, values).await
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
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

pub(crate) fn owned_executor<C>(connection: C) -> Arc<dyn StandardExecutor>
where
    C: AdapterExecutionCore,
{
    Arc::new(OwnedAdapterExecutor::new(connection))
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
        let executor = owned_executor(CloseFailsOnce { closes: 0 });
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
        let executor = owned_executor(SerializedCore {
            started: std::sync::Arc::clone(&started),
            release: std::sync::Arc::clone(&release),
        });
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
