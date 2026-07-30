use std::sync::Arc;

use crate::core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};

use super::PoolInner;

/// 让池化连接回收阶段复用数据源统一校验策略的内部 Adapter。
///
/// Rust-only 对象；只实现 `validate/close`，不会创建连接，因此不会形成第二个
/// factory 或连接池。
pub(crate) struct PoolValidationFactory {
    pool: Arc<PoolInner>,
}

impl PoolValidationFactory {
    pub(crate) fn new(pool: Arc<PoolInner>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for PoolValidationFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "pool_validation_factory_create",
        })
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        if self.pool.test_connection_internal(connection).await {
            Ok(())
        } else {
            Err(DruidError::ValidationFailed(
                "testConnectionInternal returned false".to_owned(),
            ))
        }
    }

    async fn close(&self, connection: &mut Box<dyn PhysicalConnection>) -> Result<(), DruidError> {
        self.pool.factory.close(connection).await
    }
}
