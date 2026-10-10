use super::druid_sql_xml_payload::DruidSqlXmlPayload;
use super::resource_context::ResourceContext;
use super::resource_state::ResourceState;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registration::StandardResourceRegistration;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::SqlXml;
use std::sync::{Arc, Mutex, OnceLock};

/// 由 registry 监管的XML 对象状态，独占 Send 非 Sync 驱动对象。
/// 对应 Java: com.alibaba.druid.proxy.jdbc.SQLXMLProxyImpl 的资源状态与释放职责。
pub(crate) struct DruidSqlXmlState {
    pub(crate) payload: tokio::sync::Mutex<DruidSqlXmlPayload>,
    pub(crate) context: Arc<ResourceContext>,
    registration: OnceLock<StandardResourceRegistration>,
    close_gate: tokio::sync::Mutex<()>,
    owner_error: Mutex<Option<RdbcError>>,
}
impl DruidSqlXmlState {
    /// 接管驱动对象与 owner 上下文；返回尚未登记的 actor。
    pub(crate) fn new(payload: Box<dyn SqlXml>, context: Arc<ResourceContext>) -> Self {
        Self {
            payload: tokio::sync::Mutex::new(DruidSqlXmlPayload::new(payload)),
            context,
            registration: OnceLock::new(),
            close_gate: tokio::sync::Mutex::new(()),
            owner_error: Mutex::new(None),
        }
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
impl StandardResourceHandle for DruidSqlXmlState {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        // 串行发布 Freed 与 owner 错误，晚到父级不得把通知失败误判为成功。
        let _gate = self.close_gate.lock().await;
        if let Some(error) = self.owner_error() {
            return Err(error);
        }
        let result = self
            .context
            .release_with(|| async { self.payload.lock().await.driver.free().await })
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
