use super::druid_blob::DruidBlob;
use super::druid_clob::DruidClob;
use super::druid_n_clob::DruidNClob;
use super::druid_resource_owner::DruidResourceOwner;
use super::resource_context::ResourceContext;
use super::resource_id::ResourceId;
use super::resource_kind::ResourceKind;
use super::resource_runtime_capabilities::ResourceRuntimeCapabilities;
use super::standard_logical_policy::StandardLogicalPolicy;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_registry::StandardResourceRegistry;
use super::{BeforeFilter, DruidError, RegisteredLob};
use druid_rdbc::connection::Connection;
use druid_rdbc::resource::{Blob, Clob, NClob};
use std::sync::Arc;

/// 按注册顺序分派标准 LOB 创建，并在交给外层过滤器前完成资源登记。
/// 对应 Java: FilterChainImpl#connection_createBlob/createClob/createNClob。
pub struct StandardConnectionLobFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    connection: &'a mut dyn Connection,
    registry: Arc<StandardResourceRegistry>,
    policy: Arc<StandardLogicalPolicy>,
    first_admission: Option<Arc<StandardResourceAdmission>>,
    admission_anchor: Arc<StandardResourceAdmission>,
}

impl<'a> StandardConnectionLobFilterChain<'a> {
    /// 参数为过滤器、标准连接、资源注册表、连接策略及首次创建许可。
    pub(crate) fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        connection: &'a mut dyn Connection,
        registry: Arc<StandardResourceRegistry>,
        policy: Arc<StandardLogicalPolicy>,
        admission: Arc<StandardResourceAdmission>,
    ) -> Self {
        Self {
            filters,
            position: 0,
            connection,
            registry,
            policy,
            first_admission: Some(Arc::clone(&admission)),
            admission_anchor: admission,
        }
    }

    fn admission(&mut self) -> Arc<StandardResourceAdmission> {
        if let Some(admission) = self.first_admission.take() {
            admission
        } else {
            // 关闭竞态下仍把替换资源交给 registry 的拒绝清理队列。
            self.registry
                .begin_admission(self.policy.resource_generation())
                .map(Arc::new)
                .unwrap_or_else(|_| Arc::clone(&self.admission_anchor))
        }
    }

    /// 接管替换 Blob；参数为独占驱动资源，返回已由父连接监管的结果。
    pub async fn adopt_blob(
        &mut self,
        value: Box<dyn Blob>,
    ) -> Result<RegisteredLob<dyn Blob>, DruidError> {
        let admission = self.admission();
        let id = ResourceId::local();
        let owner = Arc::new(DruidResourceOwner::new(
            id.clone(),
            ResourceKind::Blob,
            Arc::clone(&self.policy),
        ));
        let context = Arc::new(ResourceContext::new(
            id,
            ResourceKind::Blob,
            ResourceRuntimeCapabilities::large_object(),
            owner,
        ));
        DruidBlob::register(value, context, Arc::clone(&self.registry), &admission)
            .await
            .map(|managed| RegisteredLob::new(Box::new(managed) as Box<dyn Blob>))
            .map_err(DruidError::from)
    }

    /// 接管替换 Clob；参数为独占驱动资源，返回已由父连接监管的结果。
    pub async fn adopt_clob(
        &mut self,
        value: Box<dyn Clob>,
    ) -> Result<RegisteredLob<dyn Clob>, DruidError> {
        let admission = self.admission();
        let id = ResourceId::local();
        let owner = Arc::new(DruidResourceOwner::new(
            id.clone(),
            ResourceKind::Clob,
            Arc::clone(&self.policy),
        ));
        let context = Arc::new(ResourceContext::new(
            id,
            ResourceKind::Clob,
            ResourceRuntimeCapabilities::large_object(),
            owner,
        ));
        DruidClob::register(value, context, Arc::clone(&self.registry), &admission)
            .await
            .map(|managed| RegisteredLob::new(Box::new(managed) as Box<dyn Clob>))
            .map_err(DruidError::from)
    }

    /// 接管替换 NClob；参数为独占驱动资源，返回已由父连接监管的结果。
    pub async fn adopt_n_clob(
        &mut self,
        value: Box<dyn NClob>,
    ) -> Result<RegisteredLob<dyn NClob>, DruidError> {
        let admission = self.admission();
        let id = ResourceId::local();
        let owner = Arc::new(DruidResourceOwner::new(
            id.clone(),
            ResourceKind::NClob,
            Arc::clone(&self.policy),
        ));
        let context = Arc::new(ResourceContext::new(
            id,
            ResourceKind::NClob,
            ResourceRuntimeCapabilities::large_object(),
            owner,
        ));
        DruidNClob::register(value, context, Arc::clone(&self.registry), &admission)
            .await
            .map(|managed| RegisteredLob::new(Box::new(managed) as Box<dyn NClob>))
            .map_err(DruidError::from)
    }

    /// 创建 Blob；过滤器可包围、短路或通过 adopt_blob 替换结果。
    pub async fn connection_create_blob(&mut self) -> Result<RegisteredLob<dyn Blob>, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.connection_create_blob(self).await
        } else {
            let value = self
                .connection
                .create_blob()
                .await
                .map_err(DruidError::from)?;
            self.adopt_blob(value).await
        }
    }

    /// 创建 Clob；过滤器可包围、短路或通过 adopt_clob 替换结果。
    pub async fn connection_create_clob(&mut self) -> Result<RegisteredLob<dyn Clob>, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.connection_create_clob(self).await
        } else {
            let value = self
                .connection
                .create_clob()
                .await
                .map_err(DruidError::from)?;
            self.adopt_clob(value).await
        }
    }

    /// 创建 NClob；过滤器可包围、短路或通过 adopt_n_clob 替换结果。
    pub async fn connection_create_n_clob(
        &mut self,
    ) -> Result<RegisteredLob<dyn NClob>, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.connection_create_n_clob(self).await
        } else {
            let value = self
                .connection
                .create_n_clob()
                .await
                .map_err(DruidError::from)?;
            self.adopt_n_clob(value).await
        }
    }
}
