//! Standard resource registry 的 generation-safe 注册令牌。

use super::standard_resource_registry::StandardResourceRegistry;
use std::sync::Weak;

#[derive(Clone)]
pub(crate) struct StandardResourceRegistration {
    registry: Weak<StandardResourceRegistry>,
    generation: u64,
    id: u64,
}

impl StandardResourceRegistration {
    pub(crate) const fn new(
        registry: Weak<StandardResourceRegistry>,
        generation: u64,
        id: u64,
    ) -> Self {
        Self {
            registry,
            generation,
            id,
        }
    }

    pub(crate) fn unregister(&self) {
        if let Some(registry) = self.registry.upgrade() {
            registry.unregister(self.generation, self.id);
        }
    }
}
