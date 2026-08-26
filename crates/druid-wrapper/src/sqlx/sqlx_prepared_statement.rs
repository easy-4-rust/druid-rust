//! Canonical SQLx Statement and PreparedStatement actor.

use super::sqlx_connection_actor::SqlxConnectionActor;
use crate::prepared_parameter_meta_data_snapshot::PreparedParameterMetaDataSnapshot;
use crate::prepared_parameter_state::PreparedParameterState;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::metadata::{ParameterMetaData, ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::statement::{
    ExecutionResult, FetchDirection, GeneratedKeys, PreparedParameter, StatementOptions,
};
use druid_rdbc::types::Value;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

pub struct SqlxPreparedStatement {
    sql: String,
    actor: Option<Arc<SqlxConnectionActor>>,
    closed: AtomicBool,
    options: StatementOptions,
    max_field_size: AtomicI32,
    max_rows: AtomicI64,
    query_timeout: AtomicI32,
    fetch_direction: Mutex<FetchDirection>,
    fetch_size: AtomicI32,
    poolable: AtomicBool,
    close_on_completion: AtomicBool,
    statement_batch: Mutex<Vec<String>>,
    parameter_state: PreparedParameterState,
    prepared_batch: Mutex<Vec<Vec<Value>>>,
    parameter_count: usize,
    result_set_meta_data: Arc<ResultSetMetaDataSnapshot>,
    generated_key: Mutex<Option<i64>>,
    last_update_count: AtomicI64,
}

impl SqlxPreparedStatement {
    pub(crate) fn owned_statement(
        sql: String,
        actor: Arc<SqlxConnectionActor>,
        options: StatementOptions,
        parameter_count: usize,
    ) -> Self {
        actor.task4_trace_statement_create();
        Self::owned(
            sql,
            actor,
            options,
            parameter_count,
            ResultSetMetaDataSnapshot::default(),
        )
    }

    pub(crate) fn owned_prepared_statement(
        sql: String,
        actor: Arc<SqlxConnectionActor>,
        options: StatementOptions,
        parameter_count: usize,
        result_set_meta_data: ResultSetMetaDataSnapshot,
    ) -> Self {
        actor.task4_trace_prepared_create();
        Self::owned(sql, actor, options, parameter_count, result_set_meta_data)
    }

    fn owned(
        sql: String,
        actor: Arc<SqlxConnectionActor>,
        options: StatementOptions,
        parameter_count: usize,
        result_set_meta_data: ResultSetMetaDataSnapshot,
    ) -> Self {
        Self {
            sql,
            actor: Some(actor),
            closed: AtomicBool::new(false),
            options,
            max_field_size: AtomicI32::new(0),
            max_rows: AtomicI64::new(0),
            query_timeout: AtomicI32::new(0),
            fetch_direction: Mutex::new(FetchDirection::Unknown),
            fetch_size: AtomicI32::new(0),
            poolable: AtomicBool::new(false),
            close_on_completion: AtomicBool::new(false),
            statement_batch: Mutex::new(Vec::new()),
            parameter_state: PreparedParameterState::new(),
            prepared_batch: Mutex::new(Vec::new()),
            parameter_count,
            result_set_meta_data: Arc::new(result_set_meta_data),
            generated_key: Mutex::new(None),
            last_update_count: AtomicI64::new(-1),
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

    async fn execute_update_values(
        &self,
        sql: &str,
        values: Vec<Value>,
        generated_keys: &GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        if std::matches!(
            generated_keys,
            GeneratedKeys::ColumnIndexes(_) | GeneratedKeys::ColumnNames(_)
        ) {
            return Err(RdbcError::feature_not_supported(
                "sqlx_generated_keys_column_selection",
            ));
        }
        let actor = Arc::clone(self.actor()?);
        let result = self
            .bounded(actor.execute_update_result(sql, values))
            .await?;
        let count = i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".to_owned())
        })?;
        self.last_update_count.store(count, Ordering::Release);
        *self
            .generated_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            if std::matches!(generated_keys, GeneratedKeys::ReturnGeneratedKeys) {
                result.last_insert_id
            } else {
                None
            };
        Ok(count)
    }

    fn parameter_values(
        &self,
        mut parameters: Vec<PreparedParameter>,
    ) -> Result<Vec<Value>, RdbcError> {
        parameters.sort_by_key(|parameter| parameter.index.get());
        let mut values = vec![None; self.parameter_count];
        for parameter in parameters {
            let offset = parameter.index.get() - 1;
            let slot = values.get_mut(offset).ok_or_else(|| {
                RdbcError::InvalidArgument(format!(
                    "parameter {} exceeds prepared width {}",
                    parameter.index.get(),
                    self.parameter_count
                ))
            })?;
            *slot = Some(parameter.value);
        }
        values
            .into_iter()
            .enumerate()
            .map(|(offset, value)| {
                value.ok_or_else(|| {
                    RdbcError::InvalidArgument(format!("parameter {} has not been set", offset + 1))
                })
            })
            .collect()
    }
}

druid_rdbc::impl_standard_wrapper!(SqlxPreparedStatement => PreparedStatement);

#[async_trait::async_trait]
impl druid_rdbc::statement::Statement for SqlxPreparedStatement {
    fn options(&self) -> Result<StatementOptions, RdbcError> {
        self.actor()?;
        Ok(self.options)
    }

    fn max_field_size(&self) -> Result<i32, RdbcError> {
        self.actor()?;
        Ok(self.max_field_size.load(Ordering::Acquire))
    }

    fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError> {
        self.actor()?;
        if max < 0 {
            return Err(RdbcError::InvalidArgument(
                "max field size must not be negative".to_owned(),
            ));
        }
        self.max_field_size.store(max, Ordering::Release);
        Ok(())
    }

    fn max_rows(&self) -> Result<i64, RdbcError> {
        self.actor()?;
        Ok(self.max_rows.load(Ordering::Acquire))
    }

    fn set_max_rows(&mut self, max: i64) -> Result<(), RdbcError> {
        self.actor()?;
        if max < 0 {
            return Err(RdbcError::InvalidArgument(
                "max rows must not be negative".to_owned(),
            ));
        }
        self.max_rows.store(max, Ordering::Release);
        Ok(())
    }

    fn set_escape_processing(&mut self, _enabled: bool) -> Result<(), RdbcError> {
        self.actor()?;
        Ok(())
    }

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

    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        self.actor()?;
        *self
            .fetch_direction
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = direction;
        Ok(())
    }

    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.actor()?;
        Ok(*self
            .fetch_direction
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.actor()?;
        if rows < 0 {
            return Err(RdbcError::InvalidArgument(
                "fetch size must not be negative".to_owned(),
            ));
        }
        self.fetch_size.store(rows, Ordering::Release);
        Ok(())
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.actor()?;
        Ok(self.fetch_size.load(Ordering::Acquire))
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
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        let actor = Arc::clone(self.actor()?);
        let rows_affected = self
            .execute_update_values(sql, Vec::new(), &generated_keys)
            .await?;
        actor.task4_trace_statement_update();
        Ok(rows_affected)
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        let actor = Arc::clone(self.actor()?);
        if actor.statement_returns_rows(sql).await? {
            self.last_update_count.store(-1, Ordering::Release);
            Ok(vec![ExecutionResult::ResultSet(
                self.bounded(actor.execute_query(sql, Vec::new())).await?,
            )])
        } else {
            self.execute_update(sql, generated_keys)
                .await
                .map(|count| vec![ExecutionResult::UpdateCount(count)])
        }
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.actor()?;
        self.statement_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(sql);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        self.statement_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }

    fn batch(&self) -> Result<Vec<String>, RdbcError> {
        self.actor()?;
        Ok(self
            .statement_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.actor()?;
        let batch = std::mem::take(
            &mut *self
                .statement_batch
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let mut counts = Vec::with_capacity(batch.len());
        for sql in batch {
            match self
                .execute_update_values(&sql, Vec::new(), &GeneratedKeys::None)
                .await
            {
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

    async fn generated_keys(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.actor()?;
        let key = self
            .generated_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let rows = key
            .map(|value| vec![druid_rdbc::types::Row::new(vec![Value::Int(value)])])
            .unwrap_or_default();
        Ok(Box::new(BufferedRowSet::with_column_labels(
            rows,
            vec!["GENERATED_KEY".to_owned()],
        )))
    }

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.actor()?;
        let count = self.last_update_count.load(Ordering::Acquire);
        Ok((count >= 0).then_some(count))
    }

    fn set_poolable(&mut self, poolable: bool) -> Result<(), RdbcError> {
        self.actor()?;
        self.poolable.store(poolable, Ordering::Release);
        Ok(())
    }

    fn is_poolable(&self) -> Result<bool, RdbcError> {
        self.actor()?;
        Ok(self.poolable.load(Ordering::Acquire))
    }

    fn close_on_completion(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        self.close_on_completion.store(true, Ordering::Release);
        Ok(())
    }

    fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        self.actor()?;
        Ok(self.close_on_completion.load(Ordering::Acquire))
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.actor()?;
        Ok(None)
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        Ok(())
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
    fn sql(&self) -> Result<&str, RdbcError> {
        self.actor()?;
        Ok(&self.sql)
    }

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

    fn add_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        let values = self.parameter_state.values(self.parameter_count)?;
        self.prepared_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(values);
        Ok(())
    }

    fn add_parameter_batch(&mut self, parameters: Vec<PreparedParameter>) -> Result<(), RdbcError> {
        self.actor()?;
        let values = self.parameter_values(parameters)?;
        self.prepared_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(values);
        Ok(())
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        self.prepared_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
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
        self.actor()?;
        let values = self.parameter_state.values(self.parameter_count)?;
        self.execute_update_values(&self.sql, values, &GeneratedKeys::None)
            .await
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        let actor = Arc::clone(self.actor()?);
        let values = self.parameter_state.values(self.parameter_count)?;
        if actor.statement_returns_rows(&self.sql).await? {
            self.last_update_count.store(-1, Ordering::Release);
            Ok(vec![ExecutionResult::ResultSet(
                self.bounded(actor.execute_query(&self.sql, values)).await?,
            )])
        } else {
            self.execute_update_values(&self.sql, values, &GeneratedKeys::None)
                .await
                .map(|count| vec![ExecutionResult::UpdateCount(count)])
        }
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.actor()?;
        let parameter_sets = std::mem::take(
            &mut *self
                .prepared_batch
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let mut counts = Vec::with_capacity(parameter_sets.len());
        for values in parameter_sets {
            match self
                .execute_update_values(&self.sql, values, &GeneratedKeys::None)
                .await
            {
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

    fn result_set_meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.actor()?;
        Ok(Arc::clone(&self.result_set_meta_data) as Arc<dyn ResultSetMetaData>)
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        self.actor()?;
        Ok(Arc::new(PreparedParameterMetaDataSnapshot::unknown_inputs(
            self.parameter_count,
        )))
    }
}
