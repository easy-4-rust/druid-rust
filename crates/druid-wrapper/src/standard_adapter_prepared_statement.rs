//! Wrapper-private canonical PreparedStatement backed by an owned adapter executor.

use crate::prepared_parameter_meta_data_snapshot::PreparedParameterMetaDataSnapshot;
use crate::prepared_parameter_state::PreparedParameterState;
use crate::standard_adapter_executor::StandardExecutor;
use crate::standard_adapter_result_set::StandardAdapterResultSet;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::ParameterMetaData;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{PreparedParameter, PreparedStatement, Statement};
use std::sync::Arc;

pub(crate) struct StandardAdapterPreparedStatement {
    sql: String,
    executor: Option<Arc<dyn StandardExecutor>>,
    parameters: PreparedParameterState,
    parameter_count: Option<usize>,
}

impl StandardAdapterPreparedStatement {
    pub(crate) fn new(
        sql: String,
        executor: Arc<dyn StandardExecutor>,
        parameter_count: Option<usize>,
    ) -> Self {
        Self {
            sql,
            executor: Some(executor),
            parameters: PreparedParameterState::new(),
            parameter_count,
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

    async fn values_core(&self) -> Result<Vec<druid_rdbc::types::Value>, RdbcError> {
        match self.parameter_count {
            Some(count) => self.parameters.values(count).await,
            None => self.parameters.values_contiguous().await,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(StandardAdapterPreparedStatement => PreparedStatement);

#[async_trait::async_trait]
impl Statement for StandardAdapterPreparedStatement {
    async fn close(&mut self) -> Result<(), RdbcError> {
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
        self.executor_core()?;
        self.parameters.set_standard(parameter)
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.parameters.clear_parameters();
        Ok(())
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let result = executor
            .execute_query(&self.sql, self.values_core().await?)
            .await?;
        Ok(Box::new(StandardAdapterResultSet::new(result, executor)))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.executor_core()?
            .execute_update(&self.sql, self.values_core().await?)
            .await
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
