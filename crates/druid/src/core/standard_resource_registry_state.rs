//! Standard resource registry 的封闭状态机。

use super::standard_resource_handle::StandardResourceHandle;
use super::RdbcError;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::oneshot;

/// Registry 内一个 resource 的异步清理阶段。
#[derive(Clone, Copy)]
pub(crate) enum StandardResourceCleanupState {
    Open,
    Queued,
    Closing,
    Retry,
}

/// Registry 始终持有的 resource actor、重试状态与等待者。
pub(crate) struct StandardResourceRegistryEntry {
    pub(crate) resource: Arc<dyn StandardResourceHandle>,
    pub(crate) cleanup_state: StandardResourceCleanupState,
    pub(crate) cleanup_waiters: Vec<oneshot::Sender<Result<(), RdbcError>>>,
}

impl StandardResourceRegistryEntry {
    /// 创建一个由 registry 接管的 resource entry。
    pub(crate) fn new(
        resource: Arc<dyn StandardResourceHandle>,
        cleanup_state: StandardResourceCleanupState,
    ) -> Self {
        Self {
            resource,
            cleanup_state,
            cleanup_waiters: Vec::new(),
        }
    }
}

pub(crate) enum StandardResourceRegistryState {
    Open {
        generation: u64,
        resources: BTreeMap<u64, StandardResourceRegistryEntry>,
        pending: usize,
        coordinator_running: bool,
    },
    Closing {
        generation: u64,
        resources: BTreeMap<u64, StandardResourceRegistryEntry>,
        pending: usize,
        close_error: Option<RdbcError>,
        coordinator_running: bool,
    },
    Closed {
        generation: u64,
        resources: BTreeMap<u64, StandardResourceRegistryEntry>,
        close_error: Option<RdbcError>,
        coordinator_running: bool,
    },
}

impl Default for StandardResourceRegistryState {
    fn default() -> Self {
        Self::Open {
            generation: 0,
            resources: BTreeMap::new(),
            pending: 0,
            coordinator_running: false,
        }
    }
}
