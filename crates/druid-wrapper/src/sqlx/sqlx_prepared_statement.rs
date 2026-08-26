//! Canonical SQLx Statement and PreparedStatement actor.

use super::sqlx_connection_actor::SqlxConnectionActor;
use crate::prepared_parameter_meta_data_snapshot::PreparedParameterMetaDataSnapshot;
use crate::prepared_parameter_state::PreparedParameterState;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::ParameterMetaData;
use druid_rdbc::types::Value;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

pub struct SqlxPreparedStatement {
    sql: String,
    actor: Option<Arc<SqlxConnectionActor>>,
    closed: AtomicBool,
    query_timeout: AtomicI32,
    parameter_state: PreparedParameterState,
    parameter_count: usize,
}

impl SqlxPreparedStatement {
    pub(crate) fn owned_statement(
        sql: String,
        actor: Arc<SqlxConnectionActor>,
        _options: druid_rdbc::statement::StatementOptions,
        parameter_count: usize,
    ) -> Self {
        actor.task4_trace_statement_create();
        Self::owned(sql, actor, parameter_count)
    }

    pub(crate) fn owned_prepared_statement(
        sql: String,
        actor: Arc<SqlxConnectionActor>,
        _options: druid_rdbc::statement::StatementOptions,
        parameter_count: usize,
    ) -> Self {
        actor.task4_trace_prepared_create();
        Self::owned(sql, actor, parameter_count)
    }

    fn owned(sql: String, actor: Arc<SqlxConnectionActor>, parameter_count: usize) -> Self {
        Self {
            sql,
            actor: Some(actor),
            closed: AtomicBool::new(false),
            query_timeout: AtomicI32::new(0),
            parameter_state: PreparedParameterState::new(),
            parameter_count,
        }
    }

    fn actor(&self) -> Result<&Arc<SqlxConnectionActor>, RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        match self.actor.as_ref() {
            Some(actor) if !actor.is_closed() => Ok(actor),
            Some(_) | None => Err(RdbcError::ConnectionDiscarded),
        }
    }

    async fn bounded<T, F>(&self, execution: F) -> Result<T, RdbcError>
    where
        F: Future<Output = Result<T, RdbcError>>,
    {
        let seconds = self.query_timeout.load(Ordering::Acquire);
        if seconds <= 0 {
            return execution.await;
        }
        tokio::time::timeout(std::time::Duration::from_secs(seconds as u64), execution)
            .await
            .map_err(|_| RdbcError::Other("statement query timed out".to_owned()))?
    }
}

druid_rdbc::impl_standard_wrapper!(SqlxPreparedStatement => PreparedStatement);

#[async_trait::async_trait]
impl druid_rdbc::statement::Statement for SqlxPreparedStatement {
    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.actor()?;
        Ok(self.query_timeout.load(Ordering::Acquire))
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.actor()?;
        if seconds < 0 {
            return Err(RdbcError::InvalidArgument(
                "query timeout must not be negative".to_owned(),
            ));
        }
        self.query_timeout.store(seconds, Ordering::Release);
        Ok(())
    }

    async fn execute_query(
        &mut self,
        sql: &str,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let actor = Arc::clone(self.actor()?);
        self.bounded(actor.execute_query(sql, Vec::new())).await
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        if !matches!(generated_keys, druid_rdbc::statement::GeneratedKeys::None) {
            return Err(RdbcError::UnsupportedOperation {
                operation: "sqlx_standard_generated_keys",
            });
        }
        let actor = Arc::clone(self.actor()?);
        let rows_affected = self.bounded(actor.execute_update(sql, Vec::new())).await?;
        actor.task4_trace_statement_update();
        Ok(rows_affected)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed.store(true, Ordering::Release);
        self.actor.take();
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }

    async fn execute_values(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let actor = Arc::clone(self.actor()?);
        self.bounded(actor.execute_query(sql, values)).await
    }
}

#[async_trait::async_trait]
impl druid_rdbc::statement::PreparedStatement for SqlxPreparedStatement {
    fn set_parameter(
        &mut self,
        parameter: druid_rdbc::statement::PreparedParameter,
    ) -> Result<(), RdbcError> {
        let actor = Arc::clone(self.actor()?);
        self.parameter_state.set_standard(parameter)?;
        actor.task4_trace_parameter_set();
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        self.parameter_state.clear_parameters();
        Ok(())
    }

    async fn execute_prepared_query(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let actor = Arc::clone(self.actor()?);
        let values = self.parameter_state.values(self.parameter_count)?;
        let result = self.bounded(actor.execute_query(&self.sql, values)).await?;
        actor.task4_trace_prepared_query();
        Ok(result)
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        let actor = Arc::clone(self.actor()?);
        let values = self.parameter_state.values(self.parameter_count)?;
        self.bounded(actor.execute_update(&self.sql, values)).await
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        self.actor()?;
        Ok(Arc::new(PreparedParameterMetaDataSnapshot::unknown_inputs(
            self.parameter_count,
        )))
    }
}
