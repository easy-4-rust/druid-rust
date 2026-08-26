//! Standard logical resource 的统一异步关闭合同。

use super::standard_resource_registration::StandardResourceRegistration;
use super::RdbcError;
use std::sync::OnceLock;

/// holder close cascade 可以拥有和关闭的 standard resource actor。
#[async_trait::async_trait]
pub(crate) trait StandardResourceHandle: Send + Sync {
    async fn close_resource(&self) -> Result<(), RdbcError>;

    fn is_resource_closed(&self) -> bool;

    fn registration_slot(&self) -> &OnceLock<StandardResourceRegistration>;

    fn bind_registration(&self, registration: StandardResourceRegistration) -> bool {
        self.registration_slot().set(registration).is_ok()
    }

    fn unregister_resource(&self) {
        if let Some(registration) = self.registration_slot().get() {
            registration.unregister();
        }
    }
}
