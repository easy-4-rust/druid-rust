//! Shared one-shot owner-disposition notification for one logical connection lease.

use super::ConnectionRecycleDisposition;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub(crate) type OwnerDispositionObserver =
    Box<dyn FnOnce(ConnectionRecycleDisposition) + Send + 'static>;

/// Splits synchronous discard notification from the later real-holder transfer.
pub(crate) struct OwnerDispositionControl {
    signaled: AtomicBool,
    observer: Mutex<Option<OwnerDispositionObserver>>,
}

impl OwnerDispositionControl {
    pub(crate) fn new(observer: OwnerDispositionObserver) -> Self {
        Self {
            signaled: AtomicBool::new(false),
            observer: Mutex::new(Some(observer)),
        }
    }

    pub(crate) fn signal_discard(&self, disposition: ConnectionRecycleDisposition) -> bool {
        if self
            .signaled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        if let Some(observer) = self
            .observer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            observer(disposition);
        }
        true
    }

    pub(crate) fn is_signaled(&self) -> bool {
        self.signaled.load(Ordering::Acquire)
    }
}
