//! Checkout-scoped poison control shared by SQLx external-pool bridges.

use druid_rdbc::connection::Connection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

struct LeaseState {
    poisoned: AtomicBool,
    rejection_recorded: AtomicBool,
    manager_rejection_count: Weak<AtomicU64>,
}

type LeaseRegistry = HashMap<usize, Arc<LeaseState>>;

fn registry() -> &'static Mutex<LeaseRegistry> {
    static REGISTRY: OnceLock<Mutex<LeaseRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn connection_key(connection: &dyn Connection) -> usize {
    let pointer: *const dyn Connection = connection;
    pointer.cast::<()>() as usize
}

/// Private control retained for exactly one checked-out external-pool lease.
pub(crate) struct ExternalPoolLeaseControl {
    key: usize,
    state: Arc<LeaseState>,
}

impl ExternalPoolLeaseControl {
    pub(crate) fn register(
        connection: &dyn Connection,
        manager_rejection_count: Arc<AtomicU64>,
    ) -> Self {
        let key = connection_key(connection);
        let state = Arc::new(LeaseState {
            poisoned: AtomicBool::new(false),
            rejection_recorded: AtomicBool::new(false),
            manager_rejection_count: Arc::downgrade(&manager_rejection_count),
        });
        let mut controls = registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        controls.retain(|_, control| {
            Arc::strong_count(control) > 1 || control.manager_rejection_count.upgrade().is_some()
        });
        controls.insert(key, Arc::clone(&state));
        drop(controls);
        Self { key, state }
    }

    pub(crate) fn poison(&self) {
        self.state.poisoned.store(true, Ordering::Release);
    }
}

impl Drop for ExternalPoolLeaseControl {
    fn drop(&mut self) {
        if self.state.poisoned.load(Ordering::Acquire) {
            return;
        }
        let mut controls = registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let owns_entry = controls
            .get(&self.key)
            .is_some_and(|control| Arc::ptr_eq(control, &self.state));
        if owns_entry {
            controls.remove(&self.key);
        }
    }
}

/// Returns whether the current checkout was poisoned and records one manager rejection.
pub(crate) fn take_poisoned(connection: &dyn Connection) -> bool {
    let key = connection_key(connection);
    let mut controls = registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(state) = controls.get(&key).cloned() else {
        controls.remove(&key);
        return false;
    };
    if !state.poisoned.load(Ordering::Acquire) {
        return false;
    }
    if !state.rejection_recorded.swap(true, Ordering::AcqRel) {
        if let Some(counter) = state.manager_rejection_count.upgrade() {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }
    controls.remove(&key);
    true
}

#[cfg(test)]
mod lease_complete_tests;
