use super::{
    RdbcResourceAccess, RdbcResourceCapabilities, RdbcResourceId, RdbcResourceKind,
    RdbcResourceOwner, RdbcResourceState,
};
use crate::core::DruidError;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// Shared identity, capability, connection-ownership, and lifecycle state for an RDBC resource.
pub struct RdbcResourceContext {
    resource_id: RdbcResourceId,
    kind: RdbcResourceKind,
    capabilities: RdbcResourceCapabilities,
    owner: Option<Arc<dyn RdbcResourceOwner>>,
    state: AtomicU8,
    state_changed: Notify,
}

impl RdbcResourceContext {
    /// Creates a connection-bound resource context owned by a pool or driver adapter.
    #[must_use]
    pub fn new(
        resource_id: RdbcResourceId,
        kind: RdbcResourceKind,
        capabilities: RdbcResourceCapabilities,
        owner: Arc<dyn RdbcResourceOwner>,
    ) -> Self {
        Self {
            resource_id,
            kind,
            capabilities,
            owner: Some(owner),
            state: AtomicU8::new(RdbcResourceState::Open.code()),
            state_changed: Notify::new(),
        }
    }

    /// Creates a locally identified resource that does not pin a pooled connection.
    #[must_use]
    pub fn detached(kind: RdbcResourceKind, capabilities: RdbcResourceCapabilities) -> Self {
        Self {
            resource_id: RdbcResourceId::local(),
            kind,
            capabilities,
            owner: None,
            state: AtomicU8::new(RdbcResourceState::Open.code()),
            state_changed: Notify::new(),
        }
    }

    /// Returns the stable resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &RdbcResourceId {
        &self.resource_id
    }

    /// Returns the standard resource kind.
    #[must_use]
    pub const fn kind(&self) -> RdbcResourceKind {
        self.kind
    }

    /// Returns the enabled operation set.
    #[must_use]
    pub const fn capabilities(&self) -> RdbcResourceCapabilities {
        self.capabilities
    }

    /// Returns the current lifecycle state.
    #[must_use]
    pub fn state(&self) -> RdbcResourceState {
        RdbcResourceState::from_code(self.state.load(Ordering::Acquire))
    }

    /// Returns whether the resource has been explicitly released.
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.state() == RdbcResourceState::Freed
    }

    /// Rejects access after release or owner invalidation.
    pub fn ensure_open(&self) -> Result<(), DruidError> {
        if self.state() == RdbcResourceState::Open {
            Ok(())
        } else {
            Err(DruidError::rdbc_resource_closed(self.kind.standard_name()))
        }
    }

    /// Requires an operation capability and an open resource.
    pub fn require(
        &self,
        capability: RdbcResourceCapabilities,
        operation: &'static str,
    ) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.require_capability(capability, operation)
    }

    /// Requires an operation capability without changing `free()` idempotence semantics.
    pub fn require_capability(
        &self,
        capability: RdbcResourceCapabilities,
        operation: &'static str,
    ) -> Result<(), DruidError> {
        if self.capabilities.contains(capability) {
            Ok(())
        } else {
            Err(DruidError::feature_not_supported(operation))
        }
    }

    /// Preserves an access result while notifying the owner about failures.
    pub fn observe<T>(&self, result: Result<T, DruidError>) -> Result<T, DruidError> {
        if let Err(error) = &result {
            if let Some(owner) = &self.owner {
                owner.resource_failed(&self.resource_id, self.kind, error);
            }
        }
        result
    }

    /// Releases the resource exactly once across all cloned handles.
    pub async fn free<A>(&self, access: &A) -> Result<(), DruidError>
    where
        A: RdbcResourceAccess + ?Sized,
    {
        loop {
            // Register before observing the state so a concurrent release cannot be missed.
            let state_changed = self.state_changed.notified();
            tokio::pin!(state_changed);
            state_changed.as_mut().enable();
            match self.state() {
                RdbcResourceState::Freed => return Ok(()),
                RdbcResourceState::Invalid => {
                    return Err(DruidError::rdbc_resource_closed(self.kind.standard_name()));
                }
                RdbcResourceState::Releasing => {
                    state_changed.await;
                }
                RdbcResourceState::Open => {
                    if self
                        .state
                        .compare_exchange(
                            RdbcResourceState::Open.code(),
                            RdbcResourceState::Releasing.code(),
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
                .store(RdbcResourceState::Open.code(), Ordering::Release);
            self.state_changed.notify_waiters();
            if let Some(owner) = &self.owner {
                owner.resource_failed(&self.resource_id, self.kind, &error);
            }
            return Err(error);
        }

        let owner_result = if let Some(owner) = &self.owner {
            owner.resource_released(&self.resource_id, self.kind).await
        } else {
            Ok(())
        };
        self.state
            .store(RdbcResourceState::Freed.code(), Ordering::Release);
        self.state_changed.notify_waiters();
        owner_result
    }

    /// Invalidates the resource when its owning connection or transaction ends.
    pub fn invalidate(&self) {
        self.state
            .store(RdbcResourceState::Invalid.code(), Ordering::Release);
        self.state_changed.notify_waiters();
    }
}

impl fmt::Debug for RdbcResourceContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcResourceContext")
            .field("resource_id", &self.resource_id)
            .field("kind", &self.kind)
            .field("capabilities", &self.capabilities)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl Drop for RdbcResourceContext {
    fn drop(&mut self) {
        if self.state() == RdbcResourceState::Open {
            if let Some(owner) = &self.owner {
                owner.resource_abandoned(&self.resource_id, self.kind);
            }
        }
    }
}
