use crate::connection::Connection;
use crate::driver::{ConnectionFactoryProvider, ConnectionProperties, DriverExtensionDescriptor};
use crate::error::RdbcError;
use std::sync::Arc;

use crate::driver::Driver;

/// 纯 standard provider registry。
///
/// Registry 负责 JDBC URL negotiation 和具名连接工厂 provider 查找；产品级档案编排留在上层 crate。
#[derive(Clone, Default)]
pub struct DriverRegistry {
    descriptors: Vec<DriverExtensionDescriptor>,
    factory_providers: Vec<(String, Arc<dyn ConnectionFactoryProvider>)>,
}

impl std::fmt::Debug for DriverRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DriverRegistry")
            .field("descriptors", &self.descriptors)
            .field("factory_provider_count", &self.factory_providers.len())
            .finish()
    }
}

impl DriverRegistry {
    /// 创建空 registry。
    pub const fn new() -> Self {
        Self {
            descriptors: Vec::new(),
            factory_providers: Vec::new(),
        }
    }

    /// 注册一个 standard provider descriptor。
    pub fn register(&mut self, descriptor: DriverExtensionDescriptor) {
        self.descriptors.push(descriptor);
    }

    /// 返回当前 provider descriptors。
    pub fn descriptors(&self) -> &[DriverExtensionDescriptor] {
        &self.descriptors
    }

    /// 注册或替换具名连接工厂 provider；同名 provider 的最新注册生效。
    pub fn register_factory_provider(
        &mut self,
        name: impl Into<String>,
        provider: Arc<dyn ConnectionFactoryProvider>,
    ) {
        let name = name.into();
        if let Some((_, existing)) = self
            .factory_providers
            .iter_mut()
            .find(|(registered, _)| registered == &name)
        {
            *existing = provider;
        } else {
            self.factory_providers.push((name, provider));
        }
    }

    /// 查找具名 provider；返回的独立 handle 可在不持有注册表借用的情况下调用。
    #[must_use]
    pub fn factory_provider(&self, name: &str) -> Option<Arc<dyn ConnectionFactoryProvider>> {
        self.factory_providers
            .iter()
            .find(|(registered, _)| registered == name)
            .map(|(_, provider)| Arc::clone(provider))
    }

    /// 返回当前具名工厂 provider 的数量。
    #[must_use]
    pub fn factory_provider_count(&self) -> usize {
        self.factory_providers.len()
    }

    /// 按注册顺序返回首个接受 URL 的标准驱动；参数为连接 URL，未匹配时返回 `None`。
    #[must_use]
    pub fn first_accepted_driver(&self, url: &str) -> Option<Arc<dyn Driver>> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.driver().accepts_url(url))
            .map(|descriptor| Arc::clone(descriptor.driver()))
    }

    /// 按注册顺序寻找第一个接受 URL 的 driver。
    pub async fn connect(
        &self,
        url: &str,
        properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        for descriptor in &self.descriptors {
            if !descriptor.driver().accepts_url(url) {
                continue;
            }
            if let Some(connection) = descriptor.driver().connect(url, properties).await? {
                return Ok(Some(connection));
            }
        }
        Ok(None)
    }
}
