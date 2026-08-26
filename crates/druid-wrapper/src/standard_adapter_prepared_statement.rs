//! Wrapper-private canonical PreparedStatement backed by an owned adapter executor.

use crate::prepared_parameter_meta_data_snapshot::PreparedParameterMetaDataSnapshot;
use crate::prepared_parameter_state::PreparedParameterState;
use crate::standard_adapter_executor::StandardExecutor;
use crate::standard_adapter_result_set::StandardAdapterResultSet;
use crate::standard_statement_state::{
    delegate_standard_statement_state_methods, StandardStatementState,
};
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::ParameterMetaData;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, PreparedParameter, PreparedStatement, Statement,
    StatementOptions,
};
use druid_rdbc::types::{Row, Value};
use std::sync::Arc;

pub(crate) struct StandardAdapterPreparedStatement {
    sql: String,
    executor: Option<Arc<dyn StandardExecutor>>,
    parameters: PreparedParameterState,
    parameter_count: Option<usize>,
    state: StandardStatementState,
    parameter_batch: Vec<Vec<Value>>,
}

impl StandardAdapterPreparedStatement {
    pub(crate) fn new(
        sql: String,
        executor: Arc<dyn StandardExecutor>,
        parameter_count: Option<usize>,
        options: StatementOptions,
    ) -> Self {
        executor.task4_trace_prepared_create();
        Self {
            sql,
            executor: Some(executor),
            parameters: PreparedParameterState::new(),
            parameter_count,
            state: StandardStatementState::new(options),
            parameter_batch: Vec::new(),
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

    fn values_core(&self) -> Result<Vec<druid_rdbc::types::Value>, RdbcError> {
        match self.parameter_count {
            Some(count) => self.parameters.values(count),
            None => self.parameters.values_contiguous(),
        }
    }

    fn parameter_values(
        &self,
        mut parameters: Vec<PreparedParameter>,
    ) -> Result<Vec<Value>, RdbcError> {
        parameters.sort_by_key(|parameter| parameter.index.get());
        let expected = self.parameter_count.unwrap_or(parameters.len());
        if parameters.len() != expected {
            return Err(RdbcError::InvalidArgument(format!(
                "prepared batch expected {expected} parameters, found {}",
                parameters.len()
            )));
        }
        parameters
            .into_iter()
            .enumerate()
            .map(|(offset, parameter)| {
                if parameter.index.get() != offset + 1 {
                    return Err(RdbcError::InvalidArgument(
                        "prepared batch indexes must be contiguous and 1-based".to_owned(),
                    ));
                }
                Ok(parameter.value)
            })
            .collect()
    }
}

druid_rdbc::impl_standard_wrapper!(StandardAdapterPreparedStatement => PreparedStatement);

#[async_trait::async_trait]
impl Statement for StandardAdapterPreparedStatement {
    delegate_standard_statement_state_methods!();

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
        self.parameter_batch.clear();
        self.executor.take();
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.executor.is_none())
    }
}

#[async_trait::async_trait]
impl PreparedStatement for StandardAdapterPreparedStatement {
    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        self.parameters.set_standard(parameter)?;
        executor.task4_trace_parameter_set();
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.parameters.clear_parameters();
        Ok(())
    }

    fn add_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.parameter_batch.push(self.values_core()?);
        Ok(())
    }

    fn add_parameter_batch(&mut self, parameters: Vec<PreparedParameter>) -> Result<(), RdbcError> {
        self.executor_core()?;
        let values = self.parameter_values(parameters)?;
        self.parameter_batch.push(values);
        Ok(())
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.parameter_batch.clear();
        Ok(())
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let result = executor
            .execute_query(&self.sql, self.values_core()?)
            .await?;
        executor.task4_trace_prepared_query();
        Ok(Box::new(StandardAdapterResultSet::new(result, executor)))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let result = executor
            .execute_update_result(&self.sql, self.values_core()?)
            .await?;
        self.state.last_generated_key = result.last_insert_id;
        i64::try_from(result.rows_affected)
            .map_err(|_| RdbcError::InvalidArgument("rows affected exceeds canonical i64".into()))
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let values = self.values_core()?;
        if executor.statement_returns_rows(&self.sql).await? {
            let result = executor.execute_query(&self.sql, values).await?;
            executor.task4_trace_prepared_query();
            self.state.last_update_count = None;
            Ok(vec![ExecutionResult::ResultSet(Box::new(
                StandardAdapterResultSet::new(result, executor),
            ))])
        } else {
            let result = executor.execute_update_result(&self.sql, values).await?;
            let count = i64::try_from(result.rows_affected).map_err(|_| {
                RdbcError::InvalidArgument("rows affected exceeds canonical i64".into())
            })?;
            self.state.last_generated_key = result.last_insert_id;
            self.state.last_update_count = Some(count);
            Ok(vec![ExecutionResult::UpdateCount(count)])
        }
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let mut counts = Vec::with_capacity(self.parameter_batch.len());
        for values in &self.parameter_batch {
            match executor.execute_update(&self.sql, values.clone()).await {
                Ok(count) => counts.push(count),
                Err(cause) => {
                    return Err(RdbcError::BatchUpdateException {
                        update_counts: counts.into(),
                        cause: Box::new(cause),
                    });
                }
            }
        }
        self.parameter_batch.clear();
        Ok(counts)
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        self.executor_core()?;
        self.parameter_count
            .map(|count| {
                Arc::new(PreparedParameterMetaDataSnapshot::unknown_inputs(count))
                    as Arc<dyn ParameterMetaData>
            })
            .ok_or(RdbcError::UnsupportedOperation {
                operation: "standard_adapter_parameter_metadata",
            })
    }
}
