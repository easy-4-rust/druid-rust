//! Canonical pooled CallableStatement proxy.

mod cache_lifecycle;
mod identity;
mod prepared_batch;
mod shared_owner;

use super::filter::ExecOperation;
use super::optional_statement_state_delegates::delegate_optional_statement_state_methods;
use super::prepared_statement_cache_stats::PreparedStatementCacheStats;
use super::prepared_statement_key::PreparedStatementKey;
use super::standard_prepared_statement_cache::StandardPreparedStatementCache;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_registry::StandardResourceRegistry;
use super::statement_state_delegates::delegate_statement_state_methods;
use super::{DruidPooledResultSet, DruidPooledStatement, ExecResult, FilterChain, RdbcError};
use crate::stats::StatsCollector;
use druid_rdbc::metadata::{ParameterMetaData, ResultSetMetaData};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    CurrentResultDisposition, ExecutionResult, GeneratedKeys, ParameterIndex,
    PreparedInputParameter, PreparedParameter, PreparedStatement, Statement,
};
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(test)]
#[path = "druid_pooled_callable_statement/cancellation_control_tests.rs"]
mod cancellation_control_tests;

struct CallableStatementResource {
    cache_stats: Arc<PreparedStatementCacheStats>,
    cache_entry: bool,
    statement: tokio::sync::Mutex<Option<Box<dyn CallableStatement>>>,
    closed: AtomicBool,
    cancellation_gate: tokio::sync::Mutex<bool>,
}

impl CallableStatementResource {
    fn new(
        statement: Box<dyn CallableStatement>,
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

struct CallableStatementCancellation {
    resource: std::sync::Weak<CallableStatementResource>,
    control: Arc<dyn druid_rdbc::statement::StatementCancellation>,
}

#[async_trait::async_trait]
impl druid_rdbc::statement::StatementCancellation for CallableStatementCancellation {
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
    /// 检查资源后验证 JDBC 整数处置；返回下个结果是否为游标。
    async fn get_more_results_current(&mut self, current: i32) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.get_more_results(CurrentResultDisposition::try_from(current)?)
            .await
    }

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

    fn cancellation_control(
        &self,
    ) -> Result<Arc<dyn druid_rdbc::statement::StatementCancellation>, RdbcError> {
        self.resource.ensure_open()?;
        let control = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .cancellation_control()?;
        Ok(Arc::new(CallableStatementCancellation {
            resource: Arc::downgrade(&self.resource),
            control,
        }))
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
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
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
#[derive(Clone)]
pub struct DruidPooledCallableStatement {
    statement_base: DruidPooledStatement,
    shared: Arc<shared_owner::CallableStatementSharedOwner>,
}

impl DruidPooledCallableStatement {
    /// Wraps one owned canonical CallableStatement.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new_standard(
        statement: Box<dyn CallableStatement>,
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
            CallableStatementResource::new(statement, Arc::clone(&cache_stats), cache_hit);
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
        let shared = Arc::new(shared_owner::CallableStatementSharedOwner {
            statement_base: statement_base.clone(),
            resource,
            sql: sql.to_owned(),
            key: Mutex::new(Some(key)),
            cache: Mutex::new(Some(cache)),
            cache_stats,
            cache_entry: cache_hit,
            cacheable: AtomicBool::new(true),
            initial_properties,
            state: Mutex::new(CallableStatementState {
                parameters: Vec::new(),
                batch_parameter_sets: Vec::new(),
            }),
            close_lock: tokio::sync::Mutex::new(()),
        });
        let owner: Arc<dyn super::logical_statement_owner::LogicalStatementOwner> = shared.clone();
        statement_base.install_logical_owner(Arc::downgrade(&owner));
        let pooled = Self {
            statement_base,
            shared,
        };
        // 静态 Callable 返回容器仍可能只声明普通 Prepared 角色；保留原动态准入拒绝。
        // owner 已装配并登记，拒绝后由同一监管清理路径关闭真实后端及更新缓存统计。
        let role_result = {
            let raw = pooled.shared.resource.statement.lock().await;
            druid_rdbc::wrapper::Wrapper::is_wrapper_for(
                raw.as_ref().ok_or(RdbcError::ConnectionDiscarded)?.as_ref(),
                Some(std::any::TypeId::of::<dyn CallableStatement>()),
            )
        };
        let admission_result = match role_result {
            Ok(true) => Ok(()),
            Ok(false) => Err(RdbcError::UnsupportedOperation {
                operation: "connection_prepare_call",
            }),
            Err(error) => Err(error),
        };
        if let Err(error) = pooled.classify_statement_mutation(admission_result) {
            let registration =
                super::standard_resource_handle::StandardResourceHandle::registration_slot(
                    pooled.statement_base.inner.as_ref(),
                )
                .get()
                .cloned()
                .ok_or(RdbcError::ConnectionDiscarded)?;
            let _ = resources
                .close_registered_after_admission(&[registration], admission)
                .await;
            return Err(error);
        }
        Ok(pooled)
    }

    /// Returns whether this logical statement is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.statement_base.is_closed()
            || self.shared.resource.closed.load(Ordering::Acquire)
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

    /// 对已完成错误分类的执行结果只记录缓存资格，不重复通知 fatal handler。
    fn retain_cacheable_result<T>(&self, result: Result<T, RdbcError>) -> Result<T, RdbcError> {
        if result.is_err() {
            self.shared.cacheable.store(false, Ordering::Release);
        }
        result
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
        if self.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            self.shared.resource.ensure_open()
        }
    }

    /// 在原始 Callable 上执行完整 getter，转换错误同样使句柄不可复用。
    fn read_out_result<T>(
        &self,
        read: impl FnOnce(&dyn CallableStatement) -> Result<T, RdbcError>,
    ) -> Result<T, RdbcError> {
        self.ensure_open()?;
        let result = {
            let raw = self
                .shared
                .resource
                .statement
                .try_lock()
                .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?;
            read(raw.as_ref().ok_or(RdbcError::ConnectionDiscarded)?.as_ref())
        };
        self.classify_statement_mutation(result)
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

    async fn execute_prepared_results(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
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
                    Some(ExecutionResult::ResultSet(_)) | None => ExecResult::default(),
                },
            )
            .await;
        let results = self.retain_cacheable_result(results)?;
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
    /// 按 JDBC 整数处置推进结果；先检查逻辑关闭和租约，再验证参数且不改变游标。
    /// 对应 Java：DruidPooledCallableStatement#getMoreResults(int)。返回下个结果是否为游标。
    async fn get_more_results_current(&mut self, current: i32) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.get_more_results(CurrentResultDisposition::try_from(current)?)
            .await
    }

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
        let result = Statement::execute_query(&mut self.statement_base, sql).await;
        self.retain_cacheable_result(result)
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        let result = Statement::execute_update(&mut self.statement_base, sql, generated_keys).await;
        self.retain_cacheable_result(result)
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::execute(&mut self.statement_base, sql, generated_keys).await;
        self.retain_cacheable_result(result)
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let parameters = self.parameter_snapshot()?;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
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
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_batch();
        self.classify_statement_mutation(result)?;
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
        self.retain_cacheable_result(result)
    }

    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let raw = self
            .shared
            .resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .generated_keys()
            .await;
        let raw = self.classify_statement_mutation(raw)?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard_auxiliary(
                Arc::clone(&self.statement_base.inner),
                raw,
                &admission,
            )
            .await?,
        ))
    }

    /// 返回当前执行的共享结果集，无参数；当前为更新计数时返回 None。
    /// 对应 Java: DruidPooledCallableStatement#getResultSet。
    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::get_result_set(&mut self.statement_base).await;
        self.retain_cacheable_result(result)
    }

    /// 返回当前更新计数，无参数；当前无更新结果时返回 None。
    /// 对应 Java: DruidPooledCallableStatement#getUpdateCount。
    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::get_update_count(&self.statement_base);
        self.retain_cacheable_result(result)
    }

    /// 按指定处置推进共享主结果序列，返回下一结果是否为游标。
    /// 对应 Java: DruidPooledCallableStatement#getMoreResults。
    async fn get_more_results(
        &mut self,
        disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.ensure_open()?;
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
        self.shared
            .resource
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
            .shared
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
            .classify(result, Some(&self.shared.sql))
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
            .shared
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
            .classify(result, Some(&self.shared.sql))
    }
}

#[async_trait::async_trait]
impl PreparedStatement for DruidPooledCallableStatement {
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
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
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
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .shared
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
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
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
            .await;
        let raw = self.retain_cacheable_result(raw)?;
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
        self.retain_cacheable_result(result)
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.execute_prepared_results().await
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        let result = self.execute_prepared_batch_core().await;
        self.retain_cacheable_result(result)
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
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .parameter_meta_data()
    }
}

impl CallableStatement for DruidPooledCallableStatement {
    /// 校验当前语句的原始索引，参数错误使真实句柄退出缓存并记录语句异常。
    /// 参数 parameter_index 保留调用者值；返回严格 selector 或关闭/索引错误。
    /// 对应 Java: DruidPooledCallableStatement#index_parameter / record_result。
    fn index_parameter(&self, parameter_index: usize) -> Result<CallableParameter, RdbcError> {
        self.ensure_open()?;
        let result = self.classify_statement_mutation(CallableParameter::by_index(parameter_index));
        if result.is_err() {
            self.statement_base.record_exception();
        }
        result
    }

    fn register_out_parameter(
        &mut self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .shared
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
        // 对应 Java Callable 命名 setter：空名称在逻辑层拒绝，不能交给 driver 接受。
        if parameter_name.is_empty() {
            return self.classify_statement_mutation(Err(RdbcError::InvalidArgument(
                "callable parameter name is empty".to_owned(),
            )));
        }
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_named_parameter(parameter_name, parameter);
        self.classify_statement_mutation(result)
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.read_out_result(|raw| raw.out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn out_parameter_with_type_map(
        &self,
        _parameter: &CallableParameter,
        _type_map: Option<&druid_rdbc::types::RdbcTypeMap>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.read_out_result(|raw| raw.out_parameter_with_type_map(_parameter, _type_map))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn out_parameter_as(
        &self,
        _parameter: &CallableParameter,
        _target_type: &druid_rdbc::types::RdbcTargetType,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.read_out_result(|raw| raw.out_parameter_as(_parameter, _target_type))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn out_value(&self, _index: ParameterIndex) -> Result<Value, RdbcError> {
        self.read_out_result(|raw| raw.out_value(_index))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn was_null(&self) -> Result<bool, RdbcError> {
        self.read_out_result(CallableStatement::was_null)
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, RdbcError> {
        self.read_out_result(|raw| raw.string_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn n_string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, RdbcError> {
        self.read_out_result(|raw| raw.n_string_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn url_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcUrl>, RdbcError> {
        self.read_out_result(|raw| raw.url_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn ref_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcRef>, RdbcError> {
        self.read_out_result(|raw| raw.ref_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn array_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcArray>, RdbcError> {
        self.read_out_result(|raw| raw.array_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn row_id_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcRowId>, RdbcError> {
        self.read_out_result(|raw| raw.row_id_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn sql_xml_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcSqlXml>, RdbcError> {
        self.read_out_result(|raw| raw.sql_xml_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn boolean_out_parameter(&self, parameter: &CallableParameter) -> Result<bool, RdbcError> {
        self.read_out_result(|raw| raw.boolean_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn byte_out_parameter(&self, parameter: &CallableParameter) -> Result<i8, RdbcError> {
        self.read_out_result(|raw| raw.byte_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn short_out_parameter(&self, parameter: &CallableParameter) -> Result<i16, RdbcError> {
        self.read_out_result(|raw| raw.short_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn int_out_parameter(&self, parameter: &CallableParameter) -> Result<i32, RdbcError> {
        self.read_out_result(|raw| raw.int_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn long_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, RdbcError> {
        self.read_out_result(|raw| raw.long_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn float_out_parameter(&self, parameter: &CallableParameter) -> Result<f32, RdbcError> {
        self.read_out_result(|raw| raw.float_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn double_out_parameter(&self, parameter: &CallableParameter) -> Result<f64, RdbcError> {
        self.read_out_result(|raw| raw.double_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn bytes_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<Vec<u8>>, RdbcError> {
        self.read_out_result(|raw| raw.bytes_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn blob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcBlob>, RdbcError> {
        self.read_out_result(|raw| raw.blob_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcClob>, RdbcError> {
        self.read_out_result(|raw| raw.clob_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn n_clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcNClob>, RdbcError> {
        self.read_out_result(|raw| raw.n_clob_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcReader>, RdbcError> {
        self.read_out_result(|raw| raw.character_stream_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn n_character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcReader>, RdbcError> {
        self.read_out_result(|raw| raw.n_character_stream_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn big_decimal_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<bigdecimal::BigDecimal>, RdbcError> {
        self.read_out_result(|raw| raw.big_decimal_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn big_decimal_out_parameter_with_scale(
        &self,
        parameter: &CallableParameter,
        scale: i32,
    ) -> Result<Option<bigdecimal::BigDecimal>, RdbcError> {
        self.read_out_result(|raw| raw.big_decimal_out_parameter_with_scale(parameter, scale))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn date_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &druid_rdbc::types::RdbcCalendarArgument,
    ) -> Result<Option<chrono::NaiveDate>, RdbcError> {
        self.read_out_result(|raw| raw.date_out_parameter(parameter, _calendar))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn time_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &druid_rdbc::types::RdbcCalendarArgument,
    ) -> Result<Option<chrono::NaiveTime>, RdbcError> {
        self.read_out_result(|raw| raw.time_out_parameter(parameter, _calendar))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn timestamp_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &druid_rdbc::types::RdbcCalendarArgument,
    ) -> Result<Option<chrono::NaiveDateTime>, RdbcError> {
        self.read_out_result(|raw| raw.timestamp_out_parameter(parameter, _calendar))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn integer_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, RdbcError> {
        self.read_out_result(|raw| raw.integer_out_parameter(parameter))
    }
}
