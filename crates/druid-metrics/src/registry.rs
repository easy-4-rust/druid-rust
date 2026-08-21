use std::sync::Weak;

use druid::stats::DataSourceMonitorable;

/// Guard that keeps a datasource registered in the metrics runtime.
///
/// When dropped, the datasource is automatically unregistered.
#[derive(Debug)]
pub struct RegistrationGuard {
    datasource_id: u64,
    _weak_ref: Weak<dyn DataSourceMonitorable>,
    unregister_tx: Option<tokio::sync::mpsc::UnboundedSender<u64>>,
}

impl RegistrationGuard {
    pub fn new(
        datasource_id: u64,
        weak_ref: Weak<dyn DataSourceMonitorable>,
        unregister_tx: tokio::sync::mpsc::UnboundedSender<u64>,
    ) -> Self {
        Self {
            datasource_id,
            _weak_ref: weak_ref,
            unregister_tx: Some(unregister_tx),
        }
    }

    /// Returns the datasource ID associated with this guard.
    pub fn datasource_id(&self) -> u64 {
        self.datasource_id
    }
}

impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.unregister_tx.take() {
            // Best-effort unregister; receiver may be gone if runtime shut down.
            let _ = tx.send(self.datasource_id);
        }
    }
}

/// Internal registry entry holding a weak reference to a datasource.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub datasource_id: u64,
    pub weak_ref: Weak<dyn DataSourceMonitorable>,
}
