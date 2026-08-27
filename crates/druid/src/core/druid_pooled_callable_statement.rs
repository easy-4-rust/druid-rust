//! Canonical pooled CallableStatement proxy.

mod identity;
mod prepared_batch;

use super::filter::ExecOperation;
use super::optional_statement_state_delegates::delegate_optional_statement_state_methods;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_registry::StandardResourceRegistry;
use super::statement_state_delegates::delegate_statement_state_methods;
use super::{DruidPooledResultSet, DruidPooledStatement, ExecResult, FilterChain, RdbcError};
use crate::stats::StatsCollector;
use druid_rdbc::metadata::{ParameterMetaData, ResultSetMetaData};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ExecutionResult, GeneratedKeys, ParameterIndex, PreparedInputParameter, PreparedParameter,
    PreparedStatement, Statement,
};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct CallableStatementResource {
    statement: tokio::sync::Mutex<Option<Box<dyn CallableStatement>>>,
    closed: AtomicBool,
}

impl CallableStatementResource {
    fn new(statement: Box<dyn CallableStatement>) -> Arc<Self> {
        Arc::new(Self {
            statement: tokio::sync::Mutex::new(Some(statement)),
            closed: AtomicBool::new(false),
        })
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    async fn close_raw(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut statement = self.statement.lock().await;
        let Some(raw) = statement.as_mut() else {
            self.closed.store(true, Ordering::Release);
            return Ok(());
        };
        raw.close().await?;
        statement.take();
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}

struct CallableStatementView {
    resource: Arc<CallableStatementResource>,
}

impl CallableStatementView {
    fn ensure_open(&self) -> Result<(), RdbcError> {
        self.resource.ensure_open()
    }
}

druid_rdbc::impl_standard_wrapper!(CallableStatementView => Statement);

#[async_trait::async_trait]
impl Statement for CallableStatementView {
    delegate_optional_statement_state_methods!(resource.statement, "callable statement is busy");

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .query_timeout()
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_query_timeout(seconds)
    }

    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .execute_query(sql)
            .await
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .execute_update(sql, generated_keys)
            .await
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .execute(sql, generated_keys)
            .await
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_batch(sql)
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_batch()
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .execute_batch()
            .await
    }

    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .generated_keys()
            .await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.resource.close_raw().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        if self.resource.closed.load(Ordering::Acquire) {
            return Ok(true);
        }
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .is_closed()
    }

    fn warning_messages(&self) -> Result<Vec<String>, RdbcError> {
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .warning_messages()
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_warnings()
    }
}

struct CallableStatementState {
    parameters: Vec<Option<PreparedInputParameter>>,
    batch_parameter_sets: Vec<Vec<PreparedInputParameter>>,
}

/// Druid logical proxy over one canonical driver CallableStatement.
pub struct DruidPooledCallableStatement {
    statement_base: DruidPooledStatement,
    resource: Arc<CallableStatementResource>,
    sql: String,
    state: Mutex<CallableStatementState>,
}

impl DruidPooledCallableStatement {
    /// Wraps one owned canonical CallableStatement.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new_standard(
        statement: Box<dyn CallableStatement>,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
        resources: Arc<StandardResourceRegistry>,
        admission: &StandardResourceAdmission,
        policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
        connection_id: u64,
        statement_id: u64,
        result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
        metadata_id_seed: Arc<std::sync::atomic::AtomicU64>,
        lease_active: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<StatsCollector>>,
    ) -> Result<Self, RdbcError> {
        let resource = CallableStatementResource::new(statement);
        let statement_base = DruidPooledStatement::new_standard(
            Box::new(CallableStatementView {
                resource: Arc::clone(&resource),
            }),
            options,
            Arc::clone(&resources),
            policy,
            connection_id,
            statement_id,
            result_set_id_seed,
            metadata_id_seed,
            lease_active,
            filter_chain,
            stats_collector,
        );
        resources
            .register(admission, Arc::clone(&statement_base.inner))
            .await?;
        Ok(Self {
            statement_base,
            resource,
            sql: sql.to_owned(),
            state: Mutex::new(CallableStatementState {
                parameters: Vec::new(),
                batch_parameter_sets: Vec::new(),
            }),
        })
    }

    /// Returns whether this logical statement is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.statement_base.is_closed()
            || self.resource.closed.load(Ordering::Acquire)
            || !self
                .statement_base
                .inner
                .lease_active
                .load(Ordering::Acquire)
    }

    /// Returns the inherited statement view.
    #[must_use]
    pub fn pooled_statement(&self) -> &DruidPooledStatement {
        &self.statement_base
    }

    /// Returns the statement identifier.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.statement_base.id()
    }

    pub(crate) fn configure_standard_query_timeout(&self, seconds: i32) -> Result<(), RdbcError> {
        self.statement_base
            .configure_standard_query_timeout(seconds)
    }

    pub(super) fn classify_statement_mutation<T>(
        &self,
        result: Result<T, RdbcError>,
    ) -> Result<T, RdbcError> {
        self.statement_base
            .inner
            .policy
            .classify(result, Some(&self.sql))
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            self.resource.ensure_open()
        }
    }

    fn parameter_snapshot(&self) -> Result<Vec<PreparedInputParameter>, RdbcError> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                parameter.clone().ok_or_else(|| {
                    RdbcError::InvalidArgument(format!(
                        "parameter {} has not been bound",
                        index + 1
                    ))
                })
            })
            .collect()
    }

    async fn execute_prepared_results(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let parameters = self.parameter_snapshot()?;
        let resource = Arc::clone(&self.resource);
        let results = self
            .statement_base
            .inner
            .policy
            .execute_prepared(
                &self.sql,
                parameters,
                self.id(),
                ExecOperation::Execute,
                move |_| async move {
                    resource
                        .statement
                        .lock()
                        .await
                        .as_mut()
                        .ok_or(RdbcError::ConnectionDiscarded)?
                        .execute_prepared()
                        .await
                },
                |results| match results.first() {
                    Some(ExecutionResult::UpdateCount(count)) => ExecResult {
                        rows_affected: u64::try_from(*count).unwrap_or_default(),
                        ..ExecResult::default()
                    },
                    Some(ExecutionResult::ResultSet(_)) | None => ExecResult::default(),
                },
            )
            .await?;
        DruidPooledResultSet::new_standard_results(
            Arc::clone(&self.statement_base.inner),
            results,
            &admission,
        )
        .await
    }
}

#[async_trait::async_trait]
impl Statement for DruidPooledCallableStatement {
    delegate_statement_state_methods!(statement_base);

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        Statement::query_timeout(&self.statement_base)
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.configure_standard_query_timeout(seconds)
    }

    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        Statement::execute_query(&mut self.statement_base, sql).await
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        Statement::execute_update(&mut self.statement_base, sql, generated_keys).await
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        Statement::execute(&mut self.statement_base, sql, generated_keys).await
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let parameters = self.parameter_snapshot()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_batch(sql);
        self.classify_statement_mutation(result)?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(parameters);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_batch();
        self.classify_statement_mutation(result)?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clear();
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.ensure_open()?;
        let parameter_sets = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clone();
        let resource = Arc::clone(&self.resource);
        let result = self
            .statement_base
            .inner
            .policy
            .execute_prepared_batch(&self.sql, &parameter_sets, self.id(), move || async move {
                resource
                    .statement
                    .lock()
                    .await
                    .as_mut()
                    .ok_or(RdbcError::ConnectionDiscarded)?
                    .execute_batch()
                    .await
            })
            .await;
        if result.is_ok() {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .batch_parameter_sets
                .clear();
        }
        result
    }

    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let raw = self
            .resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .generated_keys()
            .await?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard(
                Arc::clone(&self.statement_base.inner),
                raw,
                &admission,
            )
            .await?,
        ))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.statement_base.is_closed() {
            return Ok(());
        }
        self.resource.close_raw().await?;
        self.statement_base.inner.finish_logical_close()
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        if self.is_closed() {
            return Ok(true);
        }
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .is_closed()
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .warnings();
        self.statement_base
            .inner
            .policy
            .classify(result, Some(&self.sql))
    }

    fn warning_messages(&self) -> Result<Vec<String>, RdbcError> {
        let mut messages = Vec::new();
        let mut warning = self.warnings()?;
        while let Some(current) = warning {
            messages.push(current.message().to_owned());
            warning = current.next_warning().cloned();
        }
        Ok(messages)
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_warnings();
        self.statement_base
            .inner
            .policy
            .classify(result, Some(&self.sql))
    }
}

#[async_trait::async_trait]
impl PreparedStatement for DruidPooledCallableStatement {
    fn sql(&self) -> Result<&str, RdbcError> {
        self.ensure_open()?;
        Ok(&self.sql)
    }

    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let index = parameter.index.get() - 1;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_parameter(parameter.clone());
        self.classify_statement_mutation(result)?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.parameters.len() <= index {
            state.parameters.resize_with(index + 1, || None);
        }
        state.parameters[index] = Some(PreparedInputParameter::RustValue(parameter.value));
        Ok(())
    }

    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_input_parameter(index, parameter.clone());
        self.classify_statement_mutation(result)?;
        let offset = index.get() - 1;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.parameters.len() <= offset {
            state.parameters.resize_with(offset + 1, || None);
        }
        state.parameters[offset] = Some(parameter);
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_parameters();
        self.classify_statement_mutation(result)?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters
            .clear();
        Ok(())
    }

    fn add_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.add_prepared_batch_core()
    }

    fn add_parameter_batch(&mut self, parameters: Vec<PreparedParameter>) -> Result<(), RdbcError> {
        self.add_parameter_batch_core(parameters)
    }

    fn add_input_parameter_batch(
        &mut self,
        parameters: Vec<(ParameterIndex, PreparedInputParameter)>,
    ) -> Result<(), RdbcError> {
        self.add_input_parameter_batch_core(parameters)
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.clear_prepared_batch_core()
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let parameters = self.parameter_snapshot()?;
        let resource = Arc::clone(&self.resource);
        let raw = self
            .statement_base
            .inner
            .policy
            .execute_prepared(
                &self.sql,
                parameters,
                self.id(),
                ExecOperation::Query,
                move |_| async move {
                    resource
                        .statement
                        .lock()
                        .await
                        .as_mut()
                        .ok_or(RdbcError::ConnectionDiscarded)?
                        .execute_prepared_query()
                        .await
                },
                |_| ExecResult::default(),
            )
            .await?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard(
                Arc::clone(&self.statement_base.inner),
                raw,
                &admission,
            )
            .await?,
        ))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        let parameters = self.parameter_snapshot()?;
        let resource = Arc::clone(&self.resource);
        self.statement_base
            .inner
            .policy
            .execute_prepared(
                &self.sql,
                parameters,
                self.id(),
                ExecOperation::Update,
                move |_| async move {
                    resource
                        .statement
                        .lock()
                        .await
                        .as_mut()
                        .ok_or(RdbcError::ConnectionDiscarded)?
                        .execute_prepared_update()
                        .await
                },
                |count| ExecResult {
                    rows_affected: u64::try_from(*count).unwrap_or_default(),
                    ..ExecResult::default()
                },
            )
            .await
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.execute_prepared_results().await
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.execute_prepared_batch_core().await
    }

    fn result_set_meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.result_set_meta_data_core()
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .parameter_meta_data()
    }
}

impl CallableStatement for DruidPooledCallableStatement {
    fn register_out_parameter(
        &mut self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .register_out_parameter(parameter, out_parameter);
        self.classify_statement_mutation(result)
    }

    fn set_named_parameter(
        &mut self,
        parameter_name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_named_parameter(parameter_name, parameter);
        self.classify_statement_mutation(result)
    }

    fn out_parameter(&self, parameter: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .out_parameter(parameter)
    }

    fn out_value(&self, index: ParameterIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .out_value(index)
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .was_null()
    }
}
