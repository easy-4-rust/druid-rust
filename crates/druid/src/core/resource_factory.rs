//! Druid resource context factory。

use super::resource_context::ResourceContext;
use super::resource_id::ResourceId;
use super::resource_kind::ResourceKind;
use super::resource_owner::ResourceOwner;
use super::resource_runtime_capabilities::ResourceRuntimeCapabilities;
use druid_rdbc::error::RdbcError;
use std::sync::Arc;

/// 创建并校验 Druid 私有 resource ownership context。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ResourceFactory;

#[allow(dead_code)]
impl ResourceFactory {
    pub(crate) fn detached(
        kind: ResourceKind,
        capabilities: ResourceRuntimeCapabilities,
    ) -> Arc<ResourceContext> {
        Arc::new(ResourceContext::detached(kind, capabilities))
    }

    pub(crate) fn owned(
        resource_id: ResourceId,
        kind: ResourceKind,
        capabilities: ResourceRuntimeCapabilities,
        owner: Arc<dyn ResourceOwner>,
    ) -> Result<Arc<ResourceContext>, RdbcError> {
        if capabilities.is_empty() {
            return Err(RdbcError::InvalidArgument(
                "resource capabilities must not be empty".to_string(),
            ));
        }
        Ok(Arc::new(ResourceContext::new(
            resource_id,
            kind,
            capabilities,
            owner,
        )))
    }
}
