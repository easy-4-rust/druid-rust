//! Canonical pooled CallableStatement proxy.

mod cache_lifecycle;
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
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    CurrentResultDisposition, ExecutionResult, GeneratedKeys, ParameterIndex,
    PreparedInputParameter, PreparedParameter, PreparedStatement, Statement,
};
use druid_rdbc::types::Value;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod callable_trait_impls;

#[cfg(test)]
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
    out_indexes: BTreeSet<usize>,
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
                out_indexes: BTreeSet::new(),
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
        let result = self.classify_statement_mutation(result);
        if let Err(error) = &result {
            self.statement_base
                .inner
                .policy
                .record_statement_access_error(error);
        }
        result
    }

    fn parameter_snapshot(&self) -> Result<Vec<PreparedInputParameter>, RdbcError> {
        let state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                // 纯 OUT 参数无需绑定 IN。此占位仅供 Filter 的位置快照使用，
                // 实际执行调用持有原始 OUT/IN 描述符的 driver statement。
                parameter
                    .clone()
                    .or_else(|| {
                        state
                            .out_indexes
                            .contains(&(index + 1))
                            .then_some(PreparedInputParameter::RustValue(Value::Null))
                    })
                    .ok_or_else(|| {
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
        // 原生 Prepared/Callable 路径也必须绑定执行 SQL，供游标关闭统计和过滤器追踪。
        self.statement_base
            .begin_single_execution(&self.shared.sql, super::StatementExecuteType::Execute);
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
