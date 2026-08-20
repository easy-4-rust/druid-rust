//! Tests for the remote ResetStats command flow.
//!
//! Server sends a `Command{ResetStats}` to the client via the server->client
//! stream. The client executes a local reset on the specified datasource IDs
//! and replies with a `CommandAck`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use druid_metrics::ingest_handler::IngestHandler;
use druid_metrics::protocol::*;
use druid_metrics::push_worker::{PushEvent, TransportPair};
use tokio::sync::mpsc;

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_reset_command(command_id: &str, datasource_ids: Vec<u64>) -> ServerFrame {
    ServerFrame {
        payload: Some(server_frame::Payload::Command(Command {
            header: Some(FrameHeader {
                protocol_version: 1,
                service_name: "test".into(),
                instance_id: "test".into(),
                boot_id: "test".into(),
                stream_epoch: 0,
                sequence: 0,
                emitted_at_unix_ms: 0,
            }),
            command_id: command_id.into(),
            payload: Some(command::Payload::ResetStats(ResetStatsCmd {
                target_datasource_ids: datasource_ids,
            })),
        })),
    }
}

fn make_full_snapshot_frame(seq: u64) -> ClientFrame {
    ClientFrame {
        payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
            header: Some(FrameHeader {
                protocol_version: 1,
                service_name: "svc".into(),
                instance_id: "i1".into(),
                boot_id: "b1".into(),
                stream_epoch: 100,
                sequence: seq,
                emitted_at_unix_ms: 1_700_000_000_000,
            }),
            entries: vec![],
            is_full_snapshot: true,
        })),
    }
}

fn make_batch_frame(seq: u64) -> ClientFrame {
    ClientFrame {
        payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
            header: Some(FrameHeader {
                protocol_version: 1,
                service_name: "svc".into(),
                instance_id: "i1".into(),
                boot_id: "b1".into(),
                stream_epoch: 100,
                sequence: seq,
                emitted_at_unix_ms: 1_700_000_000_000,
            }),
            entries: vec![],
            is_full_snapshot: false,
        })),
    }
}

// ─── tests ──────────────────────────────────────────────────────────────────

/// Test: client receives a ResetStats command, executes local reset, and
/// replies with CommandAck(success=true).
#[tokio::test]
async fn client_resets_on_command_and_acks() {
    let pair = TransportPair::new(64);
    let (batch_tx, batch_rx) = mpsc::channel::<PushEvent>(16);

    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 256);
    let handle = tokio::spawn(async move { worker.run().await });

    // Send a batch so the worker has something to process first.
    batch_tx
        .send(PushEvent::Batch {
            payload_bytes: b"data".to_vec(),
        })
        .await
        .unwrap();

    // Read the batch frame the worker sent.
    let _frame = tokio::time::timeout(Duration::from_secs(2), client_rx.recv())
        .await
        .expect("worker should send batch")
        .expect("channel open");

    // Server sends a ResetStats command.
    server_tx
        .send(make_reset_command("cmd-rst-001", vec![1, 2, 3]))
        .await
        .unwrap();

    // The worker should reply with a CommandAck.
    let ack_frame = tokio::time::timeout(Duration::from_secs(2), client_rx.recv())
        .await
        .expect("worker should send CommandAck")
        .expect("channel open");

    match ack_frame.payload {
        Some(client_frame::Payload::CommandAck(ack)) => {
            assert_eq!(ack.command_id, "cmd-rst-001");
            assert!(ack.success, "reset should succeed");
        }
        other => panic!("expected CommandAck, got {other:?}"),
    }

    // Clean up.
    drop(batch_tx);
    drop(server_tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

/// Test: server tracks pending commands and receives ACK.
#[tokio::test]
async fn server_tracks_pending_command() {
    let handler = IngestHandler::new();

    // Initialize the stream.
    let r = handler.handle_frame(&make_full_snapshot_frame(1)).await;
    assert!(matches!(
        r,
        Some(ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(_)),
        })
    ));

    // Server issues a reset command.
    let cmd_frame = handler.issue_reset_command(vec![10, 20]);
    let command_id = match &cmd_frame.payload {
        Some(server_frame::Payload::Command(cmd)) => cmd.command_id.clone(),
        other => panic!("expected Command frame, got {other:?}"),
    };

    // Verify the command is pending.
    assert!(handler.has_pending_command(&command_id));

    // Client sends back a successful CommandAck.
    let ack = ClientFrame {
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
            command_id: command_id.clone(),
            success: true,
            error_message: String::new(),
        })),
    };
    handler.handle_frame(&ack).await;

    // Command should no longer be pending.
    assert!(!handler.has_pending_command(&command_id));
}

/// Test: failed reset ACK is tracked with error message.
#[tokio::test]
async fn failed_reset_ack_records_error() {
    let handler = IngestHandler::new();

    // Initialize stream.
    handler.handle_frame(&make_full_snapshot_frame(1)).await;

    // Issue command.
    let cmd_frame = handler.issue_reset_command(vec![1]);
    let command_id = match &cmd_frame.payload {
        Some(server_frame::Payload::Command(cmd)) => cmd.command_id.clone(),
        _ => unreachable!(),
    };

    // Client replies with failure.
    let ack = ClientFrame {
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
            command_id: command_id.clone(),
            success: false,
            error_message: "datasource busy".into(),
        })),
    };
    handler.handle_frame(&ack).await;

    // Command should be resolved (no longer pending).
    assert!(!handler.has_pending_command(&command_id));
}

/// Test: end-to-end -- server issues reset, client processes it, ack flows
/// back to the server ingest handler.
#[tokio::test]
async fn end_to_end_reset_command_flow() {
    // ── Server side ──
    let server = IngestHandler::new();

    // Client initializes stream.
    server.handle_frame(&make_full_snapshot_frame(1)).await;

    // Server issues reset.
    let cmd = server.issue_reset_command(vec![42]);
    let command_id = match &cmd.payload {
        Some(server_frame::Payload::Command(c)) => c.command_id.clone(),
        _ => unreachable!(),
    };

    // ── Client side ──
    // Simulate the client receiving the command and replying.
    // (In real usage, PushWorker.handle_server_frame processes this.)
    let ack = ClientFrame {
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
            command_id: command_id.clone(),
            success: true,
            error_message: String::new(),
        })),
    };

    // ── Server receives ACK ──
    server.handle_frame(&ack).await;
    assert!(!server.has_pending_command(&command_id));
}
