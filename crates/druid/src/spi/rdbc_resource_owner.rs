use super::{RdbcResourceId, RdbcResourceKind};
use crate::core::DruidError;

/// Receives lifecycle and failure notifications from connection-bound RDBC resources.
///
/// Pool and Agent adapters use this hook to keep a physical connection leased until every
/// resource is released, deregister remote handles, and classify fatal driver errors.
#[async_trait::async_trait]
pub trait RdbcResourceOwner: Send + Sync {
    /// Completes owner-side cleanup after the resource access implementation releases its value.
    async fn resource_released(
        &self,
        _resource_id: &RdbcResourceId,
        _kind: RdbcResourceKind,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Observes an operation failure so the owner can classify or discard its connection.
    fn resource_failed(
        &self,
        _resource_id: &RdbcResourceId,
        _kind: RdbcResourceKind,
        _error: &DruidError,
    ) {
    }

    /// Records that the last resource handle disappeared without explicit asynchronous release.
    fn resource_abandoned(&self, _resource_id: &RdbcResourceId, _kind: RdbcResourceKind) {}
}
