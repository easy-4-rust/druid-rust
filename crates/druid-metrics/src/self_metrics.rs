use std::sync::atomic::{AtomicU64, Ordering};

/// Internal runtime self-metrics (counters for the metrics pipeline itself).
#[derive(Debug)]
pub struct RuntimeSelfMetrics {
    /// Number of times `try_snapshot` returned Busy.
    snapshot_busy_total: AtomicU64,
    /// Number of snapshots dropped due to queue saturation (coalesced away).
    coalesced_total: AtomicU64,
    /// Number of snapshots currently pending in the aggregator.
    pending_snapshots: AtomicU64,
}

impl RuntimeSelfMetrics {
    pub fn new() -> Self {
        Self {
            snapshot_busy_total: AtomicU64::new(0),
            coalesced_total: AtomicU64::new(0),
            pending_snapshots: AtomicU64::new(0),
        }
    }

    pub fn increment_snapshot_busy_total(&self) {
        self.snapshot_busy_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_coalesced_total(&self) {
        self.coalesced_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot_busy_total(&self) -> u64 {
        self.snapshot_busy_total.load(Ordering::Relaxed)
    }

    pub fn coalesced_total(&self) -> u64 {
        self.coalesced_total.load(Ordering::Relaxed)
    }

    pub fn pending_snapshots(&self) -> usize {
        self.pending_snapshots.load(Ordering::Relaxed) as usize
    }

    pub fn set_pending_snapshots(&self, n: usize) {
        self.pending_snapshots.store(n as u64, Ordering::Relaxed);
    }
}

impl Default for RuntimeSelfMetrics {
    fn default() -> Self {
        Self::new()
    }
}
