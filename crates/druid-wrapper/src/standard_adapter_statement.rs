//! Wrapper-private canonical Statement backed by an owned adapter executor.

use crate::standard_adapter_executor::StandardExecutor;
use crate::standard_adapter_result_set::StandardAdapterResultSet;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{GeneratedKeys, Statement};
use druid_rdbc::types::Value;
use std::sync::Arc;

pub(crate) struct StandardAdapterStatement {
    executor: Option<Arc<dyn StandardExecutor>>,
}

impl StandardAdapterStatement {
    pub(crate) fn new(executor: Arc<dyn StandardExecutor>) -> Self {
        Self {
            executor: Some(executor),
        }
    }

    fn executor_core(&self) -> Result<&Arc<dyn StandardExecutor>, RdbcError> {
        let executor = self
            .executor
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?;
        executor.ensure_open()?;
        Ok(executor)
    }

    fn generated_keys_core(generated_keys: &GeneratedKeys) -> Result<(), RdbcError> {
        if matches!(generated_keys, GeneratedKeys::None) {
            Ok(())
        } else {
            Err(RdbcError::UnsupportedOperation {
                operation: "standard_adapter_generated_keys",
            })
        }
    }
}

druid_rdbc::impl_standard_wrapper!(StandardAdapterStatement => Statement);

#[async_trait::async_trait]
impl Statement for StandardAdapterStatement {
    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let result = executor.execute_query(sql, Vec::new()).await?;
        Ok(Box::new(StandardAdapterResultSet::new(result, executor)))
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        Self::generated_keys_core(&generated_keys)?;
        self.executor_core()?.execute_update(sql, Vec::new()).await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.executor.take();
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.executor.is_none())
    }

    async fn execute_values(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let result = executor.execute_query(sql, values).await?;
        Ok(Box::new(StandardAdapterResultSet::new(result, executor)))
    }
}
