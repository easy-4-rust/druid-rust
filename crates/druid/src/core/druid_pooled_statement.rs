//! Canonical pooled Statement proxy.

use super::StatementMutation;

use super::druid_execution_filter_policy::DruidExecutionFilterPolicy;
use super::druid_pooled_result_set::StandardResultSetResource;
use super::filter::ExecOperation;
use super::logical_statement_owner::LogicalStatementOwner;
use super::standard_logical_policy::StandardLogicalPolicy;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registry::StandardResourceRegistry;
use super::{
    DruidPooledResultSet, FilterChain, ProxyAttributeValue, ProxyAttributes, RdbcError,
    StatementExecuteType, StatementPropertyEvent,
};
use crate::stats::StatsCollector;
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecResult, ExecutionResult, FetchDirection, GeneratedKeys,
    ResultSetConcurrency, ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

mod standard_statement;

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
    statement_events_enabled: AtomicBool,
    statement_closed_notified: AtomicBool,
    state: Mutex<DruidPooledStatementState>,
    result_sets: Mutex<Vec<std::sync::Weak<StandardResultSetResource>>>,
    result_sequence: Mutex<Option<StatementResultSequence>>,
    logical_owner: Mutex<Option<std::sync::Weak<dyn LogicalStatementOwner>>>,
}

struct PooledStatementCancellation {
    inner: std::sync::Weak<DruidPooledStatementInner>,
    control: Arc<dyn druid_rdbc::statement::StatementCancellation>,
}

#[async_trait::async_trait]
impl druid_rdbc::statement::StatementCancellation for PooledStatementCancellation {
    async fn cancel(&self) -> Result<(), RdbcError> {
        let inner = self.inner.upgrade().ok_or(RdbcError::ConnectionDiscarded)?;
        if !inner.lease_active.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        if inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
        {
            return Err(RdbcError::ConnectionDiscarded);
        }
        inner.policy.ensure_open()?;
        self.control.cancel().await
    }
}

struct DruidPooledStatementState {
    options: StatementOptions,
    max_field_size: i32,
    max_rows: i64,
    escape_processing: bool,
    query_timeout: i32,
    cursor_name: Option<String>,
    fetch_direction: FetchDirection,
    fetch_size: i32,
    poolable: bool,
    close_on_completion: bool,
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
    /// JDBC 兼容值：处理更多结果时关闭当前结果集。
    pub const CLOSE_CURRENT_RESULT: i32 = 1;
    /// JDBC 兼容值：处理更多结果时保留当前结果集。
    pub const KEEP_CURRENT_RESULT: i32 = 2;
    /// JDBC 兼容值：处理更多结果时关闭所有结果集。
    pub const CLOSE_ALL_RESULTS: i32 = 3;
    /// JDBC 兼容值：批处理成功但更新数量未知。
    pub const SUCCESS_NO_INFO: i32 = -2;
    /// JDBC 兼容值：批处理中的语句执行失败。
    pub const EXECUTE_FAILED: i32 = -3;
    /// JDBC 兼容值：请求返回生成键。
    pub const RETURN_GENERATED_KEYS: i32 = 1;
    /// JDBC 兼容值：不请求返回生成键。
    pub const NO_GENERATED_KEYS: i32 = 2;

    /// 测试专用：读取原查询登记的实际结果资源，返回同一共享资源。
    /// 对应 Java: Statement#getResultSet；不创建游标、不重新执行 SQL。
    #[cfg(test)]
    pub(crate) fn current_result_set_for_diagnostics(&self) -> Arc<StandardResultSetResource> {
        let sequence = self.inner.result_sequence.lock().unwrap();
        let sequence = sequence.as_ref().expect("查询必须登记结果序列");
        match &sequence.results[sequence.current] {
            TrackedStatementResult::ResultSet(resource) => {
                resource.upgrade().expect("原结果资源必须仍在 registry 中")
            }
            TrackedStatementResult::UpdateCount(_) => panic!("原查询必须返回结果集"),
        }
    }

    /// 安装创建语句的弱 owner；参数不延长 owner 生命周期，无返回值。
    /// 对应 Java: java.sql.ResultSet#getStatement 的逻辑身份绑定。
    pub(crate) fn install_logical_owner(&self, owner: std::sync::Weak<dyn LogicalStatementOwner>) {
        *self
            .inner
            .logical_owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(owner);
    }

    /// 返回仍存活的共享创建语句 owner；普通语句不额外设置 owner。
    /// 对应 Java: java.sql.ResultSet#getStatement；无参数，返回强引用或 None。
    pub(crate) fn logical_owner(&self) -> Option<Arc<dyn LogicalStatementOwner>> {
        self.inner
            .logical_owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
    }

    /// Creates a canonical pooled statement.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_standard(
        statement: Box<dyn Statement>,
        options: StatementOptions,
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
                statement_events_enabled: AtomicBool::new(false),
                statement_closed_notified: AtomicBool::new(false),
                state: Mutex::new(DruidPooledStatementState {
                    options,
                    max_field_size: 0,
                    max_rows: 0,
                    escape_processing: true,
                    query_timeout: 0,
                    cursor_name: None,
                    fetch_direction: FetchDirection::Unknown,
                    fetch_size: 0,
                    poolable: false,
                    close_on_completion: false,
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
                logical_owner: Mutex::new(None),
            }),
        }
    }

    pub(crate) fn from_inner(inner: Arc<DruidPooledStatementInner>) -> Self {
        Self { inner }
    }

    /// 用底层已支持的初始属性初始化代理状态，供关闭时精确恢复。
    pub(crate) fn initialize_prepared_properties(
        &self,
        properties: &super::prepared_statement_property_snapshot::PreparedStatementPropertySnapshot,
    ) {
        let mut state = self.state_mut();
        if let Some(value) = properties.max_field_size {
            state.max_field_size = value;
        }
        if let Some(value) = properties.max_rows {
            state.max_rows = value;
        }
        if let Some(value) = properties.query_timeout {
            state.query_timeout = value;
        }
        if let Some(value) = properties.fetch_direction {
            state.fetch_direction = value;
        }
        if let Some(value) = properties.fetch_size {
            state.fetch_size = value;
        }
    }

    /// 返回代理中已确认的 query timeout，避免恢复流程额外访问 driver getter。
    pub(crate) fn prepared_query_timeout(&self) -> i32 {
        self.state().query_timeout
    }

    /// 返回代理已确认的 max field size，供内部预编译语句恢复时避免重复访问驱动。
    pub(crate) fn prepared_max_field_size(&self) -> i32 {
        self.state().max_field_size
    }

    /// 返回代理已确认的 max rows，供内部预编译语句恢复时避免重复访问驱动。
    pub(crate) fn prepared_max_rows(&self) -> i64 {
        self.state().max_rows
    }

    /// 返回代理已确认的 fetch direction，供内部预编译语句恢复时避免重复访问驱动。
    pub(crate) fn prepared_fetch_direction(&self) -> FetchDirection {
        self.state().fetch_direction
    }

    /// 返回代理已确认的 fetch size，供内部预编译语句恢复时避免重复访问驱动。
    pub(crate) fn prepared_fetch_size(&self) -> i32 {
        self.state().fetch_size
    }

    /// Returns whether two handles share one logical Statement identity.
    #[must_use]
    pub fn is_same_statement(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn configure_standard_query_timeout(&self, seconds: i32) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetQueryTimeout(seconds))
    }

    fn classify_statement_mutation<T>(&self, result: Result<T, RdbcError>) -> Result<T, RdbcError> {
        self.classify_statement_access(result, self.last_sql().as_deref())
    }

    /// 记录非执行操作的 SQL 错误，并保留调用点的 SQL 上下文及致命错误分类。
    fn classify_statement_access<T>(
        &self,
        result: Result<T, RdbcError>,
        sql: Option<&str>,
    ) -> Result<T, RdbcError> {
        let result = self.inner.policy.classify(result, sql);
        if let Err(error) = &result {
            self.inner.policy.record_statement_access_error(error);
        }
        result
    }

    /// 在结果状态访问前执行过滤器，拒绝时不得处置已有游标。
    /// 对应 Java: DruidPooledStatement#getUpdateCount/getMoreResults。
    pub(crate) fn check_result_property(
        &self,
        event: StatementPropertyEvent,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        if let Some(chain) = &self.inner.filter_chain {
            self.classify_statement_mutation(
                chain
                    .standard_statement_property_read(&event)
                    .map_err(RdbcError::from),
            )?;
        }
        Ok(())
    }

    fn read_statement_property<T>(
        &self,
        event: StatementPropertyEvent,
        unsupported_operation: &'static str,
        cached: T,
        getter: impl FnOnce(&dyn Statement) -> Result<T, RdbcError>,
    ) -> Result<T, RdbcError> {
        self.ensure_open()?;
        if let Some(chain) = &self.inner.filter_chain {
            if let Err(error) = chain.standard_statement_property_read(&event) {
                return self.classify_statement_mutation(Err(RdbcError::from(error)));
            }
        }
        let result = {
            let statement = self
                .inner
                .statement
                .try_lock()
                .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?;
            getter(statement.as_ref())
        };
        match result {
            Ok(value) => Ok(value),
            Err(RdbcError::UnsupportedOperation { operation })
                if operation == unsupported_operation =>
            {
                Ok(cached)
            }
            Err(error) => self.classify_statement_mutation(Err(error)),
        }
    }

    /// 通过唯一清除链治理继承语句；参数为预编译 SQL 上下文，返回已分类和计错的结果。
    /// 对应 Java: DruidPooledStatement#clearBatch；成功后才清除代理批次。
    pub(crate) fn clear_batch_with_sql(&self, sql: Option<&str>) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = {
            let mut statement = self
                .inner
                .statement
                .try_lock()
                .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?;
            match &self.inner.filter_chain {
                Some(chain) => chain
                    .standard_statement_clear_batch(statement.as_mut())
                    .map_err(RdbcError::from),
                None => statement.clear_batch(),
            }
        };
        self.classify_statement_access(result, sql)?;
        self.state_mut().batch.clear();
        Ok(())
    }

    /// 通过警告链读取同一底层语句，参数为 SQL 上下文，返回原警告或一次计错后的异常。
    /// 对应 Java: DruidPooledStatement#getWarnings。
    pub(crate) fn warnings_with_sql(
        &self,
        sql: Option<&str>,
    ) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        let result = {
            let mut statement = self
                .inner
                .statement
                .try_lock()
                .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?;
            match &self.inner.filter_chain {
                Some(chain) => chain
                    .standard_statement_get_warnings(statement.as_mut())
                    .map_err(RdbcError::from),
                None => statement.warnings(),
            }
        };
        self.classify_statement_access(result, sql)
    }

    /// 通过警告链清除同一底层语句，参数为 SQL 上下文，返回原结果且不伪增执行次数。
    /// 对应 Java: DruidPooledStatement#clearWarnings；关闭检查先于过滤器和驱动。
    pub(crate) fn clear_warnings_with_sql(&self, sql: Option<&str>) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = {
            let mut statement = self
                .inner
                .statement
                .try_lock()
                .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?;
            match &self.inner.filter_chain {
                Some(chain) => chain
                    .standard_statement_clear_warnings(statement.as_mut())
                    .map_err(RdbcError::from),
                None => statement.clear_warnings(),
            }
        };
        self.classify_statement_access(result, sql)
    }

    /// 以最终生效参数提交代理状态；驱动和过滤器失败均保留原状态并仅分类一次。
    fn apply_statement_mutation(&self, mutation: StatementMutation) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = {
            let mut statement = self
                .inner
                .statement
                .try_lock()
                .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?;
            match &self.inner.filter_chain {
                Some(chain) => chain.standard_statement_mutation(statement.as_mut(), mutation),
                None => super::StandardStatementMutationFilterChain::new(
                    &[],
                    statement.as_mut(),
                    mutation.clone(),
                )
                .apply(mutation),
            }
            .map_err(RdbcError::from)
        };
        let effective = self.classify_statement_mutation(result)?;
        let mut state = self.state_mut();
        match effective {
            StatementMutation::SetMaxFieldSize(value) => state.max_field_size = value,
            StatementMutation::SetMaxRows(value) => state.max_rows = value,
            StatementMutation::SetEscapeProcessing(value) => state.escape_processing = value,
            StatementMutation::SetQueryTimeout(value) => state.query_timeout = value,
            StatementMutation::SetCursorName(value) => state.cursor_name = Some(value),
            StatementMutation::SetCursorNameNullable(value) => state.cursor_name = value,
            StatementMutation::SetFetchDirection(value) => state.fetch_direction = value,
            StatementMutation::SetFetchSize(value) => state.fetch_size = value,
            StatementMutation::SetPoolable(value) => state.poolable = value,
            StatementMutation::CloseOnCompletion => state.close_on_completion = true,
            StatementMutation::Cancel => {}
        }
        Ok(())
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

    pub(super) fn begin_single_execution(&self, sql: &str, execute_type: StatementExecuteType) {
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

    /// 记录本语句执行或结果读取的实际失败；无参数，无返回值。
    /// 对应 Java: DruidPooledStatement#checkException。
    pub(crate) fn record_exception(&self) {
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
    pub(crate) fn enable_statement_events(&self) {
        self.statement_events_enabled.store(true, Ordering::Release);
    }

    fn notify_statement_closed(&self) {
        if self.statement_events_enabled.load(Ordering::Acquire)
            && !self.statement_closed_notified.swap(true, Ordering::AcqRel)
        {
            self.policy.notify_statement_closed(self.id as usize);
        }
    }

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

    pub(super) async fn dispose_result_sets(
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
            } else {
                // 级联关闭不经过结果集外层 close，仍须把同一游标的实际峰值交回创建语句。
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.fetch_row_peak = state.fetch_row_peak.max(result_set.fetch_row_count());
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
        // 先关闭 logical 子游标；raw driver close 不一定知道已经代理/交出的 cursor。
        // 子资源失败仍由 registry 持有，可重试；此处不持有 raw statement 锁跨回调。
        self.dispose_result_sets(CurrentResultDisposition::CloseAll, None)
            .await?;
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
        self.notify_statement_closed();
        self.unregister_resource();
        if let Some(filter_chain) = &self.filter_chain {
            filter_chain.after_statement_close_with_identity(self.connection_id, self.id)?;
        }
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
        self.notify_statement_closed();
        self.unregister_resource();
        if let Some(filter_chain) = &self.filter_chain {
            filter_chain
                .after_statement_close_with_identity(self.connection_id, self.id)
                .map_err(RdbcError::from)?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl StandardResourceHandle for DruidPooledStatementInner {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        // 连接归还时也必须经过创建语句的逻辑关闭策略：恢复属性并归还可缓存句柄。
        // 在 await 前释放同步锁；owner 只有弱引用，不形成 registry/statement 强引用环。
        let owner = self
            .logical_owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(std::sync::Weak::upgrade);
        if let Some(owner) = owner {
            return owner.statement_handle().close().await;
        }
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

#[cfg(test)]
mod cancellation_control_tests;

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
