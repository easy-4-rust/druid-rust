use crate::driver::{ConnectionFactory, Driver};
use std::sync::Arc;

/// 一个纯标准 driver provider descriptor。
///
/// 该描述符只包含 standard driver/factory，不承载产品 validation、exception policy。
#[derive(Clone)]
pub struct DriverExtensionDescriptor {
    name: String,
    driver: Arc<dyn Driver>,
    factory: Option<Arc<dyn ConnectionFactory>>,
}

impl DriverExtensionDescriptor {
    /// 创建 provider descriptor。
    pub fn new(
        name: impl Into<String>,
        driver: Arc<dyn Driver>,
        factory: Option<Arc<dyn ConnectionFactory>>,
    ) -> Self {
        Self {
            name: name.into(),
            driver,
            factory,
        }
    }

    /// 返回 provider name。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回 standard driver handle。
    pub fn driver(&self) -> &Arc<dyn Driver> {
        &self.driver
    }

    /// 返回 optional standard factory handle。
    pub fn factory(&self) -> Option<&Arc<dyn ConnectionFactory>> {
        self.factory.as_ref()
    }
}

impl std::fmt::Debug for DriverExtensionDescriptor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DriverExtensionDescriptor")
            .field("name", &self.name)
            .field("has_factory", &self.factory.is_some())
            .finish_non_exhaustive()
    }
}
