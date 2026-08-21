use std::collections::{BTreeMap, VecDeque};
use std::future;
use std::sync::Arc;

use druid::stats::DruidTelemetrySnapshot;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::config::DruidMetricsConfig;
use crate::error::MetricsError;
use crate::protocol;
use crate::self_metrics::RuntimeSelfMetrics;
use crate::snapshot_encoder::encode_snapshot;

/// A pending snapshot waiting to be aggregated.
#[derive(Debug, Clone)]
pub struct PendingSnapshot {
    pub datasource_id: u64,
    pub snapshot: DruidTelemetrySnapshot,
}

/// Commands sent to the aggregator.
#[derive(Debug)]
pub enum AggregatorCommand {
    /// 立即刷新所有已经接受的快照。
    Flush {
        /// 仅当所有已接受快照成功进入 `batch_tx` 后返回成功。
        completion: oneshot::Sender<Result<(), MetricsError>>,
    },
}

/// 等待下游处理的有界聚合快照批次。
#[derive(Debug, Clone, PartialEq)]
pub struct AggregatedBatch {
    /// 已完成隐私处理与数量限制的协议条目。
    pub entries: Vec<protocol::SnapshotEntry>,
    /// 是否为远端请求的全量快照；普通窗口、flush 与 shutdown 均为 `false`。
    pub is_full_snapshot: bool,
}

/// 运行聚合任务。
///
/// 在合并窗口内按数据源 ID 保留最新快照，并通过有界通道向下游发送批次。
/// 通道已满时等待下游形成背压；通道关闭时停止聚合并保留未刷新数量。
/// 显式 flush、输入关闭和取消都会先发送当前待处理快照。
pub(crate) async fn run_aggregator(
    mut snapshot_rx: mpsc::Receiver<PendingSnapshot>,
    batch_tx: mpsc::Sender<AggregatedBatch>,
    mut command_rx: mpsc::Receiver<AggregatorCommand>,
    cancel: CancellationToken,
    config: DruidMetricsConfig,
    self_metrics: Arc<RuntimeSelfMetrics>,
) -> Result<(), MetricsError> {
    let mut pending = BTreeMap::<u64, PendingSnapshot>::new();
    let mut flush_deadline = None;
    let mut command_channel_open = true;

    loop {
        if flush_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            flush_pending(&mut pending, &batch_tx, &config, &self_metrics).await?;
            flush_deadline = None;
            continue;
        }

        let next_deadline = flush_deadline;
        let window_elapsed = async move {
            if let Some(deadline) = next_deadline {
                tokio::time::sleep_until(deadline).await;
            } else {
                future::pending::<()>().await;
            }
        };
        tokio::pin!(window_elapsed);

        enum Event {
            Cancelled,
            Command(Option<AggregatorCommand>),
            Snapshot(Option<PendingSnapshot>),
            WindowElapsed,
        }

        let event = tokio::select! {
            biased;
            () = cancel.cancelled() => Event::Cancelled,
            () = &mut window_elapsed => Event::WindowElapsed,
            command = command_rx.recv(), if command_channel_open => Event::Command(command),
            snapshot = snapshot_rx.recv() => Event::Snapshot(snapshot),
        };

        match event {
            Event::Cancelled => {
                while let Ok(snapshot) = snapshot_rx.try_recv() {
                    insert_snapshot(&mut pending, snapshot, &self_metrics);
                }
                flush_pending(&mut pending, &batch_tx, &config, &self_metrics).await?;
                return Ok(());
            }
            Event::Command(Some(AggregatorCommand::Flush { completion })) => {
                while let Ok(snapshot) = snapshot_rx.try_recv() {
                    insert_snapshot(&mut pending, snapshot, &self_metrics);
                }
                if let Err(error) =
                    flush_pending(&mut pending, &batch_tx, &config, &self_metrics).await
                {
                    let _ =
                        completion.send(Err(output_closed_error(self_metrics.pending_snapshots())));
                    return Err(error);
                }
                let _ = completion.send(Ok(()));
                flush_deadline = None;
            }
            Event::WindowElapsed => {
                flush_pending(&mut pending, &batch_tx, &config, &self_metrics).await?;
                flush_deadline = None;
            }
            Event::Command(None) => {
                command_channel_open = false;
            }
            Event::Snapshot(Some(snapshot)) => {
                let was_empty = pending.is_empty();
                insert_snapshot(&mut pending, snapshot, &self_metrics);
                if was_empty {
                    flush_deadline = Some(Instant::now() + config.coalesce_window);
                }
                if pending.len() >= config.batch_size {
                    flush_pending(&mut pending, &batch_tx, &config, &self_metrics).await?;
                    flush_deadline = None;
                }
            }
            Event::Snapshot(None) => {
                flush_pending(&mut pending, &batch_tx, &config, &self_metrics).await?;
                return Ok(());
            }
        }
    }
}

fn insert_snapshot(
    pending: &mut BTreeMap<u64, PendingSnapshot>,
    snapshot: PendingSnapshot,
    self_metrics: &RuntimeSelfMetrics,
) {
    if pending.insert(snapshot.datasource_id, snapshot).is_some() {
        self_metrics.increment_coalesced_total();
    }
    self_metrics.set_pending_snapshots(pending.len());
}

async fn flush_pending(
    pending: &mut BTreeMap<u64, PendingSnapshot>,
    batch_tx: &mpsc::Sender<AggregatedBatch>,
    config: &DruidMetricsConfig,
    self_metrics: &RuntimeSelfMetrics,
) -> Result<(), MetricsError> {
    let mut entries = std::mem::take(pending)
        .into_values()
        .map(|pending| {
            encode_snapshot(
                pending.snapshot,
                config.max_sql_statements,
                config.sql_text_policy,
            )
        })
        .collect::<VecDeque<_>>();

    while !entries.is_empty() {
        let take = config.batch_size.min(entries.len());
        let batch_entries = entries.drain(..take).collect();
        let batch = AggregatedBatch {
            entries: batch_entries,
            is_full_snapshot: false,
        };
        if let Err(error) = batch_tx.send(batch).await {
            let unflushed = error.0.entries.len() + entries.len();
            self_metrics.set_pending_snapshots(unflushed);
            return Err(output_closed_error(unflushed));
        }
        self_metrics.increment_pending_export_batches();
    }

    self_metrics.set_pending_snapshots(0);
    Ok(())
}

fn output_closed_error(unflushed: usize) -> MetricsError {
    MetricsError::ChannelSend(format!(
        "aggregated batch output closed with {unflushed} unflushed entries"
    ))
}
