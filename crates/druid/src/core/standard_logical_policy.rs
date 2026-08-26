//! Canonical standard proxies 共享的 Druid Filter/Stat/error policy。

use super::druid_execution_filter_policy::DruidExecutionFilterPolicy;
use super::exception_sorter::ExceptionSorter;
use super::fatal_error_handler::FatalErrorHandler;
use super::filter::{ExecContext, ExecOperation};
use super::{
    BatchExecContext, BatchExecKind, DruidError, ExecResult, FilterChain, PreparedInputParameter,
    RdbcError, Value,
};
use crate::stats::{RdbcStatementStat, StatsCollector};
use parking_lot::{Mutex, RwLock};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// 同一 logical lease 的 canonical execution policy actor。
pub(crate) struct StandardLogicalPolicy {
    connection_id: u64,
    data_source: String,
    lease_active: Arc<AtomicBool>,
    execution_running: Arc<AtomicBool>,
    filter_chain: Option<Arc<FilterChain>>,
    stats_collector: RwLock<Option<Arc<StatsCollector>>>,
    exception_sorter: RwLock<Option<Arc<dyn ExceptionSorter>>>,
    fatal_error_handler: RwLock<Option<Arc<dyn FatalErrorHandler>>>,
    auto_commit: RwLock<Option<bool>>,
    transaction_started_at: Mutex<Option<Instant>>,
    resource_generation: u64,
    discarded: AtomicBool,
}

impl StandardLogicalPolicy {
    pub(crate) fn new(
        connection_id: u64,
        data_source: String,
        lease_active: Arc<AtomicBool>,
        execution_running: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
        auto_commit: Option<bool>,
        resource_generation: u64,
    ) -> Arc<Self> {
        Arc::new(Self {
            connection_id,
            data_source,
            lease_active,
            execution_running,
            filter_chain,
            stats_collector: RwLock::new(None),
            exception_sorter: RwLock::new(None),
            fatal_error_handler: RwLock::new(None),
            auto_commit: RwLock::new(auto_commit),
            transaction_started_at: Mutex::new(None),
            resource_generation,
            discarded: AtomicBool::new(false),
        })
    }

    pub(crate) fn set_stats_collector(&self, collector: Arc<StatsCollector>) {
        *self.stats_collector.write() = Some(collector);
    }

    pub(crate) fn set_exception_sorter(&self, sorter: Arc<dyn ExceptionSorter>) {
        *self.exception_sorter.write() = Some(sorter);
    }

    pub(crate) fn set_fatal_error_handler(&self, handler: Arc<dyn FatalErrorHandler>) {
        *self.fatal_error_handler.write() = Some(handler);
    }

    pub(crate) fn is_discarded(&self) -> bool {
        self.discarded.load(Ordering::Acquire)
    }

    pub(crate) const fn resource_generation(&self) -> u64 {
        self.resource_generation
    }

    pub(crate) fn set_auto_commit_state(&self, auto_commit: bool) {
        *self.auto_commit.write() = Some(auto_commit);
        if auto_commit {
            self.transaction_started_at.lock().take();
        }
    }

    pub(crate) fn record_transaction_boundary(&self, commit: bool) {
        let elapsed = self
            .transaction_started_at
            .lock()
            .take()
            .map(|started_at| started_at.elapsed());
        if let Some(stats) = self.stats_collector.read().as_ref() {
            if commit {
                stats.record_commit(elapsed);
            } else {
                stats.record_rollback(elapsed);
            }
        }
    }

    pub(crate) fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.lease_active.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(RdbcError::ConnectionDiscarded)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn execute<T, F, Fut, S>(
        &self,
        sql: &str,
        params: Vec<Value>,
        statement_id: u64,
        operation: ExecOperation,
        raw: F,
        summarize: S,
    ) -> Result<T, RdbcError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<T, RdbcError>>,
        S: Fn(&T) -> ExecResult,
    {
        self.execute_with_parameter_context(
            sql,
            params,
            None,
            statement_id,
            operation,
            raw,
            summarize,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn execute_prepared<T, F, Fut, S>(
        &self,
        sql: &str,
        prepared_parameters: Vec<PreparedInputParameter>,
        statement_id: u64,
        operation: ExecOperation,
        raw: F,
        summarize: S,
    ) -> Result<T, RdbcError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<T, RdbcError>>,
        S: Fn(&T) -> ExecResult,
    {
        let params = prepared_parameters
            .iter()
            .map(|parameter| parameter.value.clone())
            .collect();
        self.execute_with_parameter_context(
            sql,
            params,
            Some(prepared_parameters),
            statement_id,
            operation,
            raw,
            summarize,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_with_parameter_context<T, F, Fut, S>(
        &self,
        sql: &str,
        params: Vec<Value>,
        prepared_parameters: Option<Vec<PreparedInputParameter>>,
        statement_id: u64,
        operation: ExecOperation,
        raw: F,
        summarize: S,
    ) -> Result<T, RdbcError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<T, RdbcError>>,
        S: Fn(&T) -> ExecResult,
    {
        self.ensure_open()?;
        let _execution = StandardExecutionGuard::new(
            Arc::clone(&self.execution_running),
            self.stats_collector
                .read()
                .as_ref()
                .map(|collector| Arc::clone(collector.statement_stat())),
        );
        let started = Instant::now();
        let mut context = ExecContext {
            connection_id: self.connection_id,
            statement_id: Some(statement_id),
            sql: sql.to_owned(),
            params: &params,
            prepared_parameters: prepared_parameters.as_deref(),
            data_source: &self.data_source,
            start: started,
            fingerprint: None,
            in_transaction: *self.auto_commit.read() == Some(false),
            operation,
        };
        let around = DruidExecutionFilterPolicy::execute(
            self.filter_chain.as_deref(),
            &mut context,
            raw,
            summarize,
        )
        .await;
        let (effective_sql, result, after) = match around {
            Ok(outcome) => outcome,
            Err(error) => {
                return self.classify(Err(RdbcError::from(error)), Some(&context.sql));
            }
        };
        let result = self.classify(result, Some(&effective_sql));
        self.record_execution(&effective_sql, &result);
        self.classify(after.map_err(RdbcError::from), Some(&effective_sql))?;
        result
    }

    pub(crate) async fn execute_statement_batch<F, Fut>(
        &self,
        sql: &str,
        statements: &[String],
        statement_id: u64,
        raw: F,
    ) -> Result<Vec<i64>, RdbcError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<i64>, RdbcError>>,
    {
        self.execute_batch_counts(
            sql,
            statements,
            &[],
            None,
            BatchExecKind::Statement,
            statement_id,
            raw,
        )
        .await
    }

    pub(crate) async fn execute_prepared_batch<F, Fut>(
        &self,
        sql: &str,
        prepared_parameter_sets: &[Vec<PreparedInputParameter>],
        statement_id: u64,
        raw: F,
    ) -> Result<Vec<i64>, RdbcError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<i64>, RdbcError>>,
    {
        self.ensure_open()?;
        let parameter_sets = prepared_parameter_sets
            .iter()
            .map(|parameters| {
                parameters
                    .iter()
                    .map(|parameter| parameter.value.clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        self.execute_batch_counts(
            sql,
            &[],
            &parameter_sets,
            Some(prepared_parameter_sets),
            BatchExecKind::PreparedStatement,
            statement_id,
            raw,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_batch_counts<F, Fut>(
        &self,
        sql: &str,
        statements: &[String],
        parameter_sets: &[Vec<Value>],
        prepared_parameter_sets: Option<&[Vec<PreparedInputParameter>]>,
        kind: BatchExecKind,
        statement_id: u64,
        raw: F,
    ) -> Result<Vec<i64>, RdbcError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<i64>, RdbcError>>,
    {
        self.ensure_open()?;
        let _execution = StandardExecutionGuard::new(
            Arc::clone(&self.execution_running),
            self.stats_collector
                .read()
                .as_ref()
                .map(|collector| Arc::clone(collector.statement_stat())),
        );
        let mut context = BatchExecContext {
            connection_id: self.connection_id,
            statement_id: Some(statement_id),
            sql,
            statements,
            parameter_sets,
            prepared_parameter_sets,
            kind,
            data_source: &self.data_source,
            start: Instant::now(),
            fingerprint: None,
            in_transaction: *self.auto_commit.read() == Some(false),
        };
        let around = DruidExecutionFilterPolicy::execute_batch(
            self.filter_chain.as_deref(),
            &mut context,
            raw,
            |counts| {
                counts
                    .iter()
                    .map(|count| i32::try_from(*count).unwrap_or(i32::MAX))
                    .collect()
            },
        )
        .await;
        let (result, after) = match around {
            Ok(outcome) => outcome,
            Err(error) => return self.classify(Err(error.into()), Some(sql)),
        };
        let result = self.classify(result, Some(sql));
        self.record_execution(sql, &result);
        self.classify(after.map_err(RdbcError::from), Some(sql))?;
        result
    }

    pub(crate) fn classify<T>(
        &self,
        result: Result<T, RdbcError>,
        sql: Option<&str>,
    ) -> Result<T, RdbcError> {
        if let Err(error) = &result {
            let fatal = self
                .exception_sorter
                .read()
                .as_ref()
                .zip(error.sql_exception())
                .is_some_and(|(sorter, exception)| sorter.is_exception_fatal(exception));
            if fatal {
                self.discarded.store(true, Ordering::Release);
                if let Some(handler) = self.fatal_error_handler.read().as_ref() {
                    let product_error = DruidError::from(error.clone());
                    if handler.handle_fatal_error(&product_error, sql) {
                        handler.request_fatal_error_refill();
                    }
                }
            }
        }
        result
    }

    fn record_execution<T>(&self, sql: &str, result: &Result<T, RdbcError>) {
        let Some(stats) = self.stats_collector.read().clone() else {
            return;
        };
        if *self.auto_commit.read() == Some(false) {
            let mut started_at = self.transaction_started_at.lock();
            if started_at.is_none() {
                *started_at = Some(Instant::now());
                stats.record_start_transaction();
            }
        }
        stats.record_execute_result(result.is_ok());
        if let Some(entry) = stats.connection_stat().entry(self.connection_id) {
            entry.set_last_sql(Some(sql.to_owned()));
            if let Err(error) = result {
                entry.error(error);
            }
        }
        if let Err(error) = result {
            stats.statement_stat().error(error);
        }
    }
}

struct StandardExecutionGuard {
    execution_running: Arc<AtomicBool>,
    statement_stat: Option<Arc<RdbcStatementStat>>,
    started_at: Instant,
}

impl StandardExecutionGuard {
    fn new(
        execution_running: Arc<AtomicBool>,
        statement_stat: Option<Arc<RdbcStatementStat>>,
    ) -> Self {
        execution_running.store(true, Ordering::Release);
        if let Some(stat) = statement_stat.as_ref() {
            stat.before_execute();
        }
        Self {
            execution_running,
            statement_stat,
            started_at: Instant::now(),
        }
    }
}

impl Drop for StandardExecutionGuard {
    fn drop(&mut self) {
        if let Some(stat) = self.statement_stat.as_ref() {
            stat.after_execute(self.started_at.elapsed());
        }
        self.execution_running.store(false, Ordering::Release);
    }
}
