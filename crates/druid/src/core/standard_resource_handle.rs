//! Standard logical resource 的统一异步关闭合同。

use super::RdbcError;

/// holder close cascade 可以拥有和关闭的 standard resource actor。
#[async_trait::async_trait]
pub(crate) trait StandardResourceHandle: Send + Sync {
    async fn close_resource(&self) -> Result<(), RdbcError>;

    fn is_resource_closed(&self) -> bool;
}
