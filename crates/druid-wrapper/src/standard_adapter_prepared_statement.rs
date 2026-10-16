//! Wrapper-private canonical PreparedStatement backed by an owned adapter executor.

use crate::prepared_parameter_meta_data_snapshot::PreparedParameterMetaDataSnapshot;
use crate::prepared_parameter_state::PreparedParameterState;
use crate::standard_adapter_executor::StandardExecutor;
use crate::standard_adapter_result_set::StandardAdapterResultSet;
use crate::standard_statement_state::{
    delegate_standard_statement_state_methods, StandardStatementState,
};
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use druid_rdbc::metadata::ParameterMetaData;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, PreparedParameter, PreparedStatement, Statement,
    StatementOptions,
};
use druid_rdbc::types::{Row, Value};
use std::sync::Arc;

#[cfg(test)]
mod cancellation_tests;

#[cfg(test)]
mod generated_keys_cancellation_tests;

pub(crate) struct StandardAdapterPreparedStatement {
    sql: String,
    executor: Option<Arc<dyn StandardExecutor>>,
    parameters: PreparedParameterState,
    parameter_count: Option<usize>,
    eager_resource_binding: bool,
    state: StandardStatementState,
    /// 当前成功执行的完整生成键；批量执行按命令顺序发布。
    generated_keys: Vec<i64>,
    parameter_batch: Vec<Vec<druid_rdbc::statement::PreparedInputParameter>>,
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
            eager_resource_binding: false,
            state: StandardStatementState::new(options),
            generated_keys: Vec::new(),
            parameter_batch: Vec::new(),
        }
    }

    /// SQLite Toasty 保留同步 setter 资源读取语义；异步 LOB 仍交给执行边界。
    pub(crate) fn with_eager_resource_binding(mut self, enabled: bool) -> Self {
        self.eager_resource_binding = enabled;
        self
    }

    fn executor_core(&self) -> Result<&Arc<dyn StandardExecutor>, RdbcError> {
        let executor = self
            .executor
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?;
        executor.ensure_open()?;
        Ok(executor)
    }

    fn cancel_standard(&mut self) -> Result<(), RdbcError> {
        self.executor_core()?.cancel_running()
    }

    /// 新执行尝试先失效旧结果，参数错误、驱动失败或取消不得暴露上一轮键。
    fn reset_execution_results(&mut self) {
        self.generated_keys.clear();
        self.state.last_update_count = None;
    }

    async fn values_core(&self) -> Result<Vec<druid_rdbc::types::Value>, RdbcError> {
        match self.parameter_count {
            Some(count) => self.parameters.values(count).await,
            None => self.parameters.values_contiguous().await,
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
        self.reset_execution_results();
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
            .generated_keys
            .iter()
            .map(|value| Row::new(vec![Value::Int(*value)]))
            .collect();
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
    fn sql(&self) -> Result<&str, RdbcError> {
        self.executor_core()?;
        Ok(&self.sql)
    }

    fn set_input_parameter(
        &mut self,
        index: druid_rdbc::statement::ParameterIndex,
        parameter: druid_rdbc::statement::PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        let parameter = if self.eager_resource_binding
            && matches!(
                parameter,
                druid_rdbc::statement::PreparedInputParameter::AsciiStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::UnicodeStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::BinaryStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::BlobStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::CharacterStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::NCharacterStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::ClobReader { .. }
                    | druid_rdbc::statement::PreparedInputParameter::NClobReader { .. }
            ) {
            // 底层槽存储一次读取后的值，重复执行及批次快照不能再消费调用方资源。
            let value = crate::prepared_parameter_materializer::PreparedParameterMaterializer::materialize_immediate(&parameter)?
                .ok_or_else(|| RdbcError::Other("synchronous stream binding requires a value".to_owned()))?;
            druid_rdbc::statement::PreparedInputParameter::RustValue(value)
        } else {
            parameter
        };
        self.parameters.set_input(index, parameter)?;
        executor.task4_trace_parameter_set();
        Ok(())
    }

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
        let inputs = match self.parameter_count {
            Some(count) => self.parameters.inputs(count)?,
            None => self.parameters.inputs_contiguous()?,
        };
        self.parameter_batch.push(inputs);
        Ok(())
    }

    fn add_parameter_batch(&mut self, parameters: Vec<PreparedParameter>) -> Result<(), RdbcError> {
        self.executor_core()?;
        let values = self.parameter_values(parameters)?;
        self.parameter_batch.push(
            values
                .into_iter()
                .map(druid_rdbc::statement::PreparedInputParameter::RustValue)
                .collect(),
        );
        Ok(())
    }

    fn add_input_parameter_batch(
        &mut self,
        mut parameters: Vec<(
            druid_rdbc::statement::ParameterIndex,
            druid_rdbc::statement::PreparedInputParameter,
        )>,
    ) -> Result<(), RdbcError> {
        self.executor_core()?;
        parameters.sort_by_key(|(index, _)| index.get());
        let expected = self.parameter_count.unwrap_or(parameters.len());
        if parameters.len() != expected
            || parameters
                .iter()
                .enumerate()
                .any(|(offset, (index, _))| index.get() != offset + 1)
        {
            return Err(RdbcError::InvalidArgument(
                "prepared batch indexes must match contiguous prepared width".to_owned(),
            ));
        }
        self.parameter_batch
            .push(parameters.into_iter().map(|(_, input)| input).collect());
        Ok(())
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.parameter_batch.clear();
        Ok(())
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.reset_execution_results();
        let executor = Arc::clone(self.executor_core()?);
        let values = self.values_core().await?;
        let execution = executor.execute_query(&self.sql, values);
        let result = if self.state.query_timeout > 0 {
            if let Ok(result) = tokio::time::timeout(
                std::time::Duration::from_secs(self.state.query_timeout as u64),
                execution,
            )
            .await
            {
                result?
            } else {
                executor.cancel_running()?;
                return Err(RdbcError::SqlException(Box::new(
                    SqlException::new(
                        0,
                        Some("HYT00".to_owned()),
                        Some("statement query timed out".to_owned()),
                    )
                    .with_class_name("java.sql.SQLTimeoutException")
                    .with_assignable_type("java.sql.SQLTransientException"),
                )));
            }
        } else {
            execution.await?
        };
        executor.task4_trace_prepared_query();
        self.generated_keys = result.generated_keys.clone();
        Ok(Box::new(StandardAdapterResultSet::new(result, executor)))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.reset_execution_results();
        let executor = Arc::clone(self.executor_core()?);
        let (count, keys) = executor
            .execute_update_keys(&self.sql, self.values_core().await?)
            .await?;
        self.generated_keys = keys;
        self.state.last_update_count = Some(count);
        Ok(count)
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.reset_execution_results();
        let executor = Arc::clone(self.executor_core()?);
        let values = self.values_core().await?;
        if executor.statement_returns_rows(&self.sql).await? {
            let result = executor.execute_query(&self.sql, values).await?;
            executor.task4_trace_prepared_query();
            self.generated_keys = result.generated_keys.clone();
            self.state.last_update_count = None;
            Ok(vec![ExecutionResult::ResultSet(Box::new(
                StandardAdapterResultSet::new(result, executor),
            ))])
        } else {
            let (count, keys) = executor.execute_update_keys(&self.sql, values).await?;
            self.generated_keys = keys;
            self.state.last_update_count = Some(count);
            Ok(vec![ExecutionResult::UpdateCount(count)])
        }
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.reset_execution_results();
        let executor = Arc::clone(self.executor_core()?);
        // 与 BASE take_batches 一致：一次执行尝试消费队列，失败或取消不重放已读资源。
        let parameter_sets = std::mem::take(&mut self.parameter_batch);
        let mut counts = Vec::with_capacity(parameter_sets.len());
        let mut keys = Vec::with_capacity(parameter_sets.len());
        for inputs in &parameter_sets {
            let execution = async {
                let values = PreparedParameterState::materialize(inputs).await?;
                executor.execute_update_keys(&self.sql, values).await
            };
            match execution.await {
                Ok((count, key)) => {
                    counts.push(count);
                    keys.extend(key);
                }
                Err(cause) => {
                    self.reset_execution_results();
                    return Err(RdbcError::BatchUpdateException {
                        update_counts: counts.into(),
                        cause: Box::new(cause),
                    });
                }
            }
        }
        // 中途失败或取消时仅丢弃局部结果；完成整批后才对调用方发布。
        self.generated_keys = keys;
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
