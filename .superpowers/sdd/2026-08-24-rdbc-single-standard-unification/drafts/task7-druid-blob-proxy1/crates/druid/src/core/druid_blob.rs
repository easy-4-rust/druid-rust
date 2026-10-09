use super::druid_blob_state::DruidBlobState;
use super::resource_context::ResourceContext;
use super::resource_runtime_capabilities::ResourceRuntimeCapabilities;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registry::StandardResourceRegistry;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{Blob, RdbcInputStream, RdbcOutputStream, ResourceOperations};
use std::sync::Arc;

/// 连接监管的标准二进制大对象代理；驱动操作独占借用，释放交给 registry。
/// 对应 Java: com.alibaba.druid.proxy.jdbc.BlobProxyImpl。
pub(crate) struct DruidBlob {
    state: Arc<DruidBlobState>,
    registry: Arc<StandardResourceRegistry>,
    capabilities: ResourceOperations,
}
impl std::fmt::Debug for DruidBlob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DruidBlob")
            .field("state", &self.state.context.state())
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}
impl DruidBlob {
    /// 接管 payload、上下文及 registry；在 admission 中登记后返回独占标准代理。
    /// 拒绝登记时由 registry 清理对象，并返回原始登记错误。
    pub(crate) async fn register(
        payload: Box<dyn Blob>,
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
        if driver_capabilities.contains(ResourceOperations::SEARCH)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::SEARCH)
        {
            capabilities |= ResourceOperations::SEARCH;
        }
        if driver_capabilities.contains(ResourceOperations::TRUNCATE)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::TRUNCATE)
        {
            capabilities |= ResourceOperations::TRUNCATE;
        }
        if driver_capabilities.contains(ResourceOperations::STREAM)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::STREAM)
        {
            capabilities |= ResourceOperations::STREAM;
        }
        if driver_capabilities.contains(ResourceOperations::RANGE)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::RANGE)
        {
            capabilities |= ResourceOperations::RANGE;
        }
        if driver_capabilities.contains(ResourceOperations::FREE)
            && context
                .capabilities()
                .contains(ResourceRuntimeCapabilities::FREE)
        {
            capabilities |= ResourceOperations::FREE;
        }
        let state = Arc::new(DruidBlobState::new(payload, context));
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
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<tokio::sync::MutexGuard<'_, Box<dyn Blob>>, RdbcError> {
        self.require(capability, operation)?;
        let payload = self.state.payload.lock().await;
        // 等待锁期间父级可能开始清理，因此持锁后再次验证。
        self.require(capability, operation)?;
        Ok(payload)
    }
}
#[async_trait::async_trait]
impl Blob for DruidBlob {
    fn capabilities(&self) -> ResourceOperations {
        self.capabilities
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        self.payload(ResourceOperations::READ, "Blob#length")
            .await?
            .length()
            .await
    }
    async fn get_bytes(&mut self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError> {
        self.payload(ResourceOperations::READ, "Blob#get_bytes")
            .await?
            .get_bytes(position, length)
            .await
    }
    async fn get_binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        self.payload(
            ResourceOperations::READ | ResourceOperations::STREAM,
            "Blob#get_binary_stream",
        )
        .await?
        .get_binary_stream()
        .await
    }
    async fn position_bytes(
        &mut self,
        pattern: &[u8],
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.payload(ResourceOperations::SEARCH, "Blob#position_bytes")
            .await?
            .position_bytes(pattern, start)
            .await
    }
    async fn position_blob(
        &mut self,
        pattern: &mut dyn Blob,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.payload(ResourceOperations::SEARCH, "Blob#position_blob")
            .await?
            .position_blob(pattern, start)
            .await
    }
    async fn set_bytes(&mut self, position: i64, bytes: &[u8]) -> Result<i32, RdbcError> {
        self.payload(ResourceOperations::WRITE, "Blob#set_bytes")
            .await?
            .set_bytes(position, bytes)
            .await
    }
    async fn set_bytes_range(
        &mut self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, RdbcError> {
        self.payload(
            ResourceOperations::WRITE | ResourceOperations::RANGE,
            "Blob#set_bytes_range",
        )
        .await?
        .set_bytes_range(position, bytes, offset, length)
        .await
    }
    async fn set_binary_stream(&mut self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        self.payload(
            ResourceOperations::WRITE | ResourceOperations::STREAM,
            "Blob#set_binary_stream",
        )
        .await?
        .set_binary_stream(position)
        .await
    }
    async fn truncate(&mut self, length: i64) -> Result<(), RdbcError> {
        self.payload(ResourceOperations::TRUNCATE, "Blob#truncate")
            .await?
            .truncate(length)
            .await
    }
    async fn get_binary_stream_range(
        &mut self,
        position: i64,
        length: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        self.payload(
            ResourceOperations::READ | ResourceOperations::STREAM | ResourceOperations::RANGE,
            "Blob#get_binary_stream_range",
        )
        .await?
        .get_binary_stream_range(position, length)
        .await
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
