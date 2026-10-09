use super::druid_sql_xml_payload::DruidSqlXmlPayload;
use super::druid_sql_xml_state::DruidSqlXmlState;
use super::resource_context::ResourceContext;
use super::resource_runtime_capabilities::ResourceRuntimeCapabilities;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registry::StandardResourceRegistry;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter, RdbcXmlRepresentationType,
    RdbcXmlResult, RdbcXmlSource, ResourceOperations, SqlXml,
};
use druid_rdbc::types::RdbcString;
use std::sync::Arc;

/// 连接监管的标准XML 对象代理；驱动操作独占借用，释放交给 registry。
/// 对应 Java: com.alibaba.druid.proxy.jdbc.SQLXMLProxyImpl。
pub(crate) struct DruidSqlXml {
    state: Arc<DruidSqlXmlState>,
    registry: Arc<StandardResourceRegistry>,
    capabilities: ResourceOperations,
}
impl std::fmt::Debug for DruidSqlXml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DruidSqlXml")
            .field("state", &self.state.context.state())
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}
impl DruidSqlXml {
    /// 接管 payload、上下文及 registry；在 admission 中登记后返回独占标准代理。
    /// 拒绝登记时由 registry 清理对象，并返回原始登记错误。
    pub(crate) async fn register(
        payload: Box<dyn SqlXml>,
        context: Arc<ResourceContext>,
        registry: Arc<StandardResourceRegistry>,
        admission: &StandardResourceAdmission,
    ) -> Result<Self, RdbcError> {
        let driver_capabilities = payload.capabilities();
        let mut capabilities = ResourceOperations::empty();
        if driver_capabilities.contains(ResourceOperations::READ)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::READ)
        {
            capabilities |= ResourceOperations::READ;
        }
        if driver_capabilities.contains(ResourceOperations::WRITE)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::WRITE)
        {
            capabilities |= ResourceOperations::WRITE;
        }

        if driver_capabilities.contains(ResourceOperations::STREAM)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::STREAM)
        {
            capabilities |= ResourceOperations::STREAM;
        }

        if driver_capabilities.contains(ResourceOperations::FREE)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::FREE)
        {
            capabilities |= ResourceOperations::FREE;
        }
        let state = Arc::new(DruidSqlXmlState::new(payload, context));
        let actor: Arc<dyn StandardResourceHandle> = state.clone();
        let result = registry.register_batch(admission, vec![actor]).await;
        admission.finish();
        result?;
        Ok(Self {
            state,
            registry,
            capabilities,
        })
    }
    fn require(
        &self,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<(), RdbcError> {
        // 先检查生命周期，再检查能力，保证关闭错误优先且不触碰底层资源。
        self.state
            .context
            .require(ResourceRuntimeCapabilities::empty(), operation)?;
        if self
            .state
            .registration()
            .is_none_or(|registration| registration.cleanup_requested())
        {
            return Err(RdbcError::rdbc_resource_closed("Blob"));
        }
        if !self.capabilities.contains(capability) {
            return Err(RdbcError::feature_not_supported(operation));
        }
        Ok(())
    }
    async fn payload(
        &self,
        read: bool,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<tokio::sync::MutexGuard<'_, DruidSqlXmlPayload>, RdbcError> {
        self.require(capability, operation)?;
        let mut payload = self.state.payload.lock().await;
        // 等待锁期间父级可能开始清理，因此持锁后再次验证。
        self.require(capability, operation)?;
        payload.admit(read)?;
        Ok(payload)
    }
}
#[async_trait::async_trait]
impl SqlXml for DruidSqlXml {
    fn capabilities(&self) -> ResourceOperations {
        self.capabilities
    }
    async fn binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        let result = self
            .payload(
                true,
                ResourceOperations::READ | ResourceOperations::STREAM,
                "SQLXML#binary_stream",
            )
            .await?
            .driver
            .binary_stream()
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn set_binary_stream(&mut self) -> Result<RdbcOutputStream, RdbcError> {
        let result = self
            .payload(
                false,
                ResourceOperations::WRITE | ResourceOperations::STREAM,
                "SQLXML#set_binary_stream",
            )
            .await?
            .driver
            .set_binary_stream()
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        let result = self
            .payload(
                true,
                ResourceOperations::READ | ResourceOperations::STREAM,
                "SQLXML#character_stream",
            )
            .await?
            .driver
            .character_stream()
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn set_character_stream(&mut self) -> Result<RdbcWriter, RdbcError> {
        let result = self
            .payload(
                false,
                ResourceOperations::WRITE | ResourceOperations::STREAM,
                "SQLXML#set_character_stream",
            )
            .await?
            .driver
            .set_character_stream()
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn string(&mut self) -> Result<RdbcString, RdbcError> {
        let result = self
            .payload(true, ResourceOperations::READ, "SQLXML#string")
            .await?
            .driver
            .string()
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn set_string(&mut self, value: &RdbcString) -> Result<(), RdbcError> {
        let result = self
            .payload(false, ResourceOperations::WRITE, "SQLXML#set_string")
            .await?
            .driver
            .set_string(value)
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn source(
        &mut self,
        representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlSource, RdbcError> {
        let result = self
            .payload(true, ResourceOperations::READ, "SQLXML#source")
            .await?
            .driver
            .source(representation)
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn result(
        &mut self,
        representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlResult, RdbcError> {
        let result = self
            .payload(false, ResourceOperations::WRITE, "SQLXML#result")
            .await?
            .driver
            .result(representation)
            .await;
        self.state.context.classify_operation_result(result)
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        // free 的 future 只等待清理结果，驱动释放与 owner 通知均由 coordinator 持有。
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
