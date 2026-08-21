use super::RdbcResourceCapabilities;
use crate::core::DruidError;
use std::fmt;

/// Common access contract implemented by every driver-owned RDBC resource.
#[async_trait::async_trait]
pub trait RdbcResourceAccess: fmt::Debug + Send + Sync {
    /// Returns the operations supported by this concrete resource instance.
    fn capabilities(&self) -> RdbcResourceCapabilities;

    /// Releases the driver-owned value.
    ///
    /// Resources without a Java `free()` operation retain the default no-op implementation.
    async fn free(&self) -> Result<(), DruidError> {
        Ok(())
    }
}
