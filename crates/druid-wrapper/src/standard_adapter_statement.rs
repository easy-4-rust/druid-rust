//! Wrapper-private canonical Statement backed by an owned adapter executor.

use crate::standard_adapter_executor::StandardExecutor;
use crate::standard_adapter_result_set::StandardAdapterResultSet;
use crate::standard_statement_state::{
    delegate_standard_statement_state_methods, StandardStatementState,
};
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, GeneratedKeys, Statement, StatementOptions,
};
use druid_rdbc::types::{Row, Value};
use std::sync::Arc;

pub(crate) struct StandardAdapterStatement {
    executor: Option<Arc<dyn StandardExecutor>>,
    state: StandardStatementState,
}

impl StandardAdapterStatement {
    pub(crate) fn new(executor: Arc<dyn StandardExecutor>, options: StatementOptions) -> Self {
        executor.task4_trace_statement_create();
        Self {
            executor: Some(executor),
            state: StandardStatementState::new(options),
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
        if matches!(
            generated_keys,
            GeneratedKeys::ColumnIndexes(_) | GeneratedKeys::ColumnNames(_)
        ) {
            Err(RdbcError::UnsupportedOperation {
                operation: "standard_adapter_generated_keys",
            })
        } else {
            Ok(())
        }
    }
}

druid_rdbc::impl_standard_wrapper!(StandardAdapterStatement => Statement);

#[async_trait::async_trait]
impl Statement for StandardAdapterStatement {
    delegate_standard_statement_state_methods!();

    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.state.last_generated_key = None;
        self.state.last_update_count = None;
        let executor = Arc::clone(self.executor_core()?);
        let result = executor.execute_query(sql, Vec::new()).await?;
        self.state.last_update_count = None;
        Ok(Box::new(StandardAdapterResultSet::new(result, executor)))
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.state.last_generated_key = None;
        self.state.last_update_count = None;
        Self::generated_keys_core(&generated_keys)?;
        let executor = Arc::clone(self.executor_core()?);
        let result = executor.execute_update_result(sql, Vec::new()).await?;
        let rows_affected = i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".into())
        })?;
        self.state.last_generated_key = result.last_insert_id;
        self.state.last_update_count = Some(rows_affected);
        executor.task4_trace_statement_update();
        Ok(rows_affected)
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.state.last_generated_key = None;
        self.state.last_update_count = None;
        Self::generated_keys_core(&generated_keys)?;
        let executor = Arc::clone(self.executor_core()?);
        if executor.statement_returns_rows(sql).await? {
            let result = executor.execute_query(sql, Vec::new()).await?;
            self.state.last_update_count = None;
            Ok(vec![ExecutionResult::ResultSet(Box::new(
                StandardAdapterResultSet::new(result, executor),
            ))])
        } else {
            let result = executor.execute_update_result(sql, Vec::new()).await?;
            let count = i64::try_from(result.rows_affected).map_err(|_| {
                RdbcError::InvalidArgument("rows affected exceeds canonical i64".into())
            })?;
            self.state.last_generated_key = result.last_insert_id;
            self.state.last_update_count = Some(count);
            executor.task4_trace_statement_update();
            Ok(vec![ExecutionResult::UpdateCount(count)])
        }
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.state.statement_batch.push(sql);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.state.statement_batch.clear();
        Ok(())
    }

    fn batch(&self) -> Result<Vec<String>, RdbcError> {
        self.executor_core()?;
        Ok(self.state.statement_batch.clone())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let mut counts = Vec::with_capacity(self.state.statement_batch.len());
        for sql in &self.state.statement_batch {
            match executor.execute_update(sql, Vec::new()).await {
                Ok(count) => counts.push(count),
                Err(cause) => {
                    return Err(RdbcError::BatchUpdateException {
                        update_counts: counts.into(),
                        cause: Box::new(cause),
                    });
                }
            }
        }
        Ok(counts)
    }

    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let rows = self
            .state
            .last_generated_key
            .map(|value| vec![Row::new(vec![Value::Int(value)])])
            .unwrap_or_default();
        Ok(Box::new(StandardAdapterResultSet::new(
            crate::standard_query_result::StandardQueryResult::with_labels(
                rows,
                vec!["GENERATED_KEY".to_owned()],
            ),
            executor,
        )))
    }

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.executor_core()?;
        Ok(self.state.last_update_count)
    }

    async fn get_more_results(
        &mut self,
        _disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.executor_core()?;
        self.state.last_update_count = None;
        Ok(false)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.state.statement_batch.clear();
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
