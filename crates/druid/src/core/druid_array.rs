use super::druid_array_state::DruidArrayState;
use super::filter_chain::FilterChain;
use super::resource_context::ResourceContext;
use super::resource_runtime_capabilities::ResourceRuntimeCapabilities;
use super::standard_logical_policy::StandardLogicalPolicy;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registry::StandardResourceRegistry;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{Array, ResourceOperations};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::types::{RdbcObject, RdbcTypeMap};
use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc,
};

/// 当前连接租约监管的 SQL 数组代理，保留元素、映射及游标身份。
/// 对应 Java: java.sql.Array；承担 Druid pooled 资源代理职责。
pub(crate) struct DruidArray {
    state: Arc<DruidArrayState>,
    registry: Arc<StandardResourceRegistry>,
    capabilities: ResourceOperations,
    connection_id: u64,
    result_set_id_seed: Arc<AtomicU64>,
    lease_active: Arc<AtomicBool>,
    filter_chain: Option<Arc<FilterChain>>,
    stats_collector: Option<Arc<crate::stats::StatsCollector>>,
    policy: Arc<StandardLogicalPolicy>,
}
impl std::fmt::Debug for DruidArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DruidArray")
            .field("state", &self.state.context.state())
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}
impl DruidArray {
    /// 接管数组与当前租约的游标上下文，登记后返回监管代理；失败返回原登记错误。
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn register(
        payload: Box<dyn Array>,
        context: Arc<ResourceContext>,
        registry: Arc<StandardResourceRegistry>,
        admission: &StandardResourceAdmission,
        connection_id: u64,
        result_set_id_seed: Arc<AtomicU64>,
        lease_active: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<crate::stats::StatsCollector>>,
        policy: Arc<StandardLogicalPolicy>,
    ) -> Result<Self, RdbcError> {
        let supported = payload.capabilities();
        let mut capabilities = ResourceOperations::empty();
        for (operation, runtime) in [
            (ResourceOperations::READ, ResourceRuntimeCapabilities::READ),
            (
                ResourceOperations::RANGE,
                ResourceRuntimeCapabilities::RANGE,
            ),
            (
                ResourceOperations::TYPE_MAP,
                ResourceRuntimeCapabilities::TYPE_MAP,
            ),
            (
                ResourceOperations::RESULT_SET,
                ResourceRuntimeCapabilities::RESULT_SET,
            ),
            (ResourceOperations::FREE, ResourceRuntimeCapabilities::FREE),
        ] {
            if supported.contains(operation) && context.capabilities().contains(runtime) {
                capabilities |= operation;
            }
        }
        let state = Arc::new(DruidArrayState::new(payload, context));
        let actor: Arc<dyn StandardResourceHandle> = state.clone();
        let registered = registry.register_batch(admission, vec![actor]).await;
        admission.finish();
        registered?;
        Ok(Self {
            state,
            registry,
            capabilities,
            connection_id,
            result_set_id_seed,
            lease_active,
            filter_chain,
            stats_collector,
            policy,
        })
    }
    fn require(
        &self,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<(), RdbcError> {
        self.state
            .context
            .require(ResourceRuntimeCapabilities::empty(), operation)?;
        if self.state.registration().is_none_or(
            super::standard_resource_registration::StandardResourceRegistration::cleanup_requested,
        ) {
            return Err(RdbcError::rdbc_resource_closed("Array"));
        }
        if !self.capabilities.contains(capability) {
            return Err(RdbcError::feature_not_supported(operation));
        }
        Ok(())
    }
    async fn payload(
        &self,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<tokio::sync::MutexGuard<'_, Box<dyn Array>>, RdbcError> {
        self.require(capability, operation)?;
        let payload = self.state.payload.lock().await;
        self.require(capability, operation)?;
        Ok(payload)
    }
}
#[async_trait::async_trait]
impl Array for DruidArray {
    fn capabilities(&self) -> ResourceOperations {
        self.capabilities
    }
    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        let result = self
            .payload(ResourceOperations::READ, "Array#base_type_name")
            .await?
            .base_type_name()
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn base_type(&mut self) -> Result<i32, RdbcError> {
        let result = self
            .payload(ResourceOperations::READ, "Array#base_type")
            .await?
            .base_type()
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn values(&mut self) -> Result<Vec<RdbcObject>, RdbcError> {
        let result = self
            .payload(ResourceOperations::READ, "Array#values")
            .await?
            .values()
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn values_with_type_map(
        &mut self,
        type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        let result = self
            .payload(
                ResourceOperations::READ | ResourceOperations::TYPE_MAP,
                "Array#values_with_type_map",
            )
            .await?
            .values_with_type_map(type_map)
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn values_range(&mut self, index: i64, count: i32) -> Result<Vec<RdbcObject>, RdbcError> {
        let result = self
            .payload(
                ResourceOperations::READ | ResourceOperations::RANGE,
                "Array#values_range",
            )
            .await?
            .values_range(index, count)
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn values_range_with_type_map(
        &mut self,
        index: i64,
        count: i32,
        type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        let result = self
            .payload(
                ResourceOperations::READ | ResourceOperations::RANGE | ResourceOperations::TYPE_MAP,
                "Array#values_range_with_type_map",
            )
            .await?
            .values_range_with_type_map(index, count, type_map)
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        let mut payload = self
            .payload(ResourceOperations::RESULT_SET, "Array#result_set")
            .await?;
        let admission = self
            .registry
            .begin_admission(self.policy.resource_generation())?;
        let raw_result = payload.result_set().await;
        let raw = self.state.context.classify_operation_result(raw_result)?;
        let state = Arc::clone(&self.state);
        // 在第一次可能挂起前保存子游标，然后释放 Array 锁；失败清理不等待自身持有的锁。
        let result=super::druid_pooled_result_set::DruidPooledResultSet::new_standard_metadata_with_handoff(
            self.connection_id,Arc::clone(&self.result_set_id_seed),Arc::clone(&self.lease_active),self.filter_chain.clone(),
            self.stats_collector.clone(),Arc::clone(&self.registry),Arc::clone(&self.policy),&admission,raw,
            move |cursor| { state.track_cursor(cursor); drop(payload); }).await;
        admission.finish();
        result.map(|cursor| Box::new(cursor) as Box<dyn ResultSet>)
    }
    async fn result_set_with_type_map(
        &mut self,
        type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let mut payload = self
            .payload(
                ResourceOperations::RESULT_SET | ResourceOperations::TYPE_MAP,
                "Array#result_set_with_type_map",
            )
            .await?;
        let admission = self
            .registry
            .begin_admission(self.policy.resource_generation())?;
        let raw_result = payload.result_set_with_type_map(type_map).await;
        let raw = self.state.context.classify_operation_result(raw_result)?;
        let state = Arc::clone(&self.state);
        // 在第一次可能挂起前保存子游标，然后释放 Array 锁；失败清理不等待自身持有的锁。
        let result=super::druid_pooled_result_set::DruidPooledResultSet::new_standard_metadata_with_handoff(
            self.connection_id,Arc::clone(&self.result_set_id_seed),Arc::clone(&self.lease_active),self.filter_chain.clone(),
            self.stats_collector.clone(),Arc::clone(&self.registry),Arc::clone(&self.policy),&admission,raw,
            move |cursor| { state.track_cursor(cursor); drop(payload); }).await;
        admission.finish();
        result.map(|cursor| Box::new(cursor) as Box<dyn ResultSet>)
    }
    async fn result_set_range(
        &mut self,
        index: i64,
        count: i32,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let mut payload = self
            .payload(
                ResourceOperations::RESULT_SET | ResourceOperations::RANGE,
                "Array#result_set_range",
            )
            .await?;
        let admission = self
            .registry
            .begin_admission(self.policy.resource_generation())?;
        let raw_result = payload.result_set_range(index, count).await;
        let raw = self.state.context.classify_operation_result(raw_result)?;
        let state = Arc::clone(&self.state);
        // 在第一次可能挂起前保存子游标，然后释放 Array 锁；失败清理不等待自身持有的锁。
        let result=super::druid_pooled_result_set::DruidPooledResultSet::new_standard_metadata_with_handoff(
            self.connection_id,Arc::clone(&self.result_set_id_seed),Arc::clone(&self.lease_active),self.filter_chain.clone(),
            self.stats_collector.clone(),Arc::clone(&self.registry),Arc::clone(&self.policy),&admission,raw,
            move |cursor| { state.track_cursor(cursor); drop(payload); }).await;
        admission.finish();
        result.map(|cursor| Box::new(cursor) as Box<dyn ResultSet>)
    }
    async fn result_set_range_with_type_map(
        &mut self,
        index: i64,
        count: i32,
        type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let mut payload = self
            .payload(
                ResourceOperations::RESULT_SET
                    | ResourceOperations::RANGE
                    | ResourceOperations::TYPE_MAP,
                "Array#result_set_range_with_type_map",
            )
            .await?;
        let admission = self
            .registry
            .begin_admission(self.policy.resource_generation())?;
        let raw_result = payload
            .result_set_range_with_type_map(index, count, type_map)
            .await;
        let raw = self.state.context.classify_operation_result(raw_result)?;
        let state = Arc::clone(&self.state);
        // 在第一次可能挂起前保存子游标，然后释放 Array 锁；失败清理不等待自身持有的锁。
        let result=super::druid_pooled_result_set::DruidPooledResultSet::new_standard_metadata_with_handoff(
            self.connection_id,Arc::clone(&self.result_set_id_seed),Arc::clone(&self.lease_active),self.filter_chain.clone(),
            self.stats_collector.clone(),Arc::clone(&self.registry),Arc::clone(&self.policy),&admission,raw,
            move |cursor| { state.track_cursor(cursor); drop(payload); }).await;
        admission.finish();
        result.map(|cursor| Box::new(cursor) as Box<dyn ResultSet>)
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        if let Some(result) = self.state.completed_release().await {
            return result;
        }
        let registration = self
            .state
            .registration()
            .ok_or(RdbcError::ConnectionDiscarded)?;
        self.registry
            .close_registered(std::slice::from_ref(registration))
            .await
    }
}
