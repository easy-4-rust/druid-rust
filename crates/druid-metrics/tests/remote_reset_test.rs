//! Truthfulness tests for remote reset and full-snapshot commands.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use druid::stats::{
    DataSourceIdentity, DataSourceMonitorable, DruidTelemetrySnapshot, PoolSnapshot,
    SnapshotUnavailable,
};
use druid_metrics::config::SqlTextPolicy;
use druid_metrics::ingest_handler::IngestHandler;
use druid_metrics::protocol::{
    client_frame, command, server_frame, ClientFrame, Command, CommandAck, FrameHeader,
    RequestFullSnapshotCmd, ResetStatsCmd, ServerFrame, SnapshotBatch, SnapshotEntry,
};
use druid_metrics::push_worker::{PushEvent, TransportPair};
use druid_metrics::registry::{RegistryCommandExecutor, RegistryEntry};
use parking_lot::RwLock;
use serde_json::Value;
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;

struct ResetProbe {
    identity: DataSourceIdentity,
    reset_count: AtomicUsize,
    snapshot_count: AtomicUsize,
    reset_observed: Notify,
    snapshot_observed: Notify,
    snapshot_result: Result<(), SnapshotUnavailable>,
}

impl ResetProbe {
    fn new(id: u64) -> Self {
        Self {
            identity: DataSourceIdentity {
                id,
                name: format!("probe-{id}"),
                driver_name: Some("probe-driver".to_owned()),
            },
            reset_count: AtomicUsize::new(0),
            snapshot_count: AtomicUsize::new(0),
            reset_observed: Notify::new(),
            snapshot_observed: Notify::new(),
            snapshot_result: Ok(()),
        }
    }

    fn busy(id: u64) -> Self {
        Self {
            snapshot_result: Err(SnapshotUnavailable::Busy),
            ..Self::new(id)
        }
    }
}

impl DataSourceMonitorable for ResetProbe {
    fn name(&self) -> &str {
        &self.identity.name
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {
        self.reset_count.fetch_add(1, Ordering::SeqCst);
        self.reset_observed.notify_one();
    }

    fn identity(&self) -> DataSourceIdentity {
        self.identity.clone()
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        self.snapshot_result.clone()?;
        self.snapshot_count.fetch_add(1, Ordering::SeqCst);
        self.snapshot_observed.notify_one();
        Ok(DruidTelemetrySnapshot {
            identity: self.identity.clone(),
            pool_snapshot: PoolSnapshot {
                active_count: 1,
                idle_count: 2,
                max_active: 8,
                max_idle: 4,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 123,
        })
    }
}

fn command_frame(command_id: &str, payload: Option<command::Payload>) -> ServerFrame {
    ServerFrame {
        payload: Some(server_frame::Payload::Command(Command {
            header: Some(FrameHeader {
                protocol_version: 1,
                service_name: "server".into(),
                instance_id: "admin".into(),
                boot_id: "admin-boot".into(),
                stream_epoch: 1,
                sequence: 0,
                emitted_at_unix_ms: 0,
            }),
            command_id: command_id.to_owned(),
            payload,
        })),
    }
}

fn outbound_batch(datasource_id: u64) -> druid_metrics::aggregator::AggregatedBatch {
    druid_metrics::aggregator::AggregatedBatch {
        entries: vec![SnapshotEntry {
            datasource_id,
            datasource_name: format!("outbound-{datasource_id}"),
            driver_name: Some("test-driver".to_owned()),
            pool: None,
            sql_stats: Vec::new(),
            wall_check_count: 0,
            wall_deny_count: 0,
            wall_violation_count: 0,
            sampling_time_millis: 1,
        }],
        is_full_snapshot: false,
    }
}

fn executor_for(probe: &Arc<ResetProbe>) -> RegistryCommandExecutor {
    let source: Arc<dyn DataSourceMonitorable> = probe.clone();
    RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry {
            datasource_id: probe.identity.id,
            weak_ref: Arc::downgrade(&source),
        }])),
        100,
        100,
        SqlTextPolicy::FingerprintOnly,
    )
}

fn executor_with_expired_unrelated_entry(probe: &Arc<ResetProbe>) -> RegistryCommandExecutor {
    let live: Arc<dyn DataSourceMonitorable> = probe.clone();
    let expired: Arc<dyn DataSourceMonitorable> = Arc::new(ResetProbe::new(999));
    let expired_weak = Arc::downgrade(&expired);
    drop(expired);
    RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![
            RegistryEntry {
                datasource_id: probe.identity.id,
                weak_ref: Arc::downgrade(&live),
            },
            RegistryEntry {
                datasource_id: 999,
                weak_ref: expired_weak,
            },
        ])),
        100,
        100,
        SqlTextPolicy::FingerprintOnly,
    )
}

fn executor_for_probes(probes: &[Arc<ResetProbe>], batch_size: usize) -> RegistryCommandExecutor {
    let entries = probes
        .iter()
        .map(|probe| {
            let source: Arc<dyn DataSourceMonitorable> = probe.clone();
            RegistryEntry {
                datasource_id: probe.identity.id,
                weak_ref: Arc::downgrade(&source),
            }
        })
        .collect();
    RegistryCommandExecutor::new(
        Arc::new(RwLock::new(entries)),
        batch_size,
        100,
        SqlTextPolicy::FingerprintOnly,
    )
}

fn command_ack(
    frame: &druid_metrics::protocol::ClientFrame,
) -> &druid_metrics::protocol::CommandAck {
    match frame.payload.as_ref() {
        Some(client_frame::Payload::CommandAck(ack)) => ack,
        other => panic!("expected CommandAck, got {other:?}"),
    }
}

fn server_full_snapshot_frame(sequence: u64) -> ClientFrame {
    ClientFrame {
        payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
            header: Some(FrameHeader {
                protocol_version: 1,
                service_name: "svc".into(),
                instance_id: "i1".into(),
                boot_id: "b1".into(),
                stream_epoch: 100,
                sequence,
                emitted_at_unix_ms: 1_700_000_000_000,
            }),
            entries: Vec::new(),
            is_full_snapshot: true,
        })),
    }
}

fn server_command_ack(command_id: String, success: bool, error_message: &str) -> ClientFrame {
    ClientFrame {
        payload: Some(client_frame::Payload::CommandAck(CommandAck {
            header: Some(FrameHeader {
                protocol_version: 1,
                service_name: "svc".into(),
                instance_id: "i1".into(),
                boot_id: "b1".into(),
                stream_epoch: 100,
                sequence: 0,
                emitted_at_unix_ms: 0,
            }),
            command_id,
            success,
            error_message: error_message.to_owned(),
        })),
    }
}

#[tokio::test]
async fn reset_invokes_each_matching_datasource_once_before_success_ack() {
    let probe = Arc::new(ResetProbe::new(41));
    let pair = TransportPair::new(8);
    let (_batch_tx, batch_rx) = mpsc::channel::<PushEvent>(4);
    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 4);
    let worker = worker.with_command_executor(executor_for(&probe));
    let handle = tokio::spawn(worker.run());

    server_tx
        .send(command_frame(
            "reset-1",
            Some(command::Payload::ResetStats(ResetStatsCmd {
                target_datasource_ids: vec![41, 41],
            })),
        ))
        .await
        .unwrap();

    let ack = client_rx.recv().await.unwrap();
    assert_eq!(probe.reset_count.load(Ordering::SeqCst), 1);
    assert!(command_ack(&ack).success);
    assert_eq!(command_ack(&ack).command_id, "reset-1");

    drop(server_tx);
    let _ = handle.await.unwrap();
}

#[tokio::test]
async fn full_snapshot_frame_precedes_its_success_ack() {
    let probe = Arc::new(ResetProbe::new(52));
    let pair = TransportPair::new(8);
    let (_batch_tx, batch_rx) = mpsc::channel::<PushEvent>(4);
    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 4);
    let worker = worker.with_command_executor(executor_for(&probe));
    let handle = tokio::spawn(worker.run());

    server_tx
        .send(command_frame(
            "full-1",
            Some(command::Payload::RequestFullSnapshot(
                RequestFullSnapshotCmd {},
            )),
        ))
        .await
        .unwrap();

    let full_frame = client_rx.recv().await.unwrap();
    let full_batch = match full_frame.payload.as_ref() {
        Some(client_frame::Payload::SnapshotBatch(batch)) => batch,
        other => panic!("expected full SnapshotBatch before ACK, got {other:?}"),
    };
    assert!(full_batch.is_full_snapshot);
    assert_eq!(full_batch.entries.len(), 1);
    assert_eq!(full_batch.entries[0].datasource_id, 52);

    let ack = client_rx.recv().await.unwrap();
    assert!(command_ack(&ack).success);
    assert_eq!(command_ack(&ack).command_id, "full-1");

    drop(server_tx);
    let _ = handle.await.unwrap();
}

#[tokio::test]
async fn empty_unknown_and_failed_commands_return_failed_ack() {
    let probe = Arc::new(ResetProbe::busy(63));
    let pair = TransportPair::new(8);
    let (_batch_tx, batch_rx) = mpsc::channel::<PushEvent>(4);
    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 4);
    let worker = worker.with_command_executor(executor_for(&probe));
    let handle = tokio::spawn(worker.run());

    let cases = [
        command_frame("empty", None),
        command_frame(
            "empty-reset",
            Some(command::Payload::ResetStats(ResetStatsCmd {
                target_datasource_ids: Vec::new(),
            })),
        ),
        command_frame(
            "unknown-reset",
            Some(command::Payload::ResetStats(ResetStatsCmd {
                target_datasource_ids: vec![999],
            })),
        ),
        command_frame(
            "busy-full",
            Some(command::Payload::RequestFullSnapshot(
                RequestFullSnapshotCmd {},
            )),
        ),
    ];

    for frame in cases {
        server_tx.send(frame).await.unwrap();
        let ack = client_rx.recv().await.unwrap();
        assert!(!command_ack(&ack).success);
        assert!(!command_ack(&ack).error_message.is_empty());
    }
    assert_eq!(probe.reset_count.load(Ordering::SeqCst), 0);

    drop(server_tx);
    let _ = handle.await.unwrap();
}

#[tokio::test]
async fn reset_of_a_live_target_ignores_an_unrelated_expired_registration() {
    let probe = Arc::new(ResetProbe::new(74));
    let pair = TransportPair::new(8);
    let (_batch_tx, batch_rx) = mpsc::channel::<PushEvent>(4);
    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 4);
    let worker = worker.with_command_executor(executor_with_expired_unrelated_entry(&probe));
    let handle = tokio::spawn(worker.run());

    server_tx
        .send(command_frame(
            "reset-live",
            Some(command::Payload::ResetStats(ResetStatsCmd {
                target_datasource_ids: vec![74],
            })),
        ))
        .await
        .unwrap();

    let ack = client_rx.recv().await.unwrap();
    assert!(command_ack(&ack).success);
    assert_eq!(probe.reset_count.load(Ordering::SeqCst), 1);

    drop(server_tx);
    let _ = handle.await.unwrap();
}

#[tokio::test]
async fn server_resolves_a_pending_reset_after_success_ack() {
    let handler = IngestHandler::new();
    handler.handle_frame(&server_full_snapshot_frame(1)).await;
    let command = handler.issue_reset_command(vec![10, 20]);
    let command_id = match command.payload {
        Some(server_frame::Payload::Command(command)) => command.command_id,
        other => panic!("expected reset command, got {other:?}"),
    };
    assert!(handler.has_pending_command(&command_id));

    handler
        .handle_frame(&server_command_ack(command_id.clone(), true, ""))
        .await;

    assert!(!handler.has_pending_command(&command_id));
}

#[tokio::test]
async fn server_resolves_a_pending_reset_after_failed_ack() {
    let handler = IngestHandler::new();
    handler.handle_frame(&server_full_snapshot_frame(1)).await;
    let command = handler.issue_reset_command(vec![1]);
    let command_id = match command.payload {
        Some(server_frame::Payload::Command(command)) => command.command_id,
        other => panic!("expected reset command, got {other:?}"),
    };
    assert!(handler.has_pending_command(&command_id));

    handler
        .handle_frame(&server_command_ack(
            command_id.clone(),
            false,
            "datasource busy",
        ))
        .await;

    assert!(!handler.has_pending_command(&command_id));
}

#[tokio::test]
async fn full_snapshot_respects_batch_size_and_waits_for_ack_capacity_between_chunks() {
    let probes = vec![
        Arc::new(ResetProbe::new(81)),
        Arc::new(ResetProbe::new(82)),
        Arc::new(ResetProbe::new(83)),
    ];
    let pair = TransportPair::new(8);
    let (_batch_tx, batch_rx) = mpsc::channel::<PushEvent>(4);
    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 1);
    let worker = worker.with_command_executor(executor_for_probes(&probes, 2));
    let handle = tokio::spawn(worker.run());

    server_tx
        .send(command_frame(
            "full-chunked",
            Some(command::Payload::RequestFullSnapshot(
                RequestFullSnapshotCmd {},
            )),
        ))
        .await
        .unwrap();

    let first = client_rx.recv().await.unwrap();
    let first_batch = match first.payload.as_ref() {
        Some(client_frame::Payload::SnapshotBatch(batch)) => batch,
        other => panic!("expected first full batch, got {other:?}"),
    };
    assert!(first_batch.is_full_snapshot);
    assert_eq!(first_batch.entries.len(), 2);
    assert!(client_rx.try_recv().is_err(), "second chunk waits for ACK");

    let first_sequence = first_batch.header.as_ref().unwrap().sequence;
    server_tx
        .send(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(
                druid_metrics::protocol::BatchAck {
                    header: None,
                    accepted_sequence: first_sequence,
                },
            )),
        })
        .await
        .unwrap();

    let second = client_rx.recv().await.unwrap();
    let second_batch = match second.payload.as_ref() {
        Some(client_frame::Payload::SnapshotBatch(batch)) => batch,
        other => panic!("expected second full batch, got {other:?}"),
    };
    assert!(second_batch.is_full_snapshot);
    assert_eq!(second_batch.entries.len(), 1);

    let ack = client_rx.recv().await.unwrap();
    assert!(command_ack(&ack).success);
    assert_eq!(command_ack(&ack).command_id, "full-chunked");

    drop(server_tx);
    let _ = handle.await.unwrap();
}

#[test]
fn mixed_unknown_or_expired_reset_targets_have_no_partial_side_effects() {
    let live = Arc::new(ResetProbe::new(91));
    let live_executor = executor_for(&live);
    assert!(live_executor.reset_stats(&[91, 999]).is_err());
    assert_eq!(live.reset_count.load(Ordering::SeqCst), 0);

    let expired_executor = executor_with_expired_unrelated_entry(&live);
    assert!(expired_executor.reset_stats(&[91, 999]).is_err());
    assert_eq!(live.reset_count.load(Ordering::SeqCst), 0);
}

async fn wait_until_outbound_is_full(client_rx: &mpsc::Receiver<ClientFrame>) {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while client_rx.len() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("initial frame fills the capacity-1 outbound channel");
}

async fn join_after_cancel(
    handle: &mut tokio::task::JoinHandle<Vec<druid_metrics::sequence_window::PendingFrame>>,
) -> Vec<druid_metrics::sequence_window::PendingFrame> {
    if let Ok(result) = tokio::time::timeout(std::time::Duration::from_secs(1), &mut *handle).await
    {
        result.expect("worker task does not panic")
    } else {
        handle.abort();
        let _ = handle.await;
        panic!("cancellation must terminate a transport-saturated worker");
    }
}

#[tokio::test]
async fn saturated_reset_ack_is_cancellation_aware_and_does_not_repeat_reset() {
    let probe = Arc::new(ResetProbe::new(92));
    let pair = TransportPair::new(1);
    let (batch_tx, batch_rx) = mpsc::channel(2);
    let cancellation = CancellationToken::new();
    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 4);
    let worker = worker
        .with_command_executor(executor_for(&probe))
        .with_cancellation_token(cancellation.clone());
    let mut handle = tokio::spawn(worker.run());

    batch_tx
        .send(PushEvent::Batch(outbound_batch(920)))
        .await
        .unwrap();
    wait_until_outbound_is_full(&client_rx).await;
    let reset_observed = probe.reset_observed.notified();
    server_tx
        .send(command_frame(
            "saturated-reset",
            Some(command::Payload::ResetStats(ResetStatsCmd {
                target_datasource_ids: vec![92],
            })),
        ))
        .await
        .unwrap();
    reset_observed.await;

    cancellation.cancel();
    let pending = join_after_cancel(&mut handle).await;
    assert_eq!(
        pending.len(),
        1,
        "original unsent-ACK frame remains replayable"
    );
    assert_eq!(probe.reset_count.load(Ordering::SeqCst), 1);
    let _initial = client_rx.recv().await.unwrap();
    assert!(
        client_rx.recv().await.is_none(),
        "success ACK was never falsely delivered"
    );
}

#[tokio::test]
async fn saturated_full_snapshot_send_is_cancellation_aware_and_preserves_both_frames() {
    let probe = Arc::new(ResetProbe::new(93));
    let pair = TransportPair::new(1);
    let (batch_tx, batch_rx) = mpsc::channel(2);
    let cancellation = CancellationToken::new();
    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 4);
    let worker = worker
        .with_command_executor(executor_for(&probe))
        .with_cancellation_token(cancellation.clone());
    let mut handle = tokio::spawn(worker.run());

    batch_tx
        .send(PushEvent::Batch(outbound_batch(930)))
        .await
        .unwrap();
    wait_until_outbound_is_full(&client_rx).await;
    let snapshot_observed = probe.snapshot_observed.notified();
    server_tx
        .send(command_frame(
            "saturated-full",
            Some(command::Payload::RequestFullSnapshot(
                RequestFullSnapshotCmd {},
            )),
        ))
        .await
        .unwrap();
    snapshot_observed.await;

    cancellation.cancel();
    let pending = join_after_cancel(&mut handle).await;
    assert_eq!(
        pending.len(),
        2,
        "initial and full snapshot frames remain replayable"
    );
    assert_eq!(probe.snapshot_count.load(Ordering::SeqCst), 1);
    let _initial = client_rx.recv().await.unwrap();
    assert!(
        client_rx.recv().await.is_none(),
        "blocked full snapshot was not delivered"
    );
}
