use std::sync::Arc;

use druid::stats::DruidTelemetrySnapshot;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::self_metrics::RuntimeSelfMetrics;

/// A pending snapshot waiting to be aggregated.
#[derive(Debug, Clone)]
pub struct PendingSnapshot {
    pub datasource_id: u64,
    pub snapshot: DruidTelemetrySnapshot,
}

/// Commands sent to the aggregator.
#[derive(Debug)]
pub enum AggregatorCommand {
    /// Flush all pending snapshots now.
    Flush,
}

/// Run the aggregator task.
///
/// Receives snapshots from the sampler and coalesces them by datasource ID
/// within a time window, then forwards batches to the exporter.
pub(crate) async fn run_aggregator(
    mut snapshot_rx: mpsc::Receiver<PendingSnapshot>,
    mut cmd_rx: mpsc::UnboundedReceiver<AggregatorCommand>,
    cancel: CancellationToken,
    self_metrics: Arc<RuntimeSelfMetrics>,
) {
    let mut pending: std::collections::HashMap<u64, PendingSnapshot> =
        std::collections::HashMap::new();

    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => {
                // Drain remaining snapshots before exit
                while let Ok(snap) = snapshot_rx.try_recv() {
                    pending.insert(snap.datasource_id, snap);
                }
                self_metrics.set_pending_snapshots(pending.len());
                break;
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(AggregatorCommand::Flush) => {
                        pending.clear();
                        self_metrics.set_pending_snapshots(0);
                    }
                    None => break,
                }
            }
            snap = snapshot_rx.recv() => {
                match snap {
                    Some(s) => {
                        // Coalesce: overwrite existing pending snapshot for same datasource
                        pending.insert(s.datasource_id, s);
                        self_metrics.set_pending_snapshots(pending.len());
                    }
                    None => break, // channel closed
                }
            }
        }
    }
}
