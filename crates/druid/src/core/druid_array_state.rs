use super::resource_context::ResourceContext;
use super::resource_state::ResourceState;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registration::StandardResourceRegistration;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::Array;
use std::sync::{Arc, Mutex, OnceLock};

/// 由 registry 监管的SQL数组状态，独占 Send 非 Sync 驱动对象。
/// 对应 Java: java.sql.Array（池化资源状态拆分） 的资源状态与释放职责。
pub(crate) struct DruidArrayState {
    pub(crate) payload: tokio::sync::Mutex<Box<dyn Array>>,
    pub(crate) context: Arc<ResourceContext>,
    registration: OnceLock<StandardResourceRegistration>,
    close_gate: tokio::sync::Mutex<()>,
    children: Mutex<Vec<Arc<super::druid_pooled_result_set::StandardResultSetResource>>>,
    owner_error: Mutex<Option<RdbcError>>,
}
impl DruidArrayState {
    /// 接管驱动对象与 owner 上下文；返回尚未登记的 actor。
    pub(crate) fn new(payload: Box<dyn Array>, context: Arc<ResourceContext>) -> Self {
        Self {
            payload: tokio::sync::Mutex::new(payload),
            context,
            registration: OnceLock::new(),
            close_gate: tokio::sync::Mutex::new(()),
            children: Mutex::new(Vec::new()),
            owner_error: Mutex::new(None),
        }
    }
    /// 同步接管新游标状态；参数为驱动游标的监管资源，无返回值。
    /// 对应 Java: Array#getResultSet 的独立资源交接。
    pub(crate) fn track_cursor(
        &self,
        cursor: Arc<super::druid_pooled_result_set::StandardResultSetResource>,
    ) {
        self.children
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(cursor);
    }
    /// 返回已登记令牌；未完成登记时返回 None。
    pub(crate) fn registration(&self) -> Option<&StandardResourceRegistration> {
        self.registration.get()
    }
    /// 等待正在发布的清理结果；已完成返回结果，未释放返回 None。
    pub(crate) async fn completed_release(&self) -> Option<Result<(), RdbcError>> {
        let _gate = self.close_gate.lock().await;
        if let Some(error) = self.owner_error() {
            return Some(Err(error));
        }
        (self.context.state() == ResourceState::Freed).then_some(Ok(()))
    }
    /// 返回终态 owner 通知错误的副本；驱动释放失败允许重试，不在此缓存。
    pub(crate) fn owner_error(&self) -> Option<RdbcError> {
        self.owner_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}
#[async_trait::async_trait]
impl StandardResourceHandle for DruidArrayState {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        // 串行发布 Freed 与 owner 错误，晚到父级不得把通知失败误判为成功。
        let _gate = self.close_gate.lock().await;
        if let Some(error) = self.owner_error() {
            return Err(error);
        }
        // 先等待当前 Array 操作交接游标，随后直接关闭子 actor，不能排队等待本协调器。
        let mut payload = self.payload.lock().await;
        let children = self
            .children
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for child in children {
            child.close_core().await?;
        }
        let result = self
            .context
            .release_with(|| async { payload.free().await })
            .await;
        if self.context.state() == ResourceState::Freed {
            if let Err(error) = &result {
                *self
                    .owner_error
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error.clone());
            } else {
                self.unregister_resource();
            }
        }
        result
    }
    fn is_resource_closed(&self) -> bool {
        self.context.state() == ResourceState::Freed
    }
    fn registration_slot(&self) -> &OnceLock<StandardResourceRegistration> {
        &self.registration
    }
}
