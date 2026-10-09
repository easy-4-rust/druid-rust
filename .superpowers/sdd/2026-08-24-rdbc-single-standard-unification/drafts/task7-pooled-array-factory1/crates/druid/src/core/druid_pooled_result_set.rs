//! Canonical pooled ResultSet proxy.

mod access;
mod shared_state;

use super::druid_pooled_statement::{
    DruidPooledStatement, DruidPooledStatementInner, TrackedStatementResult,
};
use super::result_set_meta_data_proxy::ResultSetMetaDataProxy;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registry::StandardResourceRegistry;
use super::{FilterChain, ProxyAttributeValue, ProxyAttributes, RdbcError, ResultSetFilterContext};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead};
use druid_rdbc::statement::{ExecutionResult, Statement};
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(test)]
mod diagnostic_tests;

pub(crate) struct StandardResultSetResource {
    result_set: tokio::sync::Mutex<Option<Box<dyn ResultSet>>>,
    known_row_count: Option<u64>,
    view: OnceLock<Arc<shared_state::SharedResultSetState>>,
    opened: AtomicBool,
    closing: AtomicBool,
    closed: AtomicBool,
    close_error: Mutex<Option<RdbcError>>,
    filter_chain: Option<Arc<FilterChain>>,
    filter_context: Arc<ResultSetFilterContext>,
    policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
    meta_data_attributes: Arc<ProxyAttributes>,
    registration: OnceLock<super::standard_resource_registration::StandardResourceRegistration>,
}

impl StandardResultSetResource {
    fn new(
        result_set: Box<dyn ResultSet>,
        filter_chain: Option<Arc<FilterChain>>,
        filter_context: Arc<ResultSetFilterContext>,
        policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
    ) -> Arc<Self> {
        Arc::new(Self {
            // 在接管前保存只读摘要；不锁定或推进 raw cursor，也不延长执行借用。
            known_row_count: result_set.known_row_count(),
            view: OnceLock::new(),
            result_set: tokio::sync::Mutex::new(Some(result_set)),
            opened: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            close_error: Mutex::new(None),
            filter_chain,
            filter_context,
            policy,
            meta_data_attributes: Arc::new(ProxyAttributes::default()),
            registration: OnceLock::new(),
        })
    }

    /// 接管 driver getter 返回的同一游标的新拥有型视图，不重建逻辑身份或触发 open。
    /// 对应 Java: Statement#getResultSet；参数为当前 raw 视图，返回交接结果。
    pub(crate) async fn adopt_current_view(
        &self,
        mut raw: Box<dyn ResultSet>,
    ) -> Result<(), RdbcError> {
        let mut current = self.result_set.lock().await;
        if self.closed.load(Ordering::Acquire) {
            // 已关闭游标不得复活；正常 driver 返回的是已关闭视图。
            // 若它仍开放，则释放这次交接的拥有型资源，并保留驱动错误分类。
            let closed = self
                .policy
                .classify(raw.is_closed(), self.filter_context.sql())?;
            if !closed {
                self.policy
                    .classify(raw.close().await, self.filter_context.sql())?;
            }
            return Ok(());
        }
        // 新视图已经拥有同一 driver cursor，替换旧视图不会执行 cursor close。
        // registry、共享游标状态、Filter context 与混合结果序列继续引用本资源。
        *current = Some(raw);
        Ok(())
    }

    async fn next_core(&self) -> Result<bool, RdbcError> {
        self.policy.ensure_open()?;
        if self.closed.load(Ordering::Acquire) {
            return Err(RdbcError::Other("result set is closed".to_owned()));
        }
        let mut result_set = self.result_set.lock().await;
        // 等待资源锁期间租约可能结束，优先拒绝旧租约，再报告游标关闭。
        self.policy.ensure_open()?;
        let raw = result_set
            .as_mut()
            .ok_or_else(|| RdbcError::Other("result set is closed".to_owned()))?;
        let result = match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .standard_result_set_next(raw.as_mut(), &self.filter_context)
                .await
                .map_err(RdbcError::from),
            None => raw.next().await,
        };
        self.policy.classify(result, self.filter_context.sql())
    }

    /// 返回本游标实际抓取峰值，供创建语句在级联关闭后汇总；无参数。
    /// 对应 Java: DruidPooledResultSet#close 的 fetchRowPeak 交接。
    pub(super) fn fetch_row_count(&self) -> i32 {
        self.filter_context.fetch_row_count()
    }

    pub(crate) async fn close_core(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            self.unregister_resource();
            return Ok(());
        }
        self.closing.store(true, Ordering::Release);
        let mut result_set = self.result_set.lock().await;
        let Some(raw) = result_set.as_mut() else {
            self.closed.store(true, Ordering::Release);
            self.unregister_resource();
            return Ok(());
        };
        let opened = self.opened.load(Ordering::Acquire);
        let result = match (&self.filter_chain, opened) {
            (Some(filter_chain), true) => filter_chain
                .standard_result_set_close(raw.as_mut(), &self.filter_context)
                .await
                .map_err(RdbcError::from),
            (Some(_), false) | (None, _) => raw.close().await,
        };
        if let Err(error) = self.policy.classify(result, self.filter_context.sql()) {
            *self
                .close_error
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error.clone());
            return Err(error);
        }
        self.close_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        result_set.take();
        self.closed.store(true, Ordering::Release);
        if opened {
            self.filter_context.increment_close_count();
        }
        self.unregister_resource();
        Ok(())
    }

    fn registry_cleanup_error(&self) -> Option<RdbcError> {
        if let Some(error) = self
            .close_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            return Some(error);
        }
        let cleanup_requested = self.registration.get().is_some_and(
            super::standard_resource_registration::StandardResourceRegistration::cleanup_requested,
        );
        (cleanup_requested
            || self.closing.load(Ordering::Acquire)
            || self.closed.load(Ordering::Acquire))
        .then_some(RdbcError::ConnectionDiscarded)
    }

    fn open_meta_data_core(
        &self,
    ) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, RdbcError> {
        let result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        let meta_data = self
            .policy
            .classify(raw.meta_data(), self.filter_context.sql())?;
        Ok(Arc::new(ResultSetMetaDataProxy::new(
            meta_data,
            Arc::clone(&self.meta_data_attributes),
        )))
    }

    fn value_core(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.read_value_core(column, false)
    }

    fn string_value_core(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.read_value_core(column, true)
    }

    // 两类读取共用资源锁、Filter 与错误分类；区别仅在驱动末端 getter。
    fn read_value_core(&self, column: ColumnIndex, string_read: bool) -> Result<Value, RdbcError> {
        let mut result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = match &self.filter_chain {
            Some(filter_chain) if string_read => filter_chain
                .standard_result_set_string_value(raw.as_mut(), &self.filter_context, column)
                .map_err(RdbcError::from),
            Some(filter_chain) => filter_chain
                .standard_result_set_value(raw.as_mut(), &self.filter_context, column)
                .map_err(RdbcError::from),
            None if string_read => raw.string_value(column),
            None => raw.value(column),
        };
        self.policy.classify(result, self.filter_context.sql())
    }

    fn meta_data_proxy_core(&self) -> Result<ResultSetMetaDataProxy, RdbcError> {
        let mut result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .standard_result_set_meta_data(raw.as_mut(), &self.filter_context)
                .map_err(RdbcError::from),
            None => raw.meta_data(),
        };
        let meta_data = self.policy.classify(result, self.filter_context.sql())?;
        Ok(ResultSetMetaDataProxy::new(
            meta_data,
            Arc::clone(&self.meta_data_attributes),
        ))
    }
}

#[async_trait::async_trait]
impl StandardResourceHandle for StandardResultSetResource {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        self.close_core().await
    }

    fn is_resource_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn registration_slot(
        &self,
    ) -> &OnceLock<super::standard_resource_registration::StandardResourceRegistration> {
        &self.registration
    }
}

/// Pooled canonical cursor bound to one logical connection generation.
pub struct DruidPooledResultSet {
    id: u64,
    statement: DruidPooledStatement,
    logical_owner: Option<Arc<dyn super::logical_statement_owner::LogicalStatementOwner>>,
    resource: Arc<StandardResultSetResource>,
    shared: Arc<shared_state::SharedResultSetState>,
    has_creating_statement: bool,
    filter_context: Arc<ResultSetFilterContext>,
}

struct StagedResultSet {
    id: u64,
    statement: DruidPooledStatement,
    logical_owner: Option<Arc<dyn super::logical_statement_owner::LogicalStatementOwner>>,
    resource: Arc<StandardResultSetResource>,
    filter_chain: Option<Arc<FilterChain>>,
    filter_context: Arc<ResultSetFilterContext>,
}

struct StagedOpenError {
    error: RdbcError,
    cleanup_required: bool,
}

impl StagedResultSet {
    fn new(statement: Arc<DruidPooledStatementInner>, raw: Box<dyn ResultSet>) -> Self {
        let statement_handle = DruidPooledStatement::from_inner(Arc::clone(&statement));
        let id = statement.result_set_id_seed.fetch_add(1, Ordering::AcqRel);
        let filter_context = Arc::new(
            ResultSetFilterContext::with_identity_sql_and_execute_elapsed(
                statement.connection_id,
                statement.id,
                id,
                statement_handle.last_sql(),
                statement_handle.last_execute_elapsed(),
            ),
        );
        let filter_chain = statement.filter_chain.clone();
        let resource = StandardResultSetResource::new(
            raw,
            filter_chain.clone(),
            Arc::clone(&filter_context),
            Arc::clone(&statement.policy),
        );
        statement.track_result_set(&resource);
        Self {
            id,
            logical_owner: statement_handle.logical_owner(),
            statement: statement_handle,
            resource,
            filter_chain,
            filter_context,
        }
    }

    fn into_opened(self) -> Result<DruidPooledResultSet, StagedOpenError> {
        if let Some(error) = self.resource.registry_cleanup_error() {
            return Err(StagedOpenError {
                error,
                cleanup_required: false,
            });
        }
        // 无 Filter 时不预读元数据，保留稀疏驱动在 getMetaData 调用点拒绝的语义。
        let (driver_column_map, logic_column_map, hidden_columns) =
            if let Some(filter_chain) = &self.filter_chain {
                let meta_data =
                    self.resource
                        .open_meta_data_core()
                        .map_err(|error| StagedOpenError {
                            error,
                            cleanup_required: true,
                        })?;
                let mut context =
                    super::StandardResultSetOpenContext::new(&self.filter_context, meta_data);
                self.resource
                    .policy
                    .classify(
                        filter_chain
                            .standard_result_set_open_after_with_proxy(&mut context)
                            .map_err(RdbcError::from),
                        self.filter_context.sql(),
                    )
                    .map_err(|error| StagedOpenError {
                        error,
                        cleanup_required: true,
                    })?;
                context.into_column_mappings()
            } else {
                (None, None, None)
            };
        if let Some(error) = self.resource.registry_cleanup_error() {
            return Err(StagedOpenError {
                error,
                cleanup_required: false,
            });
        }
        let shared = Arc::new(shared_state::SharedResultSetState {
            id: self.id,
            attributes: ProxyAttributes::default(),
            closed: AtomicBool::new(false),
            cursor_index: AtomicI32::new(0),
            logic_column_map: parking_lot::RwLock::new(None),
            driver_column_map: parking_lot::RwLock::new(None),
            hidden_columns: parking_lot::RwLock::new(None),
        });
        let result_set = DruidPooledResultSet {
            id: self.id,
            statement: self.statement,
            logical_owner: self.logical_owner,
            has_creating_statement: true,
            resource: self.resource,
            shared,
            filter_context: self.filter_context,
        };
        // 初始 Filter 配置与后续设置共用同一状态入口，发布视图前完成安装。
        result_set.set_logic_column_map(driver_column_map);
        result_set.set_physical_column_map(logic_column_map);
        result_set.set_hidden_columns(hidden_columns);
        // 资源只保存不含 statement/owner 的游标视图，避免 registry 强引用环。
        let _ = result_set.resource.view.set(Arc::clone(&result_set.shared));
        result_set.resource.opened.store(true, Ordering::Release);
        Ok(result_set)
    }
}

enum StagedExecutionResult {
    ResultSet(StagedResultSet),
    UpdateCount(i64),
}

impl DruidPooledResultSet {
    /// JDBC 正向抓取数值。对应 Java: java.sql.ResultSet#FETCH_FORWARD。
    pub const FETCH_FORWARD: i32 = 1000;

    /// JDBC 反向抓取数值。对应 Java: java.sql.ResultSet#FETCH_REVERSE。
    pub const FETCH_REVERSE: i32 = 1001;

    /// JDBC 未知抓取方向数值。对应 Java: java.sql.ResultSet#FETCH_UNKNOWN。
    pub const FETCH_UNKNOWN: i32 = 1002;

    /// JDBC 仅正向游标数值。对应 Java: java.sql.ResultSet#TYPE_FORWARD_ONLY。
    pub const TYPE_FORWARD_ONLY: i32 = 1003;

    /// JDBC 滚动不敏感游标数值。对应 Java: java.sql.ResultSet#TYPE_SCROLL_INSENSITIVE。
    pub const TYPE_SCROLL_INSENSITIVE: i32 = 1004;

    /// JDBC 滚动敏感游标数值。对应 Java: java.sql.ResultSet#TYPE_SCROLL_SENSITIVE。
    pub const TYPE_SCROLL_SENSITIVE: i32 = 1005;

    /// JDBC 只读并发模式数值。对应 Java: java.sql.ResultSet#CONCUR_READ_ONLY。
    pub const CONCUR_READ_ONLY: i32 = 1007;

    /// JDBC 可更新并发模式数值。对应 Java: java.sql.ResultSet#CONCUR_UPDATABLE。
    pub const CONCUR_UPDATABLE: i32 = 1008;

    /// JDBC 提交后保留游标数值。对应 Java: java.sql.ResultSet#HOLD_CURSORS_OVER_COMMIT。
    pub const HOLD_CURSORS_OVER_COMMIT: i32 = 1;

    /// JDBC 提交时关闭游标数值。对应 Java: java.sql.ResultSet#CLOSE_CURSORS_AT_COMMIT。
    pub const CLOSE_CURSORS_AT_COMMIT: i32 = 2;

    /// 返回实际驱动元数据的内部代理，复用标准 getter 的 Filter 与共享属性路径。
    /// 参数为空；关闭或失效时返回错误，否则返回只读元数据代理。
    /// 对应 Java: DruidPooledResultSet#getMetaData。
    fn meta_data_proxy_core(&self) -> Result<ResultSetMetaDataProxy, RdbcError> {
        self.ensure_open()?;
        self.resource.meta_data_proxy_core()
    }

    pub(crate) async fn new_standard(
        statement: Arc<DruidPooledStatementInner>,
        raw: Box<dyn ResultSet>,
        admission: &StandardResourceAdmission,
    ) -> Result<Self, RdbcError> {
        let result_set =
            Self::new_standard_auxiliary(Arc::clone(&statement), raw, admission).await?;
        statement.set_execution_results(vec![TrackedStatementResult::ResultSet(Arc::downgrade(
            &result_set.resource,
        ))]);
        Ok(result_set)
    }

    /// 注册独立附属游标；参数为 owner、raw 与入场许可，返回游标且不改变主结果序列。
    /// 对应 Java: Statement#getGeneratedKeys。
    pub(crate) async fn new_standard_auxiliary(
        statement: Arc<DruidPooledStatementInner>,
        raw: Box<dyn ResultSet>,
        admission: &StandardResourceAdmission,
    ) -> Result<Self, RdbcError> {
        Self::new_standard_auxiliary_with_handoff(statement, raw, admission, |_| {}).await
    }

    async fn new_standard_auxiliary_with_handoff<F>(
        statement: Arc<DruidPooledStatementInner>,
        raw: Box<dyn ResultSet>,
        admission: &StandardResourceAdmission,
        handoff: F,
    ) -> Result<Self, RdbcError>
    where
        F: FnOnce(Arc<StandardResultSetResource>) + Send,
    {
        let resources = Arc::clone(&statement.resources);
        let staged = StagedResultSet::new(statement, raw);
        // 将已接管 raw 的 actor 同步交给资源 owner，再进入异步 registry 登记。
        handoff(Arc::clone(&staged.resource));
        let handle = Arc::clone(&staged.resource) as Arc<dyn StandardResourceHandle>;
        let registrations = resources.register_batch(admission, vec![handle]).await?;
        match staged.into_opened() {
            Ok(result_set) => Ok(result_set),
            Err(open_error) => {
                let cleanup = resources
                    .close_registered_after_admission(&registrations, admission)
                    .await;
                if open_error.cleanup_required {
                    Err(open_error.error)
                } else {
                    cleanup.and(Err(open_error.error))
                }
            }
        }
    }

    /// 恢复当前结果的共享句柄；参数为创建语句和已登记资源，返回同一逻辑游标。
    /// 对应 Java: Statement#getResultSet。
    pub(crate) fn from_tracked_resource(
        statement: Arc<DruidPooledStatementInner>,
        resource: Arc<StandardResultSetResource>,
    ) -> Result<Self, RdbcError> {
        let shared = Arc::clone(resource.view.get().ok_or(RdbcError::ConnectionDiscarded)?);
        let statement = DruidPooledStatement::from_inner(statement);
        Ok(Self {
            id: shared.id,
            logical_owner: statement.logical_owner(),
            statement,
            filter_context: Arc::clone(&resource.filter_context),
            resource,
            shared,
            has_creating_statement: true,
        })
    }

    pub(crate) async fn new_standard_results(
        statement: Arc<DruidPooledStatementInner>,
        results: Vec<ExecutionResult>,
        admission: &StandardResourceAdmission,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        let resources = Arc::clone(&statement.resources);
        let mut staged_results = Vec::with_capacity(results.len());
        let mut handles = Vec::new();
        for result in results {
            match result {
                ExecutionResult::ResultSet(raw) => {
                    let staged = StagedResultSet::new(Arc::clone(&statement), raw);
                    handles.push(Arc::clone(&staged.resource) as Arc<dyn StandardResourceHandle>);
                    staged_results.push(StagedExecutionResult::ResultSet(staged));
                }
                ExecutionResult::UpdateCount(count) => {
                    staged_results.push(StagedExecutionResult::UpdateCount(count));
                }
            }
        }
        let registrations = resources.register_batch(admission, handles).await?;
        let mut opened = Vec::with_capacity(staged_results.len());
        let mut tracked = Vec::with_capacity(staged_results.len());
        for staged in staged_results {
            match staged {
                StagedExecutionResult::ResultSet(staged) => match staged.into_opened() {
                    Ok(result_set) => {
                        tracked.push(TrackedStatementResult::ResultSet(Arc::downgrade(
                            &result_set.resource,
                        )));
                        opened.push(ExecutionResult::ResultSet(Box::new(result_set)));
                    }
                    Err(open_error) => {
                        let cleanup = resources
                            .close_registered_after_admission(&registrations, admission)
                            .await;
                        return if open_error.cleanup_required {
                            Err(open_error.error)
                        } else {
                            cleanup.and(Err(open_error.error))
                        };
                    }
                },
                StagedExecutionResult::UpdateCount(count) => {
                    tracked.push(TrackedStatementResult::UpdateCount(count));
                    opened.push(ExecutionResult::UpdateCount(count));
                }
            }
        }
        statement.set_execution_results(tracked);
        Ok(opened)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new_standard_metadata(
        connection_id: u64,
        result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
        lease_active: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<crate::stats::StatsCollector>>,
        resources: Arc<StandardResourceRegistry>,
        policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
        admission: &StandardResourceAdmission,
        raw: Box<dyn ResultSet>,
    ) -> Result<Self, RdbcError> {
        Self::new_standard_metadata_with_handoff(
            connection_id,
            result_set_id_seed,
            lease_active,
            filter_chain,
            stats_collector,
            resources,
            policy,
            admission,
            raw,
            |_| {},
        )
        .await
    }

    /// 同步交接无创建语句的资源游标，返回受原连接租约监管的完整标准代理。
    /// 对应 Java: Array#getResultSet；handoff 在 registry 可能挂起前接管子 actor。
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new_standard_metadata_with_handoff<F>(
        connection_id: u64,
        result_set_id_seed: Arc<AtomicU64>,
        lease_active: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<crate::stats::StatsCollector>>,
        resources: Arc<StandardResourceRegistry>,
        policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
        admission: &StandardResourceAdmission,
        raw: Box<dyn ResultSet>,
        handoff: F,
    ) -> Result<Self, RdbcError>
    where
        F: FnOnce(Arc<StandardResultSetResource>) + Send,
    {
        let statement = DruidPooledStatement::new_standard(
            Box::new(MetadataStatement),
            druid_rdbc::statement::StatementOptions::default(),
            resources,
            policy,
            connection_id,
            0,
            result_set_id_seed,
            Arc::new(AtomicU64::new(0)),
            lease_active,
            filter_chain,
            stats_collector,
        );
        let mut result = Self::new_standard_auxiliary_with_handoff(
            Arc::clone(&statement.inner),
            raw,
            admission,
            handoff,
        )
        .await?;
        statement
            .inner
            .set_execution_results(vec![TrackedStatementResult::ResultSet(Arc::downgrade(
                &result.resource,
            ))]);
        result.has_creating_statement = false;
        Ok(result)
    }

    /// Returns the result-set identifier.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn statement(&self) -> DruidPooledStatement {
        self.statement.clone()
    }

    #[must_use]
    pub fn attributes_size(&self) -> usize {
        self.shared.attributes.len()
    }

    pub fn clear_attributes(&self) {
        self.shared.attributes.clear();
    }

    pub fn put_attribute(
        &self,
        key: impl Into<String>,
        value: ProxyAttributeValue,
    ) -> Option<ProxyAttributeValue> {
        self.shared.attributes.put(key, value)
    }

    #[must_use]
    pub fn attribute(&self, key: &str) -> Option<ProxyAttributeValue> {
        self.shared.attributes.get(key)
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.shared.closed.load(Ordering::Acquire)
            || !self.statement.inner.lease_active.load(Ordering::Acquire)
        {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    /// 在同一资源锁内治理并包围行操作；参数为操作身份，返回原Filter或驱动结果。
    /// 对应 Java: DruidPooledResultSet 的七个行操作，旧租约/关闭先于Filter和驱动。
    fn filtered_row_operation(
        &self,
        operation: super::ResultSetRowOperation,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let mut terminal = || {
                match operation {
                    super::ResultSetRowOperation::InsertRow => raw.insert_row(),
                    super::ResultSetRowOperation::UpdateRow => raw.update_row(),
                    super::ResultSetRowOperation::DeleteRow => raw.delete_row(),
                    super::ResultSetRowOperation::RefreshRow => raw.refresh_row(),
                    super::ResultSetRowOperation::CancelRowUpdates => raw.cancel_row_updates(),
                    super::ResultSetRowOperation::MoveToInsertRow => raw.move_to_insert_row(),
                    super::ResultSetRowOperation::MoveToCurrentRow => raw.move_to_current_row(),
                }
                .map_err(super::DruidError::from)
            };
            match &self.resource.filter_chain {
                Some(filters) => filters
                    .standard_result_set_row_operation(
                        &self.filter_context,
                        operation,
                        &mut terminal,
                    )
                    .map_err(RdbcError::from),
                None => terminal().map_err(RdbcError::from),
            }
        });
        // access_core 已执行既有错误分类；结果集及原创建语句只计错一次。
        self.record_operation_result(result)
    }

    /// 为已分类的结果集操作记录创建语句失败；参数为操作结果，返回原结果。
    /// 对应 Java: DruidPooledResultSet#checkException；不重复执行 fatal 分类。
    fn record_operation_result<T>(&self, result: Result<T, RdbcError>) -> Result<T, RdbcError> {
        if let Err(error) = &result {
            self.resource.policy.record_result_set_error(error);
            self.statement.record_exception();
        }
        result
    }

    /// 在同一资源锁内包围属性并校验返回类型，分类与原创建语句计错各一次。
    /// 参数为原请求、原生末端和类型提取；返回属性值或原错误。
    /// 对应 Java: DruidPooledResultSet 的属性 Filter 分派。
    fn filtered_property<T>(
        &self,
        request: super::ResultSetPropertyOperation<'_>,
        mut terminal: impl FnMut(&mut dyn ResultSet) -> Result<super::ResultSetPropertyValue, RdbcError>,
        decode: impl FnOnce(super::ResultSetPropertyValue) -> Result<T, RdbcError>,
    ) -> Result<T, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let value = match &self.resource.filter_chain {
                Some(filters) => filters
                    .standard_result_set_property(&self.filter_context, request, &mut || {
                        terminal(raw).map_err(crate::core::DruidError::from)
                    })
                    .map_err(RdbcError::from)?,
                None => terminal(raw)?,
            };
            decode(value)
        });
        self.record_operation_result(result)
    }

    /// 在同一资源锁内先治理列访问再运行对象过滤链，只向原创建语句记错一次。
    /// 参数保留六种原重载身份；返回原资源对象或精确错误。
    /// 对应 Java: DruidPooledResultSet#getObject(int/String, Map/Class)。
    fn filtered_object(
        &self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &super::ResultSetObjectRead<'_>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let column = if let Some(label) = label {
                self.validate_object_label(raw, label)?;
                None
            } else {
                column
                    .map(|column| self.driver_column_index(column))
                    .transpose()?
            };
            match &self.resource.filter_chain {
                Some(filters) => filters.standard_result_set_object(
                    raw,
                    &self.filter_context,
                    column,
                    request,
                    label,
                ),
                None => super::StandardResultSetFilterChain::new(&[], raw, &self.filter_context)
                    .result_set_object(column, label, request),
            }
            .map_err(RdbcError::from)
        });
        if result.is_err() {
            // access_core已按租约策略分类，创建语句只记录本次读取失败一次。
            self.statement.record_exception();
        }
        result
    }

    /// 在唯一资源锁内治理字节流读取，不消费句柄；返回同一流或已分类错误。
    /// 参数保留列重载和流类型。对应 Java: DruidPooledResultSet#getXxxStream。
    fn filtered_input_stream(
        &self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let column = self.governed_stream_column(raw, column, label)?;
            match &self.resource.filter_chain {
                Some(filters) => filters.standard_result_set_input_stream(
                    raw,
                    &self.filter_context,
                    column,
                    label,
                    kind,
                ),
                None => super::StandardResultSetFilterChain::new(&[], raw, &self.filter_context)
                    .result_set_input_stream(column, label, kind),
            }
            .map_err(RdbcError::from)
        });
        if result.is_err() {
            self.statement.record_exception();
        }
        result
    }

    /// 在唯一资源锁内治理字符流读取，保留原Reader游标；返回原句柄或错误。
    /// 对应 Java: DruidPooledResultSet#getCharacterStream/getNCharacterStream。
    fn filtered_reader(
        &self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let column = self.governed_stream_column(raw, column, label)?;
            match &self.resource.filter_chain {
                Some(filters) => filters.standard_result_set_reader(
                    raw,
                    &self.filter_context,
                    column,
                    label,
                    national,
                ),
                None => super::StandardResultSetFilterChain::new(&[], raw, &self.filter_context)
                    .result_set_reader(column, label, national),
            }
            .map_err(RdbcError::from)
        });
        if result.is_err() {
            self.statement.record_exception();
        }
        result
    }

    /// 在配置映射或隐藏列时先验证标签可见性；下标返回治理后的驱动列。
    /// 参数为同一原生游标和原列身份，返回可空治理列。对应 Java: DruidPooledResultSet 的列访问治理。
    fn governed_stream_column(
        &self,
        raw: &dyn ResultSet,
        column: Option<ColumnIndex>,
        label: Option<&str>,
    ) -> Result<Option<ColumnIndex>, RdbcError> {
        if let Some(label) = label {
            if self.shared.hidden_columns.read().is_some()
                || self.shared.logic_column_map.read().is_some()
                || self.shared.driver_column_map.read().is_some()
            {
                let driver = raw.find_column(label)?;
                let logical = self.logical_column_index(driver)?;
                return self.driver_column_index(logical).map(Some);
            }
            Ok(None)
        } else {
            column
                .map(|column| self.driver_column_index(column))
                .transpose()
        }
    }

    /// 配置列治理时验证原列名的可见性；返回成功或既有映射错误，不改变实际重载。
    /// 对应 Java: DruidPooledResultSet#getObject(String, ...) 的列访问治理。
    fn validate_object_label(&self, raw: &dyn ResultSet, label: &str) -> Result<(), RdbcError> {
        if self.shared.hidden_columns.read().is_some()
            || self.shared.logic_column_map.read().is_some()
            || self.shared.driver_column_map.read().is_some()
        {
            let driver = raw.find_column(label)?;
            let logical = self.logical_column_index(driver)?;
            self.driver_column_index(logical)?;
        }
        Ok(())
    }

    /// 查询逻辑列对应的驱动列；参数为逻辑列，返回映射或缺失值。
    /// 对应 Java: DruidPooledResultSet#physicalColumn。
    pub(crate) fn physical_column(&self, logical: i32) -> Option<i32> {
        self.shared
            .driver_column_map
            .read()
            .as_ref()
            .map_or(Some(logical), |mapping| mapping.get(&logical).copied())
    }

    /// 查询驱动列对应的逻辑列；参数为驱动列，返回映射或缺失值。
    /// 对应 Java: DruidPooledResultSet#logicColumn。
    pub(crate) fn logic_column(&self, driver: i32) -> Option<i32> {
        self.shared
            .logic_column_map
            .read()
            .as_ref()
            .map_or(Some(driver), |mapping| mapping.get(&driver).copied())
    }

    /// 设置共享正向列映射；参数为映射，None 恢复恒等映射，无返回值。
    /// 对应 Java: DruidPooledResultSet#setLogicColumnMap。
    pub(crate) fn set_logic_column_map(
        &self,
        mapping: Option<std::collections::HashMap<i32, i32>>,
    ) {
        *self.shared.driver_column_map.write() = mapping;
    }

    /// 设置共享反向列映射；参数为映射，None 恢复恒等映射，无返回值。
    /// 对应 Java: DruidPooledResultSet#setPhysicalColumnMap。
    pub(crate) fn set_physical_column_map(
        &self,
        mapping: Option<std::collections::HashMap<i32, i32>>,
    ) {
        *self.shared.logic_column_map.write() = mapping;
    }

    /// 返回隐藏列数量；无参数，未配置时返回零。
    /// 对应 Java: DruidPooledResultSet#getHiddenColumnCount。
    pub(crate) fn hidden_column_count(&self) -> usize {
        self.shared
            .hidden_columns
            .read()
            .as_ref()
            .map_or(0, Vec::len)
    }

    /// 返回隐藏列快照；无参数，未配置时返回 None。
    /// 对应 Java: DruidPooledResultSet#getHiddenColumns。
    pub(crate) fn hidden_columns(&self) -> Option<Vec<i32>> {
        self.shared.hidden_columns.read().clone()
    }

    /// 设置共享隐藏列；参数为驱动列列表，None 清除隐藏配置，无返回值。
    /// 对应 Java: DruidPooledResultSet#setHiddenColumns。
    pub(crate) fn set_hidden_columns(&self, columns: Option<Vec<i32>>) {
        *self.shared.hidden_columns.write() = columns;
    }

    fn driver_column(&self, logical: i32) -> Option<i32> {
        // 未配置映射才采用恒等映射；已配置但缺失键不得回退到原列。
        let driver = self.physical_column(logical)?;
        if self
            .shared
            .hidden_columns
            .read()
            .as_ref()
            .is_some_and(|columns| columns.contains(&driver))
        {
            return None;
        }
        Some(driver)
    }

    fn driver_column_index(&self, column: ColumnIndex) -> Result<ColumnIndex, RdbcError> {
        let logical = i32::try_from(column.get())
            .map_err(|_| RdbcError::InvalidArgument("column index exceeds i32".to_owned()))?;
        let driver = self
            .driver_column(logical)
            .ok_or_else(|| RdbcError::InvalidArgument("column is hidden".to_owned()))?;
        usize::try_from(driver)
            .ok()
            .and_then(|index| ColumnIndex::new(index).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("invalid driver column".to_owned()))
    }

    fn logical_column_index(&self, driver: ColumnIndex) -> Result<ColumnIndex, RdbcError> {
        let driver = i32::try_from(driver.get())
            .map_err(|_| RdbcError::InvalidArgument("column index exceeds i32".to_owned()))?;
        if self
            .shared
            .hidden_columns
            .read()
            .as_ref()
            .is_some_and(|columns| columns.contains(&driver))
        {
            return Err(RdbcError::InvalidArgument("column is hidden".to_owned()));
        }
        let logical = self.logic_column(driver).ok_or_else(|| {
            RdbcError::InvalidArgument("column has no logical mapping".to_owned())
        })?;
        usize::try_from(logical)
            .ok()
            .and_then(|index| ColumnIndex::new(index).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("invalid logical column".to_owned()))
    }
}

#[derive(Debug)]
struct MetadataStatement;

druid_rdbc::impl_standard_wrapper!(MetadataStatement => Statement);

#[async_trait::async_trait]
impl Statement for MetadataStatement {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

druid_rdbc::impl_standard_wrapper!(DruidPooledResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for DruidPooledResultSet {
    /// 返回共享的创建语句，保留 Prepared/Callable 叶子接口；metadata 结果返回 None。
    /// 对应 Java: java.sql.ResultSet#getStatement。
    fn get_statement(
        &self,
    ) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>, RdbcError> {
        // 创建语句身份属于逻辑所有权，不依赖 raw cursor 的开放状态。
        // BASE 允许在 Statement 级联关闭后仍查询同一关闭句柄；失效租约仍拒绝访问。
        if !self.statement.inner.lease_active.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let mut terminal = || {
            if !self.has_creating_statement {
                return Ok(None);
            }
            let statement: Box<dyn druid_rdbc::statement::Statement> = match &self.logical_owner {
                Some(owner) => Arc::clone(owner).statement_handle(),
                None => Box::new(self.statement.clone()),
            };
            Ok(Some(statement))
        };
        let result = match &self.resource.filter_chain {
            Some(filters) => {
                filters.standard_result_set_statement(&self.filter_context, &mut terminal)
            }
            None => terminal(),
        }
        .map_err(RdbcError::from);
        // 查询属于逻辑身份，不要求原生游标开放；Filter失败仍分类并只记原创建语句一次。
        self.record_operation_result(
            self.resource
                .policy
                .classify(result, self.filter_context.sql()),
        )
    }

    fn known_row_count(&self) -> Option<u64> {
        self.ensure_open().ok()?;
        // Statement cascade 或 more-results 可先关闭共享 resource，而不修改此代理标志。
        if self.resource.is_resource_closed() {
            return None;
        }
        self.resource.known_row_count
    }

    async fn next(&mut self) -> Result<bool, RdbcError> {
        // 有效租约中的已关闭游标保留原错误；旧租约始终由策略优先拒绝。
        self.resource.policy.ensure_open()?;
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(RdbcError::Other("result set is closed".to_owned()));
        }
        let more = self.record_operation_result(self.resource.next_core().await)?;
        if more {
            let cursor = self.shared.cursor_index.fetch_add(1, Ordering::AcqRel) + 1;
            self.filter_context.record_fetch_row_count(cursor);
        }
        Ok(more)
    }

    async fn navigate(
        &mut self,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
    ) -> Result<bool, RdbcError> {
        if std::matches!(
            navigation,
            druid_rdbc::result_set::ResultSetNavigation::Next
        ) {
            return self.next().await;
        }
        self.ensure_open()?;
        let (present, native_row) =
            self.record_operation_result(self.resource.navigate_core(navigation).await)?;
        if let Some(row) = native_row {
            self.shared.cursor_index.store(row, Ordering::Release);
            if present {
                self.filter_context.record_fetch_row_count(row);
            }
        }
        Ok(present)
    }

    fn row(&self) -> Result<i32, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::Row,
            |raw| raw.row().map(super::ResultSetPropertyValue::Integer),
            super::ResultSetPropertyValue::into_integer,
        )
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        self.resource.value_core(self.driver_column_index(column)?)
    }

    fn string_value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .string_value_core(self.driver_column_index(column)?);
        // 仅成功的 getString 返回值计入 UTF-16 长度，普通 value/object 不计入。
        if let Ok(Value::String(value)) = &result {
            self.filter_context.add_read_string_length(value);
        }
        result
    }

    fn find_column(&self, column_label: &str) -> Result<ColumnIndex, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::FindColumn(column_label),
            |raw| {
                raw.find_column(column_label)
                    .map(super::ResultSetPropertyValue::Column)
            },
            |value| self.logical_column_index(value.into_column()?),
        )
    }

    fn object(&self, column: ColumnIndex) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(Some(column), None, &super::ResultSetObjectRead::Ordinary)
    }

    /// 按原列名转发普通对象；参数为列名，返回原驱动对象或精确错误。
    /// 对应 Java: DruidPooledResultSet#getObject(String)。
    fn object_by_label(&self, label: &str) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(None, Some(label), &super::ResultSetObjectRead::Ordinary)
    }

    /// 按逻辑列转发完整读取请求，保留 Filter、列治理与驱动错误分类。
    /// 参数为列和强类型重载，返回原资源或过滤后的标量。
    /// 对应 Java: DruidPooledResultSet#getBigDecimal/getDate/getTime/getTimestamp/getBlob 等。
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            // 列治理失败也沿同一策略分类并向创建语句记错，保持双入口一致。
            let column = self.driver_column_index(column)?;
            if matches!(
                request,
                ResultSetRead::String
                    | ResultSetRead::NString
                    | ResultSetRead::Boolean
                    | ResultSetRead::Long
                    | ResultSetRead::Int
                    | ResultSetRead::Short
                    | ResultSetRead::Byte
                    | ResultSetRead::Double
                    | ResultSetRead::Float
                    | ResultSetRead::Bytes
                    | ResultSetRead::BigDecimal(_)
                    | ResultSetRead::Date(_)
                    | ResultSetRead::Time(_)
                    | ResultSetRead::Timestamp(_)
            ) {
                if let Some(filters) = &self.resource.filter_chain {
                    return filters
                        .standard_result_set_typed_value(
                            raw,
                            &self.filter_context,
                            Some(column),
                            request,
                            None,
                        )
                        .map(druid_rdbc::types::RdbcObject::Scalar)
                        .map_err(RdbcError::from);
                }
            }
            match &self.resource.filter_chain {
                Some(filters) => filters
                    .standard_result_set_resource(
                        raw,
                        &self.filter_context,
                        Some(column),
                        request,
                        None,
                    )
                    .map_err(RdbcError::from),
                None => raw.read_object(column, request),
            }
        });
        // getString 在标准描述符分派后仍只记录成功返回值的 UTF-16 长度。
        if matches!(request, ResultSetRead::String) {
            if let Ok(druid_rdbc::types::RdbcObject::Scalar(Value::String(value))) = &result {
                self.filter_context.add_read_string_length(value);
            }
        }
        if result.is_err() {
            // 底层错误已完成策略分类；本次读取只向创建语句记录一次异常。
            self.statement.record_exception();
        }
        result
    }

    /// 按原列名读取，配置列治理时先验证可见性；返回资源或标量。
    /// 对应 Java: DruidPooledResultSet#getXxx(String, ...)。
    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let scalar = matches!(
                request,
                ResultSetRead::String
                    | ResultSetRead::NString
                    | ResultSetRead::Boolean
                    | ResultSetRead::Long
                    | ResultSetRead::Int
                    | ResultSetRead::Short
                    | ResultSetRead::Byte
                    | ResultSetRead::Double
                    | ResultSetRead::Float
                    | ResultSetRead::Bytes
                    | ResultSetRead::BigDecimal(_)
                    | ResultSetRead::Date(_)
                    | ResultSetRead::Time(_)
                    | ResultSetRead::Timestamp(_)
            );
            let governed = self.shared.hidden_columns.read().is_some()
                || self.shared.logic_column_map.read().is_some()
                || self.shared.driver_column_map.read().is_some();
            let column = if governed {
                let driver = raw.find_column(label)?;
                let logical = self.logical_column_index(driver)?;
                Some(self.driver_column_index(logical)?)
            } else {
                None
            };
            if scalar {
                if let Some(filters) = &self.resource.filter_chain {
                    return filters
                        .standard_result_set_typed_value(
                            raw,
                            &self.filter_context,
                            column,
                            request,
                            Some(label),
                        )
                        .map(druid_rdbc::types::RdbcObject::Scalar)
                        .map_err(RdbcError::from);
                }
            }
            match &self.resource.filter_chain {
                Some(filters) => filters
                    .standard_result_set_resource(
                        raw,
                        &self.filter_context,
                        column,
                        request,
                        Some(label),
                    )
                    .map_err(RdbcError::from),
                None => raw.read_object_by_label(label, request),
            }
        });
        // getString 在标准描述符分派后仍只记录成功返回值的 UTF-16 长度。
        if matches!(request, ResultSetRead::String) {
            if let Ok(druid_rdbc::types::RdbcObject::Scalar(Value::String(value))) = &result {
                self.filter_context.add_read_string_length(value);
            }
        }
        if result.is_err() {
            // 底层错误已完成策略分类；本次读取只向创建语句记录一次异常。
            self.statement.record_exception();
        }
        result
    }

    /// 按驱动下标转发可空类型映射；返回原对象或原错误，并记录创建语句失败。
    /// 对应 Java: DruidPooledResultSet#getObject(int, Map)。
    fn object_with_type_map(
        &self,
        column: ColumnIndex,
        type_map: Option<&druid_rdbc::types::RdbcTypeMap>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            Some(column),
            None,
            &super::ResultSetObjectRead::TypeMap(type_map),
        )
    }

    fn object_as(
        &self,
        column: ColumnIndex,
        target_type: &druid_rdbc::types::RdbcTargetType,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            Some(column),
            None,
            &super::ResultSetObjectRead::Typed(target_type),
        )
    }

    /// 按原列名转发可空类型映射；返回驱动对象并保留列治理与错误分类。
    /// 对应 Java: DruidPooledResultSet#getObject(String, Map)。
    fn object_with_type_map_by_label(
        &self,
        label: &str,
        type_map: Option<&druid_rdbc::types::RdbcTypeMap>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            None,
            Some(label),
            &super::ResultSetObjectRead::TypeMap(type_map),
        )
    }

    /// 按原列名转发目标类型；返回驱动原对象，不强制重新转换返回值。
    /// 对应 Java: DruidPooledResultSet#getObject(String, Class)。
    fn object_as_by_label(
        &self,
        label: &str,
        target_type: &druid_rdbc::types::RdbcTargetType,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            None,
            Some(label),
            &super::ResultSetObjectRead::Typed(target_type),
        )
    }

    fn input_stream(
        &self,
        column: ColumnIndex,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, RdbcError> {
        self.filtered_input_stream(Some(column), None, kind)
    }

    /// 按原标签转发字节流重载；参数为标签和流类型，返回原共享流或错误。
    /// 对应 Java: DruidPooledResultSet#getAsciiStream/getUnicodeStream/getBinaryStream(String)。
    fn input_stream_by_label(
        &self,
        label: &str,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, RdbcError> {
        self.filtered_input_stream(None, Some(label), kind)
    }

    fn reader(
        &self,
        column: ColumnIndex,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, RdbcError> {
        self.filtered_reader(Some(column), None, national)
    }

    /// 按原标签转发字符流重载；参数保留标签及national标志，返回原共享Reader。
    /// 对应 Java: DruidPooledResultSet#getCharacterStream/getNCharacterStream(String)。
    fn reader_by_label(
        &self,
        label: &str,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, RdbcError> {
        self.filtered_reader(None, Some(label), national)
    }

    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let column = self.driver_column_index(column)?;
            if let Some(filters) = &self.resource.filter_chain {
                filters
                    .standard_result_set_update(raw, &self.filter_context, column, update)
                    .map_err(RdbcError::from)
            } else {
                raw.update_value(column, update)
            }
        });
        if let Err(error) = &result {
            self.resource.policy.record_result_set_error(error);
            // 策略已分类，ResultSet统计与创建语句异常各记录一次。
            self.statement.record_exception();
        }
        result
    }

    /// 按列名转发完整更新描述符；返回驱动结果，保留列名调用身份。
    /// 对应 Java: DruidPooledResultSet#updateXxx(String, ...)。
    fn update_value_by_label(
        &mut self,
        column_label: &str,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            // 仅配置了列治理时解析可见性；普通列名直接抵达驱动列名入口。
            let governed = self.shared.hidden_columns.read().is_some()
                || self.shared.logic_column_map.read().is_some()
                || self.shared.driver_column_map.read().is_some();
            if governed {
                let logical = self.logical_column_index(raw.find_column(column_label)?)?;
                self.driver_column_index(logical)?;
            }
            if let Some(filters) = &self.resource.filter_chain {
                filters
                    .standard_result_set_update_by_label(
                        raw,
                        &self.filter_context,
                        column_label,
                        update,
                    )
                    .map_err(RdbcError::from)
            } else {
                raw.update_value_by_label(column_label, update)
            }
        });
        if let Err(error) = &result {
            self.resource.policy.record_result_set_error(error);
            // 策略已分类，ResultSet统计与创建语句异常各记录一次。
            self.statement.record_exception();
        }
        result
    }

    fn meta_data(&self) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        let result = self
            .meta_data_proxy_core()
            .map(|metadata| Arc::new(metadata) as Arc<dyn druid_rdbc::metadata::ResultSetMetaData>);
        self.record_operation_result(result)
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::WasNull,
            |raw| {
                // 默认属性委托保留原 wasNull before/after 链，不能绕过已有观察。
                let value = match &self.resource.filter_chain {
                    Some(filters) => filters
                        .standard_result_set_was_null(raw, &self.filter_context)
                        .map_err(RdbcError::from),
                    None => raw.was_null(),
                }?;
                Ok(super::ResultSetPropertyValue::Boolean(value))
            },
            super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::IsBeforeFirst,
            |raw| {
                raw.is_before_first()
                    .map(super::ResultSetPropertyValue::Boolean)
            },
            super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::IsAfterLast,
            |raw| {
                raw.is_after_last()
                    .map(super::ResultSetPropertyValue::Boolean)
            },
            super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn is_first(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::IsFirst,
            |raw| raw.is_first().map(super::ResultSetPropertyValue::Boolean),
            super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn is_last(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::IsLast,
            |raw| raw.is_last().map(super::ResultSetPropertyValue::Boolean),
            super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn set_fetch_direction(
        &mut self,
        direction: druid_rdbc::statement::FetchDirection,
    ) -> Result<(), RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::SetFetchDirection(direction),
            |raw| {
                raw.set_fetch_direction(direction)
                    .map(|()| super::ResultSetPropertyValue::Unit)
            },
            super::ResultSetPropertyValue::into_unit,
        )
    }

    fn fetch_direction(&self) -> Result<druid_rdbc::statement::FetchDirection, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::FetchDirection,
            |raw| {
                raw.fetch_direction()
                    .map(super::ResultSetPropertyValue::FetchDirection)
            },
            super::ResultSetPropertyValue::into_direction,
        )
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::SetFetchSize(rows),
            |raw| {
                raw.set_fetch_size(rows)
                    .map(|()| super::ResultSetPropertyValue::Unit)
            },
            super::ResultSetPropertyValue::into_unit,
        )
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::FetchSize,
            |raw| raw.fetch_size().map(super::ResultSetPropertyValue::Integer),
            super::ResultSetPropertyValue::into_integer,
        )
    }

    fn result_set_type(&self) -> Result<druid_rdbc::statement::ResultSetType, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::ResultSetType,
            |raw| {
                raw.result_set_type()
                    .map(super::ResultSetPropertyValue::ResultSetType)
            },
            super::ResultSetPropertyValue::into_result_set_type,
        )
    }

    fn concurrency(&self) -> Result<druid_rdbc::statement::ResultSetConcurrency, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::Concurrency,
            |raw| {
                raw.concurrency()
                    .map(super::ResultSetPropertyValue::Concurrency)
            },
            super::ResultSetPropertyValue::into_concurrency,
        )
    }

    fn holdability(&self) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::Holdability,
            |raw| {
                raw.holdability()
                    .map(super::ResultSetPropertyValue::Holdability)
            },
            super::ResultSetPropertyValue::into_holdability,
        )
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        self.record_operation_result(self.resource.access_core(|raw| raw.warnings()))
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.record_operation_result(self.resource.access_core(ResultSet::clear_warnings))
    }

    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::CursorName,
            |raw| raw.cursor_name().map(super::ResultSetPropertyValue::String),
            super::ResultSetPropertyValue::into_string,
        )
    }

    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::RowUpdated,
            |raw| {
                raw.row_updated()
                    .map(super::ResultSetPropertyValue::Boolean)
            },
            super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn row_inserted(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::RowInserted,
            |raw| {
                raw.row_inserted()
                    .map(super::ResultSetPropertyValue::Boolean)
            },
            super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn row_deleted(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::ResultSetPropertyOperation::RowDeleted,
            |raw| {
                raw.row_deleted()
                    .map(super::ResultSetPropertyValue::Boolean)
            },
            super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn insert_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::ResultSetRowOperation::InsertRow)
    }

    fn update_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::ResultSetRowOperation::UpdateRow)
    }

    fn delete_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::ResultSetRowOperation::DeleteRow)
    }

    fn refresh_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::ResultSetRowOperation::RefreshRow)
    }

    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::ResultSetRowOperation::CancelRowUpdates)
    }

    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::ResultSetRowOperation::MoveToInsertRow)
    }

    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::ResultSetRowOperation::MoveToCurrentRow)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.shared.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        self.resource.close_resource().await?;
        self.shared.closed.store(true, Ordering::Release);
        self.statement
            .record_fetch_row_count(self.filter_context.fetch_row_count());
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        let closed =
            self.shared.closed.load(Ordering::Acquire) || self.resource.is_resource_closed();
        if closed || !self.statement.inner.lease_active.load(Ordering::Acquire) {
            return Ok(closed);
        }
        self.filtered_property(
            super::ResultSetPropertyOperation::IsClosed,
            |_| Ok(super::ResultSetPropertyValue::Boolean(closed)),
            super::ResultSetPropertyValue::into_boolean,
        )
    }
}

impl std::fmt::Debug for DruidPooledResultSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledResultSet")
            .field("id", &self.id)
            .field("closed", &self.shared.closed.load(Ordering::Acquire))
            .field("cursor", &self.shared.cursor_index.load(Ordering::Acquire))
            .field("logic_column_map", &self.shared.logic_column_map)
            .field("hidden_column_count", &self.hidden_column_count())
            .field("hidden_columns", &self.hidden_columns())
            .finish_non_exhaustive()
    }
}
