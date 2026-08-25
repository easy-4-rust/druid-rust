//! Wrapper-private owned executor shared by canonical adapter handles.

use druid_rdbc::error::RdbcError;
use druid_rdbc::types::{Row, Value};
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
    ) -> Result<Vec<Row>, RdbcError>;

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
    async fn execute_query(&self, sql: &str, values: Vec<Value>) -> Result<Vec<Row>, RdbcError>;
    async fn parameter_count(&self, sql: &str) -> Result<Option<usize>, RdbcError>;
    async fn close(&self) -> Result<(), RdbcError>;
    fn is_closed(&self) -> bool;
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

    async fn execute_query(&self, sql: &str, values: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
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
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::types::{Row, Value};

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
        ) -> Result<Vec<Row>, RdbcError> {
            Ok(Vec::new())
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
}
