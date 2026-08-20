use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::registry::RegistryEntry;
use crate::self_metrics::RuntimeSelfMetrics;

/// Run the sampler task.
///
/// Periodically polls all registered datasources via `try_snapshot()` and sends
/// snapshots through the bounded channel. Never blocks on a busy datasource.
/// Reads from the shared registry (written to by `runtime::register()`).
pub(crate) async fn run_sampler(
    registry: Arc<RwLock<Vec<RegistryEntry>>>,
    snapshot_tx: mpsc::Sender<crate::aggregator::PendingSnapshot>,
    cancel: CancellationToken,
    interval: std::time::Duration,
    self_metrics: Arc<RuntimeSelfMetrics>,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            _ = ticker.tick() => {
                sample_all(&registry, &snapshot_tx, &self_metrics).await;
            }
        }
    }
}

async fn sample_all(
    registry: &Arc<RwLock<Vec<RegistryEntry>>>,
    snapshot_tx: &mpsc::Sender<crate::aggregator::PendingSnapshot>,
    self_metrics: &RuntimeSelfMetrics,
) {
    // Take a snapshot of all entries under the read lock
    let entries: Vec<RegistryEntry> = {
        let guard = registry.read();
        guard.clone()
    };

    let mut dead_ids: Vec<u64> = Vec::new();

    for entry in &entries {
        let Some(source) = entry.weak_ref.upgrade() else {
            dead_ids.push(entry.datasource_id);
            continue;
        };

        match source.try_snapshot() {
            Ok(snapshot) => {
                let pending = crate::aggregator::PendingSnapshot {
                    datasource_id: entry.datasource_id,
                    snapshot,
                };
                // try_send is non-blocking: if the queue is full, we coalesce
                match snapshot_tx.try_send(pending) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        // Queue saturated: increment coalesced counter, don't block SQL
                        self_metrics.increment_coalesced_total();
                        self_metrics.increment_snapshot_busy_total();
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                }
            }
            Err(druid_core::stats::SnapshotUnavailable::Busy) => {
                self_metrics.increment_snapshot_busy_total();
            }
            Err(druid_core::stats::SnapshotUnavailable::Closed) => {
                dead_ids.push(entry.datasource_id);
            }
        }
    }

    // Remove dead entries (Weak upgraded to None)
    if !dead_ids.is_empty() {
        let mut guard = registry.write();
        guard.retain(|e| !dead_ids.contains(&e.datasource_id));
    }
}
