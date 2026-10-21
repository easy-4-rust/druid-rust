//! Canonical SQLx Statement and PreparedStatement actor.

use super::sqlx_connection_actor::SqlxConnectionActor;
use crate::prepared_parameter_meta_data_snapshot::PreparedParameterMetaDataSnapshot;
use crate::prepared_parameter_state::PreparedParameterState;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::metadata::{ParameterMetaData, ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::statement::{
    ExecutionResult, FetchDirection, GeneratedKeys, PreparedParameter, ResultSetType,
    StatementCancellation, StatementOptions,
};
use druid_rdbc::types::Value;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

mod prepared_statement;

mod statement;

struct CancellationState {
    generation: u64,
    sender: tokio::sync::watch::Sender<bool>,
    interrupted: Arc<AtomicBool>,
    pending_generation: Option<u64>,
    active: bool,
    closed: bool,
}

struct CancellationDomain {
    state: Mutex<CancellationState>,
}

impl CancellationDomain {
    fn new() -> Self {
        Self {
            state: Mutex::new(CancellationState {
                generation: 0,
                sender: tokio::sync::watch::channel(false).0,
                interrupted: Arc::new(AtomicBool::new(false)),
                pending_generation: None,
                active: false,
                closed: false,
            }),
        }
    }

    fn sender(&self) -> tokio::sync::watch::Sender<bool> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .sender
            .clone()
    }

    fn control(self: &Arc<Self>) -> Result<Arc<dyn StatementCancellation>, RdbcError> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let generation = if state.active || state.sender.receiver_count() > 0 {
            state.generation
        } else {
            state.generation.saturating_add(1)
        };
        Ok(Arc::new(SqlxStatementCancellation {
            domain: Arc::clone(self),
            generation,
        }))
    }

    fn begin_execution(self: &Arc<Self>) -> Result<ExecutionGeneration, RdbcError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let generation = state.generation.checked_add(1).ok_or_else(|| {
            RdbcError::InvalidArgument("statement execution generation overflow".to_owned())
        })?;
        state.sender.send_replace(true);
        state.interrupted.store(true, Ordering::Release);
        state.generation = generation;
        state.sender = tokio::sync::watch::channel(false).0;
        state.interrupted = Arc::new(AtomicBool::new(false));
        state.active = true;
        let cancelled = state.pending_generation == Some(generation);
        if state
            .pending_generation
            .is_some_and(|pending| pending <= generation)
        {
            state.pending_generation = None;
        }
        if cancelled {
            state.active = false;
            state.interrupted.store(true, Ordering::Release);
            Err(cancelled_exception())
        } else {
            Ok(ExecutionGeneration {
                domain: Arc::clone(self),
                generation,
            })
        }
    }

    fn cancel_generation(&self, generation: u64) -> Result<(), RdbcError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        if generation < state.generation {
            return Ok(());
        }
        if generation == state.generation {
            // 只操作当前仍在执行或持有活动游标的代次；完成代次的迟到请求为空操作。
            if state.active || state.sender.receiver_count() > 0 {
                state.interrupted.store(true, Ordering::Release);
            }
            if state.sender.receiver_count() > 0 {
                state.sender.send_replace(true);
            }
        } else if generation == state.generation.saturating_add(1) {
            state.pending_generation = Some(generation);
        }
        Ok(())
    }

    fn cancel_current_or_next(&self, can_interrupt_active: bool) -> Result<(), RdbcError> {
        let target = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.closed {
                return Err(RdbcError::ConnectionDiscarded);
            }
            let has_active_execution = state.active || state.sender.receiver_count() > 0;
            if has_active_execution && !can_interrupt_active {
                return Err(RdbcError::feature_not_supported(
                    "sqlx_statement_active_cancellation",
                ));
            }
            if has_active_execution {
                state.generation
            } else {
                state.generation.saturating_add(1)
            }
        };
        self.cancel_generation(target)
    }

    fn close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closed = true;
        state.pending_generation = None;
        state.sender.send_replace(true);
    }

    fn interruption(&self) -> Arc<AtomicBool> {
        Arc::clone(
            &self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .interrupted,
        )
    }

    fn finish_execution(&self, generation: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.generation == generation {
            state.active = false;
        }
    }
}

struct ExecutionGeneration {
    domain: Arc<CancellationDomain>,
    generation: u64,
}

impl Drop for ExecutionGeneration {
    fn drop(&mut self) {
        self.domain.finish_execution(self.generation);
    }
}

struct SqlxStatementCancellation {
    domain: Arc<CancellationDomain>,
    generation: u64,
}

#[async_trait::async_trait]
impl StatementCancellation for SqlxStatementCancellation {
    async fn cancel(&self) -> Result<(), RdbcError> {
        self.domain.cancel_generation(self.generation)
    }
}

pub(super) fn cancelled_exception() -> RdbcError {
    RdbcError::SqlException(Box::new(SqlException::new(
        0,
        Some("HY008".to_owned()),
        Some("statement execution cancelled".to_owned()),
    )))
}

pub struct SqlxPreparedStatement {
    sql: String,
    actor: Option<Arc<SqlxConnectionActor>>,
    closed: AtomicBool,
    cancellation: Arc<CancellationDomain>,
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
            cancellation: Arc::new(CancellationDomain::new()),
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
            .map_err(|_| {
                // 本地截止时间映射为标准超时异常，保留 SQLState 和可赋值父类型。
                // 驱动自身返回的异常由内层 Result 原样传播。对应 Java: Statement#setQueryTimeout。
                RdbcError::SqlException(Box::new(
                    SqlException::new(
                        0,
                        Some("HYT00".to_owned()),
                        Some("statement query timed out".to_owned()),
                    )
                    .with_class_name("java.sql.SQLTimeoutException")
                    .with_assignable_type("java.sql.SQLTransientException"),
                ))
            })?
    }

    /// 在可能失败或挂起的执行步骤前使上一轮结果失效。
    /// 参数物化、驱动错误或调用取消均不得继续暴露旧生成键。
    fn reset_execution_results(&mut self) -> Result<ExecutionGeneration, RdbcError> {
        self.actor()?;
        *self
            .generated_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Vec::new();
        self.last_update_count.store(-1, Ordering::Release);
        // 每次执行拥有独立代次；旧句柄不能操作新 Sender，准入处消费预取消。
        self.cancellation.begin_execution()
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
        let interrupted = self.cancellation.interruption();
        if std::matches!(generated_keys, GeneratedKeys::ReturnGeneratedKeys) {
            return actor
                .execute_update_keys(
                    sql,
                    values,
                    self.query_timeout.load(Ordering::Acquire),
                    interrupted,
                )
                .await;
        }
        let result = actor
            .execute_update_result(
                sql,
                values,
                self.query_timeout.load(Ordering::Acquire),
                interrupted,
            )
            .await?;
        let count = i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".to_owned())
        })?;
        Ok((count, Vec::new()))
    }

    /// 正 fetch hint 的前向查询使用按需游标；其他调用保留原缓冲行为。
    async fn execute_query_values(
        &self,
        actor: &Arc<SqlxConnectionActor>,
        sql: &str,
        values: Vec<Value>,
        allow_stream: bool,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let fetch_size = self.fetch_size.load(Ordering::Acquire);
        if allow_stream
            && fetch_size > 0
            && self.options.result_set_type() == ResultSetType::ForwardOnly
        {
            self.bounded(actor.execute_async_query(
                sql,
                values,
                self.options,
                fetch_size,
                self.cancellation.sender().subscribe(),
                self.cancellation.interruption(),
            ))
            .await
        } else {
            actor
                .execute_query_with_timeout(
                    sql,
                    values,
                    self.query_timeout.load(Ordering::Acquire),
                    self.cancellation.interruption(),
                )
                .await
        }
    }

    /// 执行准备查询并更新本次生成键；无插入行时清除上次键。
    /// 对应 Java PreparedStatement#executeQuery 与 Statement#getGeneratedKeys。
    async fn execute_prepared_query_values(
        &self,
        actor: &Arc<SqlxConnectionActor>,
        values: Vec<Value>,
        allow_stream: bool,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let (rows, key) = if matches!(
            self.prepared_generated_keys,
            GeneratedKeys::ReturnGeneratedKeys
        ) {
            actor
                .execute_query_with_generated_key(
                    &self.sql,
                    values,
                    self.query_timeout.load(Ordering::Acquire),
                    self.cancellation.interruption(),
                )
                .await?
        } else {
            (
                self.execute_query_values(actor, &self.sql, values, allow_stream)
                    .await?,
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

#[cfg(test)]
mod cancellation_domain_tests {
    use super::{CancellationDomain, RdbcError};
    use std::sync::Arc;

    #[test]
    fn pending_cancel_is_consumed_before_a_non_interruptible_backend_starts_query() {
        let domain = Arc::new(CancellationDomain::new());

        domain
            .cancel_current_or_next(false)
            .expect("a not-yet-started operation can be cancelled on every backend");
        let error = match domain.begin_execution() {
            Err(error) => error,
            Ok(_) => panic!("the cancelled generation must not start"),
        };

        assert!(matches!(error, RdbcError::SqlException(ref source)
            if source.sql_state() == Some("HY008")));
    }

    #[test]
    fn active_cancel_requires_a_backend_interrupt_handle() {
        let domain = Arc::new(CancellationDomain::new());
        let execution = domain
            .begin_execution()
            .expect("the execution generation must start");

        let error = domain
            .cancel_current_or_next(false)
            .expect_err("a backend without an interrupt handle cannot cancel active work");
        assert!(matches!(error, RdbcError::SqlException(ref source)
            if source.sql_state() == Some("0A000")));
        drop(execution);
    }
}
