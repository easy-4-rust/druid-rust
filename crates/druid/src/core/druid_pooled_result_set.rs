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

mod staging;

mod result_set_trait;

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
        self.record_operation_result(result)
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
        self.record_operation_result(result)
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
        self.record_operation_result(result)
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

druid_rdbc::impl_standard_wrapper!(DruidPooledResultSet => ResultSet);

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
