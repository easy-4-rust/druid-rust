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
    prepared_batch: Mutex<Vec<Vec<druid_rdbc::statement::PreparedInputParameter>>>,
    parameter_count: usize,
    result_set_meta_data: Arc<ResultSetMetaDataSnapshot>,
    /// 当前成功执行的完整生成键，批次完成后按执行顺序发布。
    generated_key: Mutex<Vec<i64>>,
    /// 对应 Java Connection#prepareStatement 的生成键请求，随缓存句柄保留。
    prepared_generated_keys: GeneratedKeys,
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
            generated_key: Mutex::new(Vec::new()),
            prepared_generated_keys: GeneratedKeys::None,
            last_update_count: AtomicI64::new(-1),
        }
    }

    /// 为已创建句柄保留调用方的准备请求；执行不得改用默认 None。
    pub(crate) fn with_generated_keys_request(mut self, generated_keys: GeneratedKeys) -> Self {
        self.prepared_generated_keys = generated_keys;
        self
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

    /// 在可能失败或挂起的执行步骤前使上一轮结果失效。
    /// 参数物化、驱动错误或调用取消均不得继续暴露旧生成键。
    fn reset_execution_results(&self) {
        *self
            .generated_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Vec::new();
        self.last_update_count.store(-1, Ordering::Release);
    }

    async fn execute_update_values(
        &self,
        sql: &str,
        values: Vec<Value>,
        generated_keys: &GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        let (count, key) = self
            .execute_update_values_result(sql, values, generated_keys)
            .await?;
        self.last_update_count.store(count, Ordering::Release);
        *self
            .generated_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = key;
        Ok(count)
    }

    /// 返回单次执行结果供 batch 暂存，避免批次中途把部分结果写入句柄。
    async fn execute_update_values_result(
        &self,
        sql: &str,
        values: Vec<Value>,
        generated_keys: &GeneratedKeys,
    ) -> Result<(i64, Vec<i64>), RdbcError> {
        if std::matches!(
            generated_keys,
            GeneratedKeys::ColumnIndexes(_) | GeneratedKeys::ColumnNames(_)
        ) {
            return Err(RdbcError::feature_not_supported(
                "sqlx_generated_keys_column_selection",
            ));
        }
        let actor = Arc::clone(self.actor()?);
        if std::matches!(generated_keys, GeneratedKeys::ReturnGeneratedKeys) {
            return self.bounded(actor.execute_update_keys(sql, values)).await;
        }
        let result = self
            .bounded(actor.execute_update_result(sql, values))
            .await?;
        let count = i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".to_owned())
        })?;
        Ok((count, Vec::new()))
    }

    /// 执行准备查询并更新本次生成键；无插入行时清除上次键。
    /// 对应 Java PreparedStatement#executeQuery 与 Statement#getGeneratedKeys。
    async fn execute_prepared_query_values(
        &self,
        actor: &Arc<SqlxConnectionActor>,
        values: Vec<Value>,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let (rows, key) = if matches!(
            self.prepared_generated_keys,
            GeneratedKeys::ReturnGeneratedKeys
        ) {
            self.bounded(actor.execute_query_with_generated_key(&self.sql, values))
                .await?
        } else {
            (
                self.bounded(actor.execute_query(&self.sql, values)).await?,
                Vec::new(),
            )
        };
        self.last_update_count.store(-1, Ordering::Release);
        *self
            .generated_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = key;
        Ok(rows)
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
        let keys = std::mem::take(
            &mut *self
                .generated_key
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let rows = keys
            .into_iter()
            .map(|value| druid_rdbc::types::Row::new(vec![Value::Int(value)]))
            .collect();
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
    fn set_input_parameter(
        &mut self,
        index: druid_rdbc::statement::ParameterIndex,
        parameter: druid_rdbc::statement::PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        let actor = Arc::clone(self.actor()?);
        let parameter = if actor.backend_key() == "sqlite"
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
            // SQLite 保留 BASE 同步绑定：底层保存读取后的值，执行及缓存复用不再次读流。
            let value = crate::prepared_parameter_materializer::PreparedParameterMaterializer::materialize_immediate(&parameter)?
                .ok_or_else(|| RdbcError::Other("synchronous stream binding requires a value".to_owned()))?;
            druid_rdbc::statement::PreparedInputParameter::RustValue(value)
        } else {
            parameter
        };
        self.parameter_state.set_input(index, parameter)?;
        actor.task4_trace_parameter_set();
        Ok(())
    }

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
        let values = self.parameter_state.inputs(self.parameter_count)?;
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
            .push(
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
        self.actor()?;
        parameters.sort_by_key(|(index, _)| index.get());
        if parameters.len() != self.parameter_count
            || parameters
                .iter()
                .enumerate()
                .any(|(offset, (index, _))| index.get() != offset + 1)
        {
            return Err(RdbcError::InvalidArgument(
                "prepared batch indexes must match contiguous prepared width".to_owned(),
            ));
        }
        self.prepared_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(parameters.into_iter().map(|(_, input)| input).collect());
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
        self.reset_execution_results();
        let actor = Arc::clone(self.actor()?);
        let values = self.parameter_state.values(self.parameter_count).await?;
        let result = self.execute_prepared_query_values(&actor, values).await?;
        actor.task4_trace_prepared_query();
        Ok(result)
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.reset_execution_results();
        self.actor()?;
        let values = self.parameter_state.values(self.parameter_count).await?;
        self.execute_update_values(&self.sql, values, &self.prepared_generated_keys)
            .await
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.reset_execution_results();
        let actor = Arc::clone(self.actor()?);
        let values = self.parameter_state.values(self.parameter_count).await?;
        if actor.statement_returns_rows(&self.sql).await? {
            Ok(vec![ExecutionResult::ResultSet(
                self.execute_prepared_query_values(&actor, values).await?,
            )])
        } else {
            self.execute_update_values(&self.sql, values, &self.prepared_generated_keys)
                .await
                .map(|count| vec![ExecutionResult::UpdateCount(count)])
        }
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.reset_execution_results();
        self.actor()?;
        let parameter_sets = std::mem::take(
            &mut *self
                .prepared_batch
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let mut counts = Vec::with_capacity(parameter_sets.len());
        let mut keys = Vec::with_capacity(parameter_sets.len());
        for inputs in parameter_sets {
            let execution = async {
                let values = PreparedParameterState::materialize(&inputs).await?;
                self.execute_update_values_result(&self.sql, values, &self.prepared_generated_keys)
                    .await
            };
            match execution.await {
                Ok((count, key)) => {
                    counts.push(count);
                    keys.extend(key);
                }
                Err(cause) => {
                    return Err(RdbcError::BatchUpdateException {
                        update_counts: counts.into(),
                        cause: Box::new(cause),
                    });
                }
            }
        }
        *self
            .generated_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = keys;
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
