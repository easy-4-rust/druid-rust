use crate::error::RdbcError;
use crate::resource::{
    RdbcResourceAccess, ResourceIdentity, ResourceLifecycleState, ResourceOperations, ResourceType,
};
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::Notify;

/// Shared identity, capability, connection-ownership, and lifecycle state for an RDBC resource.
pub(crate) struct ResourceLifecycle {
    resource_id: ResourceIdentity,
    kind: ResourceType,
    capabilities: ResourceOperations,
    state: AtomicU8,
    state_changed: Notify,
}

impl ResourceLifecycle {
    /// Creates a locally identified resource that does not pin a pooled connection.
    #[must_use]
    pub fn detached(kind: ResourceType, capabilities: ResourceOperations) -> Self {
        Self {
            resource_id: ResourceIdentity::local(),
            kind,
            capabilities,
            state: AtomicU8::new(ResourceLifecycleState::Open.code()),
            state_changed: Notify::new(),
        }
    }

    /// Returns the stable resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &ResourceIdentity {
        &self.resource_id
    }

    /// Returns the standard resource kind.
    #[must_use]
    pub const fn kind(&self) -> ResourceType {
        self.kind
    }

    /// Returns the enabled operation set.
    #[must_use]
    pub const fn capabilities(&self) -> ResourceOperations {
        self.capabilities
    }

    /// Returns the current lifecycle state.
    #[must_use]
    pub fn state(&self) -> ResourceLifecycleState {
        ResourceLifecycleState::from_code(self.state.load(Ordering::Acquire))
    }

    /// Returns whether the resource has been explicitly released.
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.state() == ResourceLifecycleState::Freed
    }

    /// Rejects access after release or owner invalidation.
    pub fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.state() == ResourceLifecycleState::Open {
            Ok(())
        } else {
            Err(RdbcError::rdbc_resource_closed(self.kind.standard_name()))
        }
    }

    /// Requires an operation capability and an open resource.
    pub fn require(
        &self,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.require_capability(capability, operation)
    }

    /// Requires an operation capability without changing `free()` idempotence semantics.
    pub fn require_capability(
        &self,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<(), RdbcError> {
        if self.capabilities.contains(capability) {
            Ok(())
        } else {
            Err(RdbcError::feature_not_supported(operation))
        }
    }

    /// Preserves the typed access result without introducing Druid owner callbacks.
    pub fn observe<T>(&self, result: Result<T, RdbcError>) -> Result<T, RdbcError> {
        result
    }

    /// Releases the resource exactly once across all cloned handles.
    pub async fn free<A>(&self, access: &A) -> Result<(), RdbcError>
    where
        A: RdbcResourceAccess + ?Sized,
    {
        loop {
            // Register before observing the state so a concurrent release cannot be missed.
            let state_changed = self.state_changed.notified();
            tokio::pin!(state_changed);
            state_changed.as_mut().enable();
            match self.state() {
                ResourceLifecycleState::Freed => return Ok(()),
                ResourceLifecycleState::Invalid => {
                    return Err(RdbcError::rdbc_resource_closed(self.kind.standard_name()));
                }
                ResourceLifecycleState::Releasing => {
                    state_changed.await;
                }
                ResourceLifecycleState::Open => {
                    if self
                        .state
                        .compare_exchange(
                            ResourceLifecycleState::Open.code(),
                            ResourceLifecycleState::Releasing.code(),
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        break;
                    }
                }
            }
        }

        if let Err(error) = access.free().await {
            self.state
                .store(ResourceLifecycleState::Open.code(), Ordering::Release);
            self.state_changed.notify_waiters();
            return Err(error);
        }

        self.state
            .store(ResourceLifecycleState::Freed.code(), Ordering::Release);
        self.state_changed.notify_waiters();
        Ok(())
    }
}

impl fmt::Debug for ResourceLifecycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceLifecycle")
            .field("resource_id", &self.resource_id)
            .field("kind", &self.kind)
            .field("capabilities", &self.capabilities)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}
