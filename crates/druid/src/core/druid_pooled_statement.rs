//! Canonical pooled Statement proxy.

use super::druid_execution_filter_policy::DruidExecutionFilterPolicy;
use super::druid_pooled_result_set::StandardResultSetResource;
use super::filter::ExecOperation;
use super::standard_logical_policy::StandardLogicalPolicy;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registry::StandardResourceRegistry;
use super::statement_state_delegates::delegate_statement_state_methods;
use super::{
    DruidPooledResultSet, FilterChain, ProxyAttributeValue, ProxyAttributes, RdbcError,
    StatementExecuteType,
};
use crate::stats::StatsCollector;
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecResult, ExecutionResult, GeneratedKeys, Statement,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub(crate) struct DruidPooledStatementInner {
    pub(crate) connection_id: u64,
    pub(crate) id: u64,
    pub(crate) statement: tokio::sync::Mutex<Box<dyn Statement>>,
    pub(crate) resources: Arc<StandardResourceRegistry>,
    pub(crate) policy: Arc<StandardLogicalPolicy>,
    registration: OnceLock<super::standard_resource_registration::StandardResourceRegistration>,
    pub(crate) result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
    #[allow(dead_code)]
    pub(crate) metadata_id_seed: Arc<std::sync::atomic::AtomicU64>,
    attributes: ProxyAttributes,
    pub(crate) lease_active: Arc<AtomicBool>,
    pub(crate) filter_chain: Option<Arc<FilterChain>>,
    pub(crate) stats_collector: Option<Arc<StatsCollector>>,
    state: Mutex<DruidPooledStatementState>,
    result_sets: Mutex<Vec<std::sync::Weak<StandardResultSetResource>>>,
    result_sequence: Mutex<Option<StatementResultSequence>>,
}

struct DruidPooledStatementState {
    closed: bool,
    last_sql: Option<String>,
    last_execute_type: Option<StatementExecuteType>,
    last_execute_started_at: Option<Instant>,
    last_execute_elapsed: Option<Duration>,
    first_result_set: bool,
    fetch_row_peak: i32,
    exception_count: u64,
    update_count: i64,
    batch: Vec<String>,
}

pub(crate) enum TrackedStatementResult {
    ResultSet(std::sync::Weak<StandardResultSetResource>),
    UpdateCount(i64),
}

struct StatementResultSequence {
    results: Vec<TrackedStatementResult>,
    current: usize,
}

/// Logical Statement that owns one canonical driver statement.
#[derive(Clone)]
pub struct DruidPooledStatement {
    pub(crate) inner: Arc<DruidPooledStatementInner>,
}

impl DruidPooledStatement {
    /// Creates a canonical pooled statement.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_standard(
        statement: Box<dyn Statement>,
        resources: Arc<StandardResourceRegistry>,
        policy: Arc<StandardLogicalPolicy>,
        connection_id: u64,
        id: u64,
        result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
        metadata_id_seed: Arc<std::sync::atomic::AtomicU64>,
        lease_active: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<StatsCollector>>,
    ) -> Self {
        Self {
            inner: Arc::new(DruidPooledStatementInner {
                connection_id,
                id,
                statement: tokio::sync::Mutex::new(statement),
                resources,
                policy,
                registration: OnceLock::new(),
                result_set_id_seed,
                metadata_id_seed,
                attributes: ProxyAttributes::default(),
                lease_active,
                filter_chain,
                stats_collector,
                state: Mutex::new(DruidPooledStatementState {
                    closed: false,
                    last_sql: None,
                    last_execute_type: None,
                    last_execute_started_at: None,
                    last_execute_elapsed: None,
                    first_result_set: false,
                    fetch_row_peak: -1,
                    exception_count: 0,
                    update_count: -1,
                    batch: Vec::new(),
                }),
                result_sets: Mutex::new(Vec::new()),
                result_sequence: Mutex::new(None),
            }),
        }
    }

    pub(crate) fn from_inner(inner: Arc<DruidPooledStatementInner>) -> Self {
        Self { inner }
    }

    /// Returns whether two handles share one logical Statement identity.
    #[must_use]
    pub fn is_same_statement(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn configure_standard_query_timeout(&self, seconds: i32) -> Result<(), RdbcError> {
        self.inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .set_query_timeout(seconds)
    }

    /// Returns the Druid statement identifier.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.inner.id
    }

    #[must_use]
    pub fn attributes_size(&self) -> usize {
        self.inner.attributes.len()
    }

    pub fn clear_attributes(&self) {
        self.inner.attributes.clear();
    }

    #[must_use]
    pub fn attributes(&self) -> std::collections::HashMap<String, ProxyAttributeValue> {
        self.inner.attributes.snapshot()
    }

    #[must_use]
    pub fn attribute(&self, key: &str) -> Option<ProxyAttributeValue> {
        self.inner.attributes.get(key)
    }

    pub fn put_attribute(
        &self,
        key: impl Into<String>,
        value: ProxyAttributeValue,
    ) -> Option<ProxyAttributeValue> {
        self.inner.attributes.put(key, value)
    }

    #[must_use]
    pub fn fetch_row_peak(&self) -> i32 {
        self.state().fetch_row_peak
    }

    #[must_use]
    pub fn exception_count(&self) -> u64 {
        self.state().exception_count
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state().closed
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.is_closed() || !self.inner.lease_active.load(Ordering::Acquire) {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn begin_single_execution(&self, sql: &str, execute_type: StatementExecuteType) {
        let mut state = self.state_mut();
        state.last_sql = Some(sql.to_owned());
        state.last_execute_type = Some(execute_type);
        state.last_execute_started_at = Some(Instant::now());
        state.last_execute_elapsed = None;
        state.first_result_set = false;
        state.update_count = -1;
    }

    fn begin_batch_execution(&self) {
        let mut state = self.state_mut();
        state.last_execute_type = Some(StatementExecuteType::ExecuteBatch);
        state.last_execute_started_at = Some(Instant::now());
        state.last_execute_elapsed = None;
        state.first_result_set = false;
        state.update_count = -1;
    }

    /// Returns the last executed SQL.
    #[must_use]
    pub fn last_sql(&self) -> Option<String> {
        self.state().last_sql.clone()
    }

    #[must_use]
    pub fn last_execute_elapsed(&self) -> Option<Duration> {
        self.state().last_execute_elapsed
    }

    #[must_use]
    pub fn last_execute_type(&self) -> Option<StatementExecuteType> {
        self.state().last_execute_type
    }

    #[must_use]
    pub fn last_execute_start_elapsed(&self) -> Option<Duration> {
        self.state()
            .last_execute_started_at
            .map(|started_at| started_at.elapsed())
    }

    #[must_use]
    pub fn is_first_result_set(&self) -> bool {
        self.state().first_result_set
    }

    pub(crate) fn record_fetch_row_count(&self, count: i32) {
        let mut state = self.state_mut();
        state.fetch_row_peak = state.fetch_row_peak.max(count);
    }

    fn record_exception(&self) {
        let mut state = self.state_mut();
        state.exception_count = state.exception_count.saturating_add(1);
    }

    fn state(&self) -> std::sync::MutexGuard<'_, DruidPooledStatementState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn state_mut(&self) -> std::sync::MutexGuard<'_, DruidPooledStatementState> {
        self.state()
    }
}

impl DruidPooledStatementInner {
    pub(crate) fn track_result_set(&self, result_set: &Arc<StandardResultSetResource>) {
        let mut result_sets = self
            .result_sets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        result_sets.retain(|candidate| candidate.strong_count() > 0);
        result_sets.push(Arc::downgrade(result_set));
    }

    pub(crate) fn set_execution_results(&self, results: Vec<TrackedStatementResult>) {
        let (first_result_set, update_count) = match results.first() {
            Some(TrackedStatementResult::ResultSet(_)) => (true, -1),
            Some(TrackedStatementResult::UpdateCount(count)) => (false, *count),
            None => (false, -1),
        };
        *self
            .result_sequence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            (!results.is_empty()).then_some(StatementResultSequence {
                results,
                current: 0,
            });
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.first_result_set = first_result_set;
        state.update_count = update_count;
    }

    async fn dispose_result_sets(
        &self,
        disposition: CurrentResultDisposition,
        current: Option<std::sync::Weak<StandardResultSetResource>>,
    ) -> Result<(), RdbcError> {
        let targets = {
            let mut result_sets = self
                .result_sets
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            result_sets.retain(|candidate| candidate.strong_count() > 0);
            // Future non-exhaustive dispositions retain results by default, matching KeepCurrent.
            #[allow(clippy::match_same_arms)]
            match disposition {
                CurrentResultDisposition::KeepCurrent => Vec::new(),
                CurrentResultDisposition::CloseAll => result_sets
                    .iter()
                    .filter_map(std::sync::Weak::upgrade)
                    .collect(),
                CurrentResultDisposition::Default | CurrentResultDisposition::CloseCurrent => {
                    current
                        .and_then(|result_set| result_set.upgrade())
                        .into_iter()
                        .collect()
                }
                _ => Vec::new(),
            }
        };
        let mut first_error = None;
        for result_set in targets {
            if let Err(error) = result_set.close_core().await {
                first_error.get_or_insert(error);
            }
        }
        let mut result_sets = self
            .result_sets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        result_sets.retain(|candidate| candidate.strong_count() > 0);
        first_error.map_or(Ok(()), Err)
    }

    pub(crate) fn begin_standard_admission(&self) -> Result<StandardResourceAdmission, RdbcError> {
        self.resources
            .begin_admission(self.policy.resource_generation())
    }

    async fn close_standard_core(&self) -> Result<(), RdbcError> {
        if self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
        {
            self.unregister_resource();
            return Ok(());
        }
        let mut statement = self.statement.lock().await;
        if self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
        {
            self.unregister_resource();
            return Ok(());
        }
        statement.close().await?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
        drop(statement);
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.statement_stat().increment_statement_close_counter();
        }
        if let Some(filter_chain) = &self.filter_chain {
            filter_chain.after_statement_close_with_identity(self.connection_id, self.id)?;
        }
        self.unregister_resource();
        Ok(())
    }

    pub(crate) fn finish_logical_close(&self) -> Result<(), RdbcError> {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.closed {
                self.unregister_resource();
                return Ok(());
            }
            state.closed = true;
        }
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.statement_stat().increment_statement_close_counter();
        }
        if let Some(filter_chain) = &self.filter_chain {
            filter_chain
                .after_statement_close_with_identity(self.connection_id, self.id)
                .map_err(RdbcError::from)?;
        }
        self.unregister_resource();
        Ok(())
    }
}

#[async_trait::async_trait]
impl StandardResourceHandle for DruidPooledStatementInner {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        self.close_standard_core().await
    }

    fn is_resource_closed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
    }

    fn registration_slot(
        &self,
    ) -> &OnceLock<super::standard_resource_registration::StandardResourceRegistration> {
        &self.registration
    }
}

druid_rdbc::impl_standard_wrapper!(DruidPooledStatement => Statement);

#[async_trait::async_trait]
impl Statement for DruidPooledStatement {
    delegate_statement_state_methods!(inner.statement, "statement is busy");

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        self.inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .query_timeout()
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.configure_standard_query_timeout(seconds)
    }

    async fn execute_query(
        &mut self,
        sql: &str,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.ensure_open()?;
        let admission = self.inner.begin_standard_admission()?;
        self.begin_single_execution(sql, StatementExecuteType::ExecuteQuery);
        let started = Instant::now();
        let inner = Arc::clone(&self.inner);
        let raw = self
            .inner
            .policy
            .execute(
                sql,
                Vec::new(),
                self.id(),
                ExecOperation::Query,
                move |effective_sql| async move {
                    inner
                        .statement
                        .lock()
                        .await
                        .execute_query(&effective_sql)
                        .await
                },
                |_| ExecResult::default(),
            )
            .await;
        let raw = match raw {
            Ok(raw) => raw,
            Err(error) => {
                self.record_exception();
                return Err(error);
            }
        };
        self.state_mut().last_execute_elapsed = Some(started.elapsed());
        Ok(Box::new(
            DruidPooledResultSet::new_standard(Arc::clone(&self.inner), raw, &admission).await?,
        ))
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        self.begin_single_execution(sql, StatementExecuteType::ExecuteUpdate);
        let started = Instant::now();
        let inner = Arc::clone(&self.inner);
        let result = self
            .inner
            .policy
            .execute(
                sql,
                Vec::new(),
                self.id(),
                ExecOperation::Update,
                move |effective_sql| async move {
                    inner
                        .statement
                        .lock()
                        .await
                        .execute_update(&effective_sql, generated_keys)
                        .await
                },
                |count| ExecResult {
                    rows_affected: u64::try_from(*count).unwrap_or_default(),
                    ..ExecResult::default()
                },
            )
            .await;
        match result {
            Ok(count) => {
                let mut state = self.state_mut();
                state.last_execute_elapsed = Some(started.elapsed());
                state.update_count = count;
                Ok(count)
            }
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        let admission = self.inner.begin_standard_admission()?;
        self.begin_single_execution(sql, StatementExecuteType::Execute);
        let inner = Arc::clone(&self.inner);
        let results = self
            .inner
            .policy
            .execute(
                sql,
                Vec::new(),
                self.id(),
                ExecOperation::Execute,
                move |effective_sql| async move {
                    inner
                        .statement
                        .lock()
                        .await
                        .execute(&effective_sql, generated_keys)
                        .await
                },
                |results| match results.first() {
                    Some(ExecutionResult::UpdateCount(count)) => ExecResult {
                        rows_affected: u64::try_from(*count).unwrap_or_default(),
                        ..ExecResult::default()
                    },
                    Some(ExecutionResult::ResultSet(_)) => ExecResult {
                        row_count: Some(0),
                        ..ExecResult::default()
                    },
                    None => ExecResult::default(),
                },
            )
            .await;
        let results = match results {
            Ok(results) => results,
            Err(error) => {
                self.record_exception();
                return Err(error);
            }
        };
        DruidPooledResultSet::new_standard_results(Arc::clone(&self.inner), results, &admission)
            .await
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let effective_sql = self.inner.policy.classify(
            DruidExecutionFilterPolicy::statement_add_batch_sql(
                self.inner.filter_chain.as_deref(),
                &sql,
            )
            .map_err(RdbcError::from),
            Some(&sql),
        )?;
        let result = self
            .inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .add_batch(effective_sql.clone());
        self.inner.policy.classify(result, Some(&effective_sql))?;
        self.state_mut().batch.push(effective_sql);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .clear_batch();
        self.inner.policy.classify(result, None)?;
        self.state_mut().batch.clear();
        Ok(())
    }

    fn batch(&self) -> Result<Vec<String>, RdbcError> {
        self.ensure_open()?;
        Ok(self.state().batch.clone())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.ensure_open()?;
        self.begin_batch_execution();
        let statements = self.state().batch.clone();
        let sql = statements.join("\n;\n");
        let inner = Arc::clone(&self.inner);
        let result = self
            .inner
            .policy
            .execute_statement_batch(&sql, &statements, self.id(), move || async move {
                inner.statement.lock().await.execute_batch().await
            })
            .await;
        match result {
            Ok(counts) => {
                let mut state = self.state_mut();
                state.batch.clear();
                state.update_count = if counts.len() == 1 { counts[0] } else { -1 };
                Ok(counts)
            }
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    async fn generated_keys(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.ensure_open()?;
        let admission = self.inner.begin_standard_admission()?;
        let raw = self.inner.statement.lock().await.generated_keys().await?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard(Arc::clone(&self.inner), raw, &admission).await?,
        ))
    }

    async fn get_result_set(
        &mut self,
    ) -> Result<Option<Box<dyn druid_rdbc::result_set::ResultSet>>, RdbcError> {
        self.ensure_open()?;
        let admission = self.inner.begin_standard_admission()?;
        let raw = self.inner.statement.lock().await.get_result_set().await?;
        match raw {
            Some(raw) => Ok(Some(Box::new(
                DruidPooledResultSet::new_standard(Arc::clone(&self.inner), raw, &admission)
                    .await?,
            ))),
            None => Ok(None),
        }
    }

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.ensure_open()?;
        let count = self.state().update_count;
        Ok((count >= 0).then_some(count))
    }

    async fn get_more_results(
        &mut self,
        disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let (has_sequence, current) = {
            let sequence = self
                .inner
                .result_sequence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current = sequence.as_ref().and_then(|sequence| {
                match sequence.results.get(sequence.current) {
                    Some(TrackedStatementResult::ResultSet(result_set)) => Some(result_set.clone()),
                    Some(TrackedStatementResult::UpdateCount(_)) | None => None,
                }
            });
            (sequence.is_some(), current)
        };
        self.inner.dispose_result_sets(disposition, current).await?;
        if has_sequence {
            let (has_more, update_count) = {
                let mut sequence_guard = self
                    .inner
                    .result_sequence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let Some(sequence) = sequence_guard.as_mut() else {
                    return Ok(false);
                };
                sequence.current = sequence.current.saturating_add(1);
                let next = match sequence.results.get(sequence.current) {
                    Some(TrackedStatementResult::ResultSet(_)) => Some((true, -1)),
                    Some(TrackedStatementResult::UpdateCount(count)) => Some((false, *count)),
                    None => None,
                };
                if let Some(next) = next {
                    next
                } else {
                    *sequence_guard = None;
                    (false, -1)
                }
            };
            let mut state = self.state_mut();
            state.first_result_set = has_more;
            state.update_count = update_count;
            return Ok(has_more);
        }
        let has_more = self
            .inner
            .statement
            .lock()
            .await
            .get_more_results(disposition)
            .await?;
        let mut state = self.state_mut();
        state.first_result_set = has_more;
        state.update_count = -1;
        Ok(has_more)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.inner.close_standard_core().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        if self.is_closed() || !self.inner.lease_active.load(Ordering::Acquire) {
            return Ok(true);
        }
        self.inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .is_closed()
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        let result = self
            .inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .warnings();
        self.inner
            .policy
            .classify(result, self.last_sql().as_deref())
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
        let result = self
            .inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .clear_warnings();
        self.inner
            .policy
            .classify(result, self.last_sql().as_deref())
    }
}

impl std::fmt::Debug for DruidPooledStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledStatement")
            .field("id", &self.id())
            .field("closed", &self.is_closed())
            .field("fetch_row_peak", &self.fetch_row_peak())
            .field("exception_count", &self.exception_count())
            .field("update_count", &self.state().update_count)
            .finish()
    }
}
