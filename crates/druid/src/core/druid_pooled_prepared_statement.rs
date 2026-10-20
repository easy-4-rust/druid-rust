//! Canonical pooled PreparedStatement proxy.

mod identity;
mod prepared_batch;
mod shared_owner;

use super::filter::{ExecOperation, StatementPropertyEvent};
use super::optional_statement_state_delegates::delegate_optional_statement_state_methods;
use super::prepared_statement_cache_stats::PreparedStatementCacheStats;
use super::prepared_statement_key::PreparedStatementKey;
use super::standard_prepared_statement_cache::StandardPreparedStatementCache;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_registry::StandardResourceRegistry;
use super::statement_state_delegates::{
    delegate_cached_statement_property_reads, delegate_statement_state_methods,
};
use super::{DruidPooledResultSet, DruidPooledStatement, ExecResult, FilterChain, RdbcError};
use crate::stats::StatsCollector;
use druid_rdbc::metadata::{ParameterMetaData, ResultSetMetaData};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, GeneratedKeys, PreparedInputParameter,
    PreparedParameter, PreparedStatement, Statement,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod cancellation_control_tests;

struct PreparedStatementResource {
    cache_stats: Arc<PreparedStatementCacheStats>,
    cache_entry: bool,
    statement: tokio::sync::Mutex<Option<Box<dyn PreparedStatement>>>,
    closed: AtomicBool,
    cancellation_gate: tokio::sync::Mutex<bool>,
}

impl PreparedStatementResource {
    fn new(
        statement: Box<dyn PreparedStatement>,
        cache_stats: Arc<PreparedStatementCacheStats>,
        cache_entry: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            cache_stats,
            cache_entry,
            statement: tokio::sync::Mutex::new(Some(statement)),
            closed: AtomicBool::new(false),
            cancellation_gate: tokio::sync::Mutex::new(true),
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
        let mut cancellation_gate = self.cancellation_gate.lock().await;
        *cancellation_gate = false;
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut statement = self.statement.lock().await;
        let Some(raw) = statement.as_mut() else {
            self.closed.store(true, Ordering::Release);
            return Ok(());
        };
        raw.close().await?;
        // 只有 registry 仍持有 raw 时在此计数；正常关闭/入缓存已 take 的路径不会重复。
        if self.cache_entry {
            self.cache_stats.record_checked_out_cache_delete();
        } else {
            self.cache_stats.record_close();
        }
        statement.take();
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}

struct PreparedStatementCancellation {
    resource: std::sync::Weak<PreparedStatementResource>,
    control: Arc<dyn druid_rdbc::statement::StatementCancellation>,
}

#[async_trait::async_trait]
impl druid_rdbc::statement::StatementCancellation for PreparedStatementCancellation {
    async fn cancel(&self) -> Result<(), RdbcError> {
        let resource = self
            .resource
            .upgrade()
            .ok_or(RdbcError::ConnectionDiscarded)?;
        let cancellation_gate = resource.cancellation_gate.lock().await;
        if !*cancellation_gate {
            return Err(RdbcError::ConnectionDiscarded);
        }
        resource.ensure_open()?;
        self.control.cancel().await
    }
}

/// Concrete Statement view over the canonical prepared handle.
struct PreparedStatementView {
    resource: Arc<PreparedStatementResource>,
}

impl PreparedStatementView {
    fn ensure_open(&self) -> Result<(), RdbcError> {
        self.resource.ensure_open()
    }
}

druid_rdbc::impl_standard_wrapper!(PreparedStatementView => Statement);

#[async_trait::async_trait]
impl Statement for PreparedStatementView {
    /// 检查资源后验证 JDBC 整数处置；返回下个结果是否为游标。
    async fn get_more_results_current(&mut self, current: i32) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.get_more_results(CurrentResultDisposition::try_from(current)?)
            .await
    }

    delegate_optional_statement_state_methods!(resource.statement, "prepared statement is busy");

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

    fn cancellation_control(
        &self,
    ) -> Result<Arc<dyn druid_rdbc::statement::StatementCancellation>, RdbcError> {
        self.resource.ensure_open()?;
        let control = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .cancellation_control()?;
        Ok(Arc::new(PreparedStatementCancellation {
            resource: Arc::downgrade(&self.resource),
            control,
        }))
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

    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .get_result_set()
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

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .get_update_count()
    }

    async fn get_more_results(
        &mut self,
        disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .get_more_results(disposition)
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

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.resource.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .warnings()
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
    parameters: Vec<Option<PreparedInputParameter>>,
    batch_parameter_sets: Vec<Vec<PreparedInputParameter>>,
}

/// Druid logical proxy over one canonical driver PreparedStatement.
#[derive(Clone)]
pub struct DruidPooledPreparedStatement {
    statement_base: DruidPooledStatement,
    shared: Arc<shared_owner::PreparedStatementSharedOwner>,
}

impl DruidPooledPreparedStatement {
    /// Wraps one owned canonical PreparedStatement.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new_standard(
        statement: Box<dyn PreparedStatement>,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
        key: PreparedStatementKey,
        cache: Arc<tokio::sync::Mutex<StandardPreparedStatementCache>>,
        cache_stats: Arc<PreparedStatementCacheStats>,
        cache_hit: bool,
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
        let resource =
            PreparedStatementResource::new(statement, Arc::clone(&cache_stats), cache_hit);
        let statement_base = DruidPooledStatement::new_standard(
            Box::new(PreparedStatementView {
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
        statement_base.inner.enable_statement_events();
        resources
            .register(admission, Arc::clone(&statement_base.inner))
            .await?;
        // 注册后再读取属性：即使 getter 失败或调用者取消，registry 仍持有关闭责任。
        let properties = {
            let raw = resource.statement.lock().await;
            super::prepared_statement_property_snapshot::PreparedStatementPropertySnapshot::capture(
                raw.as_ref().ok_or(RdbcError::ConnectionDiscarded)?.as_ref(),
            )
        };
        let initial_properties = match properties {
            Ok(properties) => properties,
            Err(error) => {
                let error = statement_base
                    .inner
                    .policy
                    .classify::<()>(Err(error), Some(sql))
                    .unwrap_err();
                let registration =
                    super::standard_resource_handle::StandardResourceHandle::registration_slot(
                        statement_base.inner.as_ref(),
                    )
                    .get()
                    .cloned()
                    .ok_or(RdbcError::ConnectionDiscarded)?;
                let _ = resources
                    .close_registered_after_admission(&[registration], admission)
                    .await;
                return Err(error);
            }
        };
        statement_base.initialize_prepared_properties(&initial_properties);
        let shared = Arc::new(shared_owner::PreparedStatementSharedOwner {
            statement_base: statement_base.clone(),
            resource,
            sql: sql.to_owned(),
            key: Mutex::new(Some(key)),
            cache: Mutex::new(Some(cache)),
            cache_stats,
            cache_entry: cache_hit,
            cacheable: AtomicBool::new(true),
            has_execution_result: AtomicBool::new(false),
            initial_properties,
            state: Mutex::new(PreparedStatementState {
                parameters: Vec::new(),
                batch_parameter_sets: Vec::new(),
            }),
            close_lock: tokio::sync::Mutex::new(()),
        });
        let owner: Arc<dyn super::logical_statement_owner::LogicalStatementOwner> = shared.clone();
        statement_base.install_logical_owner(Arc::downgrade(&owner));
        Ok(Self {
            statement_base,
            shared,
        })
    }

    /// Returns whether the logical statement is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        // close 失败后的逻辑完成状态独立于底层释放与 fatal 租约失效。
        self.statement_base.is_closed()
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
        if result.is_err() {
            self.shared.cacheable.store(false, Ordering::Release);
        }
        self.statement_base
            .inner
            .policy
            .classify(result, Some(&self.shared.sql))
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.is_closed()
            || !self
                .statement_base
                .inner
                .lease_active
                .load(Ordering::Acquire)
        {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            self.shared.resource.ensure_open()
        }
    }

    fn parameter_snapshot(&self) -> Result<Vec<PreparedInputParameter>, RdbcError> {
        self.shared
            .state
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

    fn retain_cacheable_result<T>(&self, result: Result<T, RdbcError>) -> Result<T, RdbcError> {
        if result.is_err() {
            self.shared.cacheable.store(false, Ordering::Release);
        }
        result
    }

    /// 在参数校验、Filter 和任何 await 前使当前租约的旧执行结果失效。
    fn begin_execution_result(&self) {
        self.shared
            .has_execution_result
            .store(false, Ordering::Release);
    }

    /// 只有完成整个逻辑执行后才允许读取底层生成键。
    fn complete_execution_result<T>(&self, result: Result<T, RdbcError>) -> Result<T, RdbcError> {
        self.shared
            .has_execution_result
            .store(result.is_ok(), Ordering::Release);
        self.retain_cacheable_result(result)
    }

    async fn close_logical(&mut self) -> Result<(), RdbcError> {
        let shared = Arc::clone(&self.shared);
        let _close_guard = shared.close_lock.lock().await;
        let cache_owner = shared
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let cache_key = shared
            .key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if self.statement_base.is_closed() {
            return Ok(());
        }
        let resource = Arc::clone(&self.shared.resource);
        let mut cancellation_gate = resource.cancellation_gate.lock().await;
        *cancellation_gate = false;
        // cached raw 可继续存活，但本 logical statement 的子游标必须先全部结束。
        self.statement_base
            .inner
            .dispose_result_sets(CurrentResultDisposition::CloseAll, None)
            .await?;
        if let Err(error) = self
            .shared
            .initial_properties
            .restore(&mut self.statement_base)
        {
            self.shared.cacheable.store(false, Ordering::Release);
            return Err(error);
        }
        let policy = Arc::clone(&self.statement_base.inner.policy);
        let mut statement = resource.statement.lock().await;
        let Some(raw) = statement.as_mut() else {
            resource.closed.store(true, Ordering::Release);
            return self.statement_base.inner.finish_logical_close();
        };

        let cache_enabled = match cache_owner.as_ref() {
            Some(cache) => cache.lock().await.is_enabled(),
            None => false,
        };
        let cleanup_error = match policy.classify(raw.clear_parameters(), Some(&self.shared.sql)) {
            Ok(()) => match policy.classify(raw.clear_prepared_batch(), Some(&self.shared.sql)) {
                Ok(()) => policy
                    .classify(raw.clear_batch(), Some(&self.shared.sql))
                    .err(),
                Err(error) => Some(error),
            },
            Err(error) => Some(error),
        };
        if let Some(error) = cleanup_error {
            let mut raw = statement.take().ok_or(RdbcError::ConnectionDiscarded)?;
            let close_result = policy.classify(raw.close().await, Some(&self.shared.sql));
            if close_result.is_ok() {
                if self.shared.cache_entry || cache_enabled {
                    self.shared.cache_stats.record_checked_out_cache_delete();
                } else {
                    self.shared.cache_stats.record_close();
                }
            } else if let Some(cache) = cache_owner.as_ref() {
                cache.lock().await.retain_failed_close(vec![raw]);
            }
            resource.closed.store(true, Ordering::Release);
            self.shared
                .key
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            self.shared
                .cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            drop(statement);
            // 返回原清理错误时不能发布逻辑关闭成功；registry 留待下一次 close 完成通知。
            return Err(error);
        }

        let raw_closed = raw.is_closed()?;
        if let (Some(key), Some(cache)) = (cache_key.as_ref(), cache_owner.clone()) {
            if self
                .statement_base
                .inner
                .lease_active
                .load(Ordering::Acquire)
                && cache_enabled
                && self.shared.cacheable.load(Ordering::Acquire)
                && !raw_closed
            {
                let raw = statement.take().ok_or(RdbcError::ConnectionDiscarded)?;
                resource.closed.store(true, Ordering::Release);
                drop(statement);
                let removed = cache.lock().await.put(key.clone(), raw);
                self.shared.cache_stats.record_cache_insert();
                self.shared
                    .key
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                self.shared
                    .cache
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                let mut first_error = None;
                let mut failed = Vec::new();
                for mut evicted in removed {
                    if let Err(error) = evicted.close().await {
                        first_error.get_or_insert(error);
                        failed.push(evicted);
                    }
                    self.shared.cache_stats.record_cache_delete();
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
            if self.shared.cache_entry {
                self.shared.cache_stats.record_checked_out_cache_delete();
            } else if cache_enabled && !self.shared.cacheable.load(Ordering::Acquire) {
                // Java 对从未入缓存的执行失败语句也递减；已借出条目此前已递减，不能混用。
                self.shared.cache_stats.record_cache_delete();
            } else if cache_enabled
                && !self
                    .statement_base
                    .inner
                    .lease_active
                    .load(Ordering::Acquire)
            {
                self.shared.cache_stats.record_checked_out_cache_delete();
            } else {
                self.shared.cache_stats.record_close();
            }
        }
        statement.take();
        resource.closed.store(true, Ordering::Release);
        self.shared
            .key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        self.shared
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        drop(statement);
        self.statement_base.inner.finish_logical_close()
    }

    async fn execute_prepared_results(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        // 原生 Prepared/Callable 路径也必须绑定执行 SQL，供游标关闭统计和过滤器追踪。
        self.statement_base
            .begin_single_execution(&self.shared.sql, super::StatementExecuteType::Execute);
        self.begin_execution_result();
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let parameters = self.parameter_snapshot()?;
        let resource = Arc::clone(&self.shared.resource);
        let results = self
            .statement_base
            .inner
            .policy
            .execute_prepared(
                &self.shared.sql,
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
                    Some(ExecutionResult::ResultSet(rows)) => ExecResult {
                        row_count: rows.known_row_count(),
                        ..ExecResult::default()
                    },
                    None => ExecResult::default(),
                },
            )
            .await;
        let results = self.retain_cacheable_result(results)?;
        let result = DruidPooledResultSet::new_standard_results(
            Arc::clone(&self.statement_base.inner),
            results,
            &admission,
        )
        .await;
        self.complete_execution_result(result)
    }
}

#[async_trait::async_trait]
impl Statement for DruidPooledPreparedStatement {
    /// 按 JDBC 整数处置推进结果；先检查逻辑关闭和租约，再验证参数且不改变游标。
    /// 对应 Java：DruidPooledPreparedStatement#getMoreResults(int)。返回下个结果是否为游标。
    async fn get_more_results_current(&mut self, current: i32) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let result = self.statement_base.get_more_results_current(current).await;
        self.retain_cacheable_result(result)
    }

    delegate_statement_state_methods!(statement_base);
    delegate_cached_statement_property_reads!(statement_base);

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        self.retain_cacheable_result(Statement::query_timeout(&self.statement_base))
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.configure_standard_query_timeout(seconds)
    }

    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        self.begin_execution_result();
        let result = Statement::execute_query(&mut self.statement_base, sql).await;
        self.complete_execution_result(result)
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        self.begin_execution_result();
        let result = Statement::execute_update(&mut self.statement_base, sql, generated_keys).await;
        self.complete_execution_result(result)
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        self.begin_execution_result();
        let result = Statement::execute(&mut self.statement_base, sql, generated_keys).await;
        self.complete_execution_result(result)
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let parameters = self.parameter_snapshot()?;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_batch(sql);
        self.classify_statement_mutation(result)?;
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(parameters);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .statement_base
            .clear_batch_with_sql(Some(&self.shared.sql));
        // 基类已完成一次分类和计错；这里只保留预编译缓存失效约束。
        self.retain_cacheable_result(result)?;
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clear();
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.ensure_open()?;
        self.begin_execution_result();
        let parameter_sets = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clone();
        let resource = Arc::clone(&self.shared.resource);
        let result = self
            .statement_base
            .inner
            .policy
            .execute_prepared_batch(
                &self.shared.sql,
                &parameter_sets,
                self.id(),
                move || async move {
                    resource
                        .statement
                        .lock()
                        .await
                        .as_mut()
                        .ok_or(RdbcError::ConnectionDiscarded)?
                        .execute_batch()
                        .await
                },
            )
            .await;
        if result.is_ok() {
            self.shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .batch_parameter_sets
                .clear();
        }
        self.complete_execution_result(result)
    }

    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        self.retain_cacheable_result(
            self.statement_base
                .check_result_property(StatementPropertyEvent::GetGeneratedKeys),
        )?;
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let raw: Box<dyn ResultSet> = if self.shared.has_execution_result.load(Ordering::Acquire) {
            let result = self
                .shared
                .resource
                .statement
                .lock()
                .await
                .as_mut()
                .ok_or(RdbcError::ConnectionDiscarded)?
                .generated_keys()
                .await;
            // getter 驱动错误与参数变更共用分类、失效和缓存淘汰策略。
            self.classify_statement_mutation(result)?
        } else {
            Box::new(druid_rdbc::extensions::BufferedRowSet::with_column_labels(
                Vec::new(),
                vec!["GENERATED_KEY".to_owned()],
            ))
        };
        let result = DruidPooledResultSet::new_standard_auxiliary(
            Arc::clone(&self.statement_base.inner),
            raw,
            &admission,
        )
        .await
        .map(|result_set| Box::new(result_set) as Box<dyn ResultSet>);
        self.retain_cacheable_result(result)
    }

    /// 返回当前执行结果的共享游标，无参数；返回 None 表示当前为更新计数。
    /// 对应 Java: DruidPooledPreparedStatement#getResultSet。
    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::get_result_set(&mut self.statement_base).await;
        self.retain_cacheable_result(result)
    }

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        let count =
            self.retain_cacheable_result(Statement::get_update_count(&self.statement_base))?;
        // 缓存命中继承 raw handle，不继承前一个逻辑租约的执行结果。
        Ok(
            if self.shared.has_execution_result.load(Ordering::Acquire) {
                count
            } else {
                None
            },
        )
    }

    async fn get_more_results(
        &mut self,
        disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        let result = Statement::get_more_results(&mut self.statement_base, disposition).await;
        self.retain_cacheable_result(result)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.close_logical().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        if self.is_closed() {
            return Ok(true);
        }
        let raw = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?;
        match raw.as_ref() {
            Some(statement) => statement.is_closed(),
            // raw 在清理失败时可以已经释放；逻辑 close 尚未成功，保留 BASE 可观察状态。
            None => Ok(self.statement_base.is_closed()),
        }
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        let result = self
            .statement_base
            .warnings_with_sql(Some(&self.shared.sql));
        self.retain_cacheable_result(result)
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
            .statement_base
            .clear_warnings_with_sql(Some(&self.shared.sql));
        self.retain_cacheable_result(result)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for DruidPooledPreparedStatement {
    fn sql(&self) -> Result<&str, RdbcError> {
        self.ensure_open()?;
        Ok(&self.shared.sql)
    }

    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let index = parameter.index.get() - 1;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_parameter(parameter.clone());
        self.classify_statement_mutation(result)?;
        let mut state = self
            .shared
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
        index: druid_rdbc::statement::ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_input_parameter(index, parameter.clone());
        self.classify_statement_mutation(result)?;
        let offset = index.get() - 1;
        let mut state = self
            .shared
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
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_parameters();
        self.classify_statement_mutation(result)?;
        self.shared
            .state
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
        parameters: Vec<(
            druid_rdbc::statement::ParameterIndex,
            PreparedInputParameter,
        )>,
    ) -> Result<(), RdbcError> {
        self.add_input_parameter_batch_core(parameters)
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.clear_prepared_batch_core()
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        // 原生 Prepared/Callable 路径也必须绑定执行 SQL，供游标关闭统计和过滤器追踪。
        self.statement_base
            .begin_single_execution(&self.shared.sql, super::StatementExecuteType::ExecuteQuery);
        self.begin_execution_result();
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let parameters = self.parameter_snapshot()?;
        let resource = Arc::clone(&self.shared.resource);
        let raw = self
            .statement_base
            .inner
            .policy
            .execute_prepared(
                &self.shared.sql,
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
        let result = DruidPooledResultSet::new_standard(
            Arc::clone(&self.statement_base.inner),
            raw,
            &admission,
        )
        .await
        .map(|rows| Box::new(rows) as Box<dyn ResultSet>);
        self.complete_execution_result(result)
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        // 原生 Prepared/Callable 路径也必须绑定执行 SQL，供游标关闭统计和过滤器追踪。
        self.statement_base
            .begin_single_execution(&self.shared.sql, super::StatementExecuteType::ExecuteUpdate);
        self.begin_execution_result();
        let parameters = self.parameter_snapshot()?;
        let resource = Arc::clone(&self.shared.resource);
        let result = self
            .statement_base
            .inner
            .policy
            .execute_prepared(
                &self.shared.sql,
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
            .await;
        self.complete_execution_result(result)
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
        self.shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .parameter_meta_data()
    }
}
