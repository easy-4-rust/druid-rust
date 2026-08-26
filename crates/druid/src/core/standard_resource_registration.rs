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

    /// 返回 registry 是否已把该 actor 转入 close/retry ownership。
    pub(crate) fn cleanup_requested(&self) -> bool {
        self.registry
            .upgrade()
            .is_none_or(|registry| registry.cleanup_requested(self.generation, self.id))
    }

    /// 返回 registration 所属 logical generation。
    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    /// 返回 generation 内单调递增的 resource id。
    pub(crate) const fn id(&self) -> u64 {
        self.id
    }
}
