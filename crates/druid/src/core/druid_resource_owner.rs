use super::resource_id::ResourceId;
use super::resource_kind::ResourceKind;
use super::resource_owner::ResourceOwner;
use super::standard_logical_policy::StandardLogicalPolicy;
use druid_rdbc::error::RdbcError;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// 将一个连接绑定资源的通知与错误交给同一租约策略。
/// 对应 Java: com.alibaba.druid.proxy.jdbc.BlobProxyImpl 的所属连接异常处理职责。
pub(crate) struct DruidResourceOwner {
    resource_id: ResourceId,
    kind: ResourceKind,
    policy: Arc<StandardLogicalPolicy>,
    released: AtomicBool,
}
impl DruidResourceOwner {
    /// 绑定资源身份、类型和所属租约策略；返回尚未释放的 owner。
    pub(crate) fn new(
        resource_id: ResourceId,
        kind: ResourceKind,
        policy: Arc<StandardLogicalPolicy>,
    ) -> Self {
        Self {
            resource_id,
            kind,
            policy,
            released: AtomicBool::new(false),
        }
    }
    fn matches(&self, id: &ResourceId, kind: ResourceKind) -> bool {
        self.resource_id == *id && self.kind == kind
    }
}
#[async_trait::async_trait]
impl ResourceOwner for DruidResourceOwner {
    fn ensure_open(&self) -> Result<(), RdbcError> {
        self.policy.ensure_open()
    }
    async fn resource_released(
        &self,
        id: &ResourceId,
        kind: ResourceKind,
    ) -> Result<(), RdbcError> {
        if !self.matches(id, kind) {
            self.policy.discard_resource_lease();
            return Err(RdbcError::Other(
                "resource release owner identity mismatch".to_owned(),
            ));
        }
        // 记录真实资源完成通知；重复通知不再次改变所属连接的处置。
        self.released.store(true, Ordering::Release);
        Ok(())
    }
    fn resource_failed(&self, id: &ResourceId, kind: ResourceKind, error: &RdbcError) {
        if !self.matches(id, kind) {
            self.policy.discard_resource_lease();
            return;
        }
        if !self.released.load(Ordering::Acquire) {
            self.policy.classify_resource_error(error);
        }
    }
    fn resource_abandoned(&self, id: &ResourceId, kind: ResourceKind) {
        if !self.matches(id, kind) || !self.released.load(Ordering::Acquire) {
            // 未收到完成通知就失去资源所有权，连接不得返回可复用队列。
            self.policy.discard_resource_lease();
        }
    }
}
