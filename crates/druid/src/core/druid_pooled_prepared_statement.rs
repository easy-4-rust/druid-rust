//! Canonical pooled PreparedStatement proxy.

use super::filter::ExecOperation;
use super::prepared_statement_key::PreparedStatementKey;
use super::standard_prepared_statement_cache::StandardPreparedStatementCache;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_registry::StandardResourceRegistry;
use super::{DruidPooledResultSet, DruidPooledStatement, ExecResult, FilterChain, RdbcError};
use crate::stats::StatsCollector;
use druid_rdbc::metadata::ParameterMetaData;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    ExecutionResult, GeneratedKeys, PreparedParameter, PreparedStatement, Statement,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct PreparedStatementResource {
    statement: tokio::sync::Mutex<Option<Box<dyn PreparedStatement>>>,
    closed: AtomicBool,
}

impl PreparedStatementResource {
    fn new(statement: Box<dyn PreparedStatement>) -> Arc<Self> {
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

/// Concrete Statement view over the canonical prepared handle.
struct PreparedStatementView {
    resource: Arc<PreparedStatementResource>,
}

druid_rdbc::impl_standard_wrapper!(PreparedStatementView => Statement);

#[async_trait::async_trait]
impl Statement for PreparedStatementView {
    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .query_timeout()
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_query_timeout(seconds)
    }

    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.resource.ensure_open()?;
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
        self.resource.ensure_open()?;
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
        self.resource.ensure_open()?;
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
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_batch(sql)
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_batch()
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.resource.ensure_open()?;
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
        self.resource.ensure_open()?;
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
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .is_closed()
    }

    fn warning_messages(&self) -> Result<Vec<String>, RdbcError> {
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .warning_messages()
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_warnings()
    }
}

struct PreparedStatementState {
    parameters: Vec<Option<PreparedParameter>>,
    batch_parameter_sets: Vec<Vec<PreparedParameter>>,
}

/// Druid logical proxy over one canonical driver PreparedStatement.
pub struct DruidPooledPreparedStatement {
    statement_base: DruidPooledStatement,
    resource: Arc<PreparedStatementResource>,
    sql: String,
    key: Option<PreparedStatementKey>,
    cache: Option<Arc<tokio::sync::Mutex<StandardPreparedStatementCache>>>,
    state: Mutex<PreparedStatementState>,
}

impl std::fmt::Debug for DruidPooledPreparedStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledPreparedStatement")
            .field("sql", &self.sql)
            .field("statement_base", &self.statement_base)
            .field("cached", &self.cache.is_some())
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}

druid_rdbc::impl_standard_wrapper!(DruidPooledPreparedStatement => PreparedStatement);

impl DruidPooledPreparedStatement {
    /// Wraps one owned canonical PreparedStatement.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new_standard(
        statement: Box<dyn PreparedStatement>,
        sql: &str,
        key: PreparedStatementKey,
        cache: Arc<tokio::sync::Mutex<StandardPreparedStatementCache>>,
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
        let resource = PreparedStatementResource::new(statement);
        let statement_base = DruidPooledStatement::new_standard(
            Box::new(PreparedStatementView {
                resource: Arc::clone(&resource),
            }),
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
            key: Some(key),
            cache: Some(cache),
            state: Mutex::new(PreparedStatementState {
                parameters: Vec::new(),
                batch_parameter_sets: Vec::new(),
            }),
        })
    }

    /// Returns whether the logical statement is closed.
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
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_query_timeout(seconds)
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            self.resource.ensure_open()
        }
    }

    fn parameter_snapshot(&self) -> Result<Vec<PreparedParameter>, RdbcError> {
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

    async fn close_logical(&mut self) -> Result<(), RdbcError> {
        if self.statement_base.is_closed() {
            return Ok(());
        }
        let mut statement = self.resource.statement.lock().await;
        let Some(raw) = statement.as_mut() else {
            self.resource.closed.store(true, Ordering::Release);
            return self.statement_base.inner.finish_logical_close();
        };

        raw.clear_parameters()?;
        let raw_closed = raw.is_closed()?;
        if let (Some(key), Some(cache)) = (self.key.as_ref(), self.cache.clone()) {
            let cache_enabled = cache.lock().await.is_enabled();
            if self
                .statement_base
                .inner
                .lease_active
                .load(Ordering::Acquire)
                && cache_enabled
                && !raw_closed
            {
                let raw = statement.take().ok_or(RdbcError::ConnectionDiscarded)?;
                self.resource.closed.store(true, Ordering::Release);
                drop(statement);
                let removed = cache.lock().await.put(key.clone(), raw);
                self.key.take();
                self.cache.take();
                let mut first_error = None;
                let mut failed = Vec::new();
                for mut evicted in removed {
                    if let Err(error) = evicted.close().await {
                        first_error.get_or_insert(error);
                        failed.push(evicted);
                    }
                }
                if !failed.is_empty() {
                    cache.lock().await.retain_failed_close(failed);
                }
                self.statement_base.inner.finish_logical_close()?;
                return first_error.map_or(Ok(()), Err);
            }
        }

        if !raw_closed {
            raw.close().await?;
        }
        statement.take();
        self.resource.closed.store(true, Ordering::Release);
        self.key.take();
        self.cache.take();
        drop(statement);
        self.statement_base.inner.finish_logical_close()
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
impl Statement for DruidPooledPreparedStatement {
    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        Statement::query_timeout(&self.statement_base)
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.ensure_open()?;
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
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_batch(sql)?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(parameters);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_batch()?;
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
        self.close_logical().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        if self.is_closed() {
            return Ok(true);
        }
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .is_closed()
    }

    fn warning_messages(&self) -> Result<Vec<String>, RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .warning_messages()
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_warnings()
    }
}

#[async_trait::async_trait]
impl PreparedStatement for DruidPooledPreparedStatement {
    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let index = parameter.index.get() - 1;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_parameter(parameter.clone())?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.parameters.len() <= index {
            state.parameters.resize_with(index + 1, || None);
        }
        state.parameters[index] = Some(parameter);
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_parameters()?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters
            .clear();
        Ok(())
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

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .parameter_meta_data()
    }
}
