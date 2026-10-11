//! Tests for the server-side `IngestHandler`.
//!
//! The `IngestHandler` receives `ClientFrame` messages from a stream, deduplicates
//! by stream identity + sequence, and updates an in-memory snapshot repository.

use druid_metrics::ingest_handler::IngestHandler;
use druid_metrics::protocol::*;
use std::time::Duration;

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_header(seq: u64, service: &str, instance: &str, boot: &str, epoch: u64) -> FrameHeader {
    FrameHeader {
        protocol_version: 1,
        service_name: service.into(),
        instance_id: instance.into(),
        boot_id: boot.into(),
        stream_epoch: epoch,
        sequence: seq,
        emitted_at_unix_ms: 1_700_000_000_000,
    }
}

fn make_batch_frame(
    seq: u64,
    service: &str,
    instance: &str,
    boot: &str,
    epoch: u64,
) -> ClientFrame {
    ClientFrame {
        payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
            header: Some(make_header(seq, service, instance, boot, epoch)),
            entries: vec![SnapshotEntry {
                datasource_id: 1,
                datasource_name: "test-db".into(),
                driver_name: Some("postgres".into()),
                pool: Some(PoolSnapshotMsg {
                    active_count: 5,
                    idle_count: 3,
                    max_active: 20,
                    max_idle: 10,
                    waiting_count: 0,
                }),
                sql_stats: vec![],
                wall_check_count: 0,
                wall_deny_count: 0,
                wall_violation_count: 0,
                sampling_time_millis: 1_700_000_000_000,
            }],
            is_full_snapshot: false,
            snapshot_transaction: None,
        })),
    }
}

fn make_full_snapshot_frame(
    seq: u64,
    service: &str,
    instance: &str,
    boot: &str,
    epoch: u64,
) -> ClientFrame {
    ClientFrame {
        payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
            header: Some(make_header(seq, service, instance, boot, epoch)),
            entries: vec![SnapshotEntry {
                datasource_id: 42,
                datasource_name: "full-snap-db".into(),
                driver_name: None,
                pool: Some(PoolSnapshotMsg {
                    active_count: 1,
                    idle_count: 0,
                    max_active: 5,
                    max_idle: 2,
                    waiting_count: 0,
                }),
                sql_stats: vec![],
                wall_check_count: 0,
                wall_deny_count: 0,
                wall_violation_count: 0,
                sampling_time_millis: 1_700_000_000_000,
            }],
            is_full_snapshot: true,
            snapshot_transaction: Some(SnapshotTransaction {
                transaction_id: format!("test-transaction-{seq}"),
                chunk_index: 0,
                chunk_count: 1,
                end: true,
            }),
        })),
    }
}

/// Helper: initialize a stream by sending a full snapshot and asserting `BatchAck`.
async fn init_stream(handler: &IngestHandler, svc: &str, inst: &str, boot: &str, epoch: u64) {
    let r = handler
        .handle_frame(&make_full_snapshot_frame(1, svc, inst, boot, epoch))
        .await;
    match r {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(ack)),
        }) => assert_eq!(ack.accepted_sequence, 1),
        other => panic!("expected BatchAck for full snapshot init, got {other:?}"),
    }
}

// ─── tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn goodbye_releases_stream_capacity_for_next_session() {
    let handler = IngestHandler::new().with_limits(1, 1);
    init_stream(&handler, "svc", "instance-a", "boot-a", 1).await;
    let goodbye = ClientFrame {
        payload: Some(client_frame::Payload::Goodbye(ClientGoodbye {
            header: Some(make_header(0, "svc", "instance-a", "boot-a", 1)),
            reason: "client_shutdown".to_owned(),
        })),
    };
    assert!(handler.handle_frame(&goodbye).await.is_none());

    init_stream(&handler, "svc", "instance-b", "boot-b", 1).await;
}

#[tokio::test]
async fn successor_epoch_reuses_the_same_service_capacity_slot() {
    let handler = IngestHandler::new().with_limits(1, 1);
    init_stream(&handler, "svc", "instance-a", "boot-a", 41).await;

    let hello = ClientFrame {
        payload: Some(client_frame::Payload::Hello(ClientHello {
            header: Some(make_header(0, "svc", "instance-a", "boot-a", 42)),
            supported_protocol_version: 1,
            full_snapshot_requested: false,
        })),
    };
    let response = handler.handle_frame(&hello).await;
    assert!(matches!(
        response.and_then(|frame| frame.payload),
        Some(server_frame::Payload::HelloAck(HelloAck {
            require_full_snapshot: true,
            ..
        }))
    ));
    init_stream(&handler, "svc", "instance-a", "boot-a", 42).await;

    let retired = handler
        .handle_frame(&make_batch_frame(2, "svc", "instance-a", "boot-a", 41))
        .await;
    assert!(matches!(
        retired.and_then(|frame| frame.payload),
        Some(server_frame::Payload::Error(_))
    ));

    let unrelated = handler
        .handle_frame(&make_full_snapshot_frame(
            1,
            "svc",
            "instance-b",
            "boot-b",
            1,
        ))
        .await;
    assert!(matches!(
        unrelated.and_then(|frame| frame.payload),
        Some(server_frame::Payload::Error(_))
    ));
}

#[tokio::test]
async fn expired_disconnected_stream_releases_capacity_for_new_session() {
    let handler = IngestHandler::new()
        .with_limits(1, 1)
        .with_stream_ttl(Duration::from_millis(10));
    init_stream(&handler, "svc", "lost-instance", "old-boot", 1).await;
    tokio::time::sleep(Duration::from_millis(25)).await;

    init_stream(&handler, "svc", "replacement", "new-boot", 1).await;
}

#[tokio::test(start_paused = true)]
async fn heartbeat_extends_stream_ttl_without_allocating_new_state() {
    let handler = IngestHandler::new()
        .with_limits(1, 1)
        .with_stream_ttl(Duration::from_secs(10));
    init_stream(&handler, "svc", "active", "boot", 1).await;
    tokio::time::advance(Duration::from_secs(9)).await;
    let heartbeat = ClientFrame {
        payload: Some(client_frame::Payload::Heartbeat(ClientHeartbeat {
            header: Some(make_header(0, "svc", "active", "boot", 1)),
            pending_ack_count: 0,
        })),
    };
    assert!(handler.handle_frame(&heartbeat).await.is_none());
    tokio::time::advance(Duration::from_secs(9)).await;

    let rejected = handler
        .handle_frame(&make_full_snapshot_frame(1, "svc", "new", "boot", 1))
        .await;
    assert!(matches!(
        rejected,
        Some(ServerFrame {
            payload: Some(server_frame::Payload::Error(_)),
        })
    ));
    tokio::time::advance(Duration::from_secs(2)).await;
    init_stream(&handler, "svc", "new", "boot", 1).await;
}

#[tokio::test]
async fn duplicate_batch_only_ingested_once() {
    let handler = IngestHandler::new();

    // Initialize stream with full snapshot first.
    init_stream(&handler, "svc", "inst1", "boot1", 100).await;

    // Now send seq 2 twice.
    let frame = make_batch_frame(2, "svc", "inst1", "boot1", 100);
    let resp1 = handler.handle_frame(&frame).await;
    let resp2 = handler.handle_frame(&frame).await;

    // Both should return BatchAck (idempotent).
    match resp1 {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(ack)),
        }) => assert_eq!(ack.accepted_sequence, 2),
        other => panic!("expected BatchAck, got {other:?}"),
    }
    match resp2 {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(ack)),
        }) => assert_eq!(ack.accepted_sequence, 2),
        other => panic!("expected BatchAck for duplicate, got {other:?}"),
    }

    // init_stream ingested 1 batch + first send ingested 1 = 2 total.
    // The duplicate should NOT increment the counter.
    assert_eq!(handler.ingest_count(), 2);
}

#[tokio::test]
async fn sequence_gap_returns_resync_required() {
    let handler = IngestHandler::new();

    // Initialize stream.
    init_stream(&handler, "svc", "i1", "b1", 100).await;

    // Send seq 2 (ok, next expected = 2).
    let r1 = handler
        .handle_frame(&make_batch_frame(2, "svc", "i1", "b1", 100))
        .await;
    assert!(matches!(
        r1,
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(_)),
        })
    ));

    // Send seq 4 (gap -- expected seq 3).
    let r2 = handler
        .handle_frame(&make_batch_frame(4, "svc", "i1", "b1", 100))
        .await;
    match r2 {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::ResyncRequired(resync)),
        }) => {
            assert_eq!(resync.expected_sequence, 3);
        }
        other => panic!("expected ResyncRequired, got {other:?}"),
    }
}

#[tokio::test]
async fn new_stream_without_full_snapshot_requires_it() {
    let handler = IngestHandler::new();

    // First contact with a normal (non-full) batch on a new stream.
    let r = handler
        .handle_frame(&make_batch_frame(1, "svc", "i1", "b1", 100))
        .await;
    match r {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::HelloAck(ack)),
        }) => {
            assert!(
                ack.require_full_snapshot,
                "new stream should require full snapshot"
            );
        }
        other => panic!("expected HelloAck with require_full_snapshot=true, got {other:?}"),
    }
}

#[tokio::test]
async fn full_snapshot_accepted_then_normal_batches() {
    let handler = IngestHandler::new();

    // First batch on a new stream triggers HelloAck requiring full snapshot.
    let r1 = handler
        .handle_frame(&make_batch_frame(1, "svc", "i1", "b1", 100))
        .await;
    assert!(matches!(
        r1,
        Some(ServerFrame {
            payload: Some(server_frame::Payload::HelloAck(_)),
        })
    ));

    // Client sends full snapshot (seq=1, is_full_snapshot=true).
    let r2 = handler
        .handle_frame(&make_full_snapshot_frame(1, "svc", "i1", "b1", 100))
        .await;
    match r2 {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(ack)),
        }) => assert_eq!(ack.accepted_sequence, 1),
        other => panic!("expected BatchAck for full snapshot, got {other:?}"),
    }

    // Now normal batches should be accepted.
    let r3 = handler
        .handle_frame(&make_batch_frame(2, "svc", "i1", "b1", 100))
        .await;
    match r3 {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(ack)),
        }) => assert_eq!(ack.accepted_sequence, 2),
        other => panic!("expected BatchAck for normal batch after full snapshot, got {other:?}"),
    }
}

#[tokio::test]
async fn different_streams_are_independent() {
    let handler = IngestHandler::new();

    // Initialize both streams with full snapshots.
    init_stream(&handler, "svc", "i1", "b1", 100).await;
    init_stream(&handler, "svc", "i2", "b2", 200).await;

    // Send seq 2 on stream A.
    let r1 = handler
        .handle_frame(&make_batch_frame(2, "svc", "i1", "b1", 100))
        .await;
    match r1 {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(ack)),
        }) => assert_eq!(ack.accepted_sequence, 2),
        other => panic!("expected BatchAck for stream A seq 2, got {other:?}"),
    }

    // Send seq 2 on stream B (independent sequence space).
    let r2 = handler
        .handle_frame(&make_batch_frame(2, "svc", "i2", "b2", 200))
        .await;
    match r2 {
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(ack)),
        }) => assert_eq!(ack.accepted_sequence, 2),
        other => panic!("expected BatchAck for stream B seq 2, got {other:?}"),
    }
}

#[tokio::test]
async fn snapshot_repository_stores_latest() {
    let handler = IngestHandler::new();

    // Ingest a full snapshot.
    init_stream(&handler, "svc", "i1", "b1", 100).await;

    // Check the in-memory repository.
    let snapshots = handler.latest_snapshots("svc", "i1", "b1");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].datasource_name, "full-snap-db");
}
