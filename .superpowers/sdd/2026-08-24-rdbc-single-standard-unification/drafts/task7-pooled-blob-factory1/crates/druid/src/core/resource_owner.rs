//! Druid resource owner callback。

use super::resource_id::ResourceId;
use super::resource_kind::ResourceKind;
use druid_rdbc::error::RdbcError;

/// 接收 resource release/failure/abandon 通知的 Druid 私有 owner。
#[async_trait::async_trait]
pub(crate) trait ResourceOwner: Send + Sync {
    /// 检查所属租约是否仍允许资源操作；返回成功或所属连接关闭错误。
    fn ensure_open(&self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn resource_released(
        &self,
        _resource_id: &ResourceId,
        _kind: ResourceKind,
    ) -> Result<(), RdbcError> {
        Ok(())
    }

    fn resource_failed(&self, _resource_id: &ResourceId, _kind: ResourceKind, _error: &RdbcError) {}

    fn resource_abandoned(&self, _resource_id: &ResourceId, _kind: ResourceKind) {}
}
