//! Standard live resource 在raw acquisition前取得的generation permit。

use super::standard_resource_registry::StandardResourceRegistry;
use std::sync::{Arc, Weak};

/// 在一个logical generation内占用尚未完成的resource admission。
pub(crate) struct StandardResourceAdmission {
    registry: Weak<StandardResourceRegistry>,
    generation: u64,
}

impl StandardResourceAdmission {
    pub(crate) const fn new(registry: Weak<StandardResourceRegistry>, generation: u64) -> Self {
        Self {
            registry,
            generation,
        }
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn belongs_to(&self, registry: &Arc<StandardResourceRegistry>) -> bool {
        self.registry
            .upgrade()
            .is_some_and(|owner| Arc::ptr_eq(&owner, registry))
    }
}

impl Drop for StandardResourceAdmission {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            registry.finish_admission(self.generation);
        }
    }
}
