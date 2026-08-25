//! Canonical ConnectionFactory 的 Task 6 transitional physical-connect envelope。

use super::{
    PhysicalConnection, PhysicalConnectionFactory, RdbcError, StandardConnectionCompatibility,
};
use std::sync::Arc;

/// 让现有 Druid physical-connect Filter 链观察 canonical factory 创建过程。
pub(crate) struct StandardConnectionFactoryCompatibility {
    factory: Arc<dyn druid_rdbc::driver::ConnectionFactory>,
}

impl StandardConnectionFactoryCompatibility {
    pub(crate) fn new(factory: Arc<dyn druid_rdbc::driver::ConnectionFactory>) -> Self {
        Self { factory }
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for StandardConnectionFactoryCompatibility {
    fn driver_name(&self) -> Option<&str> {
        Some(self.factory.name())
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, RdbcError> {
        Ok(Box::new(StandardConnectionCompatibility::new(
            self.factory.create().await?,
        )?))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), RdbcError> {
        connection.ping().await
    }
}
