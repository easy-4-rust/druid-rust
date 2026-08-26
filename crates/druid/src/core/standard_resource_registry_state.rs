//! Standard resource registry 的封闭状态机。

use super::standard_resource_handle::StandardResourceHandle;
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) enum StandardResourceRegistryState {
    Open {
        generation: u64,
        resources: BTreeMap<u64, Arc<dyn StandardResourceHandle>>,
    },
    Closing {
        generation: u64,
        resources: BTreeMap<u64, Arc<dyn StandardResourceHandle>>,
    },
    Closed {
        generation: u64,
        resources: BTreeMap<u64, Arc<dyn StandardResourceHandle>>,
    },
}

impl Default for StandardResourceRegistryState {
    fn default() -> Self {
        Self::Open {
            generation: 0,
            resources: BTreeMap::new(),
        }
    }
}
