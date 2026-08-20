//! Tests for the client-side `PushWorker`.
//!
//! The `PushWorker` consumes batches from an mpsc channel, assigns sequence
//! numbers via a `SequenceWindow`, and pushes them to the server through a
//! transport abstraction. It processes ACKs to free window slots.

use std::time::Duration;

use druid_metrics::protocol::{BatchAck, FrameHeader, ServerFrame};
use druid_metrics::push_worker::{PushEvent, TransportPair};
use tokio::sync::mpsc;

fn make_ack(seq: u64) -> ServerFrame {
    ServerFrame {
        payload: Some(druid_metrics::protocol::server_frame::Payload::BatchAck(
            BatchAck {
                header: Some(FrameHeader {
                    protocol_version: 1,
                    service_name: "test".into(),
                    instance_id: "test".into(),
                    boot_id: "test".into(),
                    stream_epoch: 0,
                    sequence: 0,
                    emitted_at_unix_ms: 0,
                }),
                accepted_sequence: seq,
            },
        )),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Test 1: Normal push -- worker sends batch, receives ACK, window drains.
// ──────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn normal_push_and_ack() {
    let pair = TransportPair::new(64);
    let (batch_tx, batch_rx) = mpsc::channel::<PushEvent>(16);

    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 256);
    let handle = tokio::spawn(async move { worker.run().await });

    // Enqueue one batch.
    batch_tx
        .send(PushEvent::Batch {
            payload_bytes: b"hello".to_vec(),
        })
        .await
        .unwrap();

    // The worker should send a ClientFrame containing a SnapshotBatch.
    let frame = tokio::time::timeout(Duration::from_secs(2), client_rx.recv())
        .await
        .expect("worker should send a frame within 2s")
        .expect("channel open");
    let seq = match frame.payload {
        Some(druid_metrics::protocol::client_frame::Payload::SnapshotBatch(batch)) => {
            batch.header.as_ref().unwrap().sequence
        }
        other => panic!("expected SnapshotBatch, got {other:?}"),
    };
    assert_eq!(seq, 1);

    // Simulate server ACK.
    server_tx.send(make_ack(seq)).await.unwrap();

    // Give the worker time to process the ACK.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Shut down cleanly.
    drop(batch_tx);
    drop(server_tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

// ──────────────────────────────────────────────────────────────────────────────
// Test 2: Worker waits when no batches are available.
// ──────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn worker_waits_when_queue_empty() {
    let pair = TransportPair::new(64);
    let (batch_tx, batch_rx) = mpsc::channel::<PushEvent>(16);

    let (mut client_rx, _server_tx, worker) = pair.into_worker(batch_rx, 256);
    let handle = tokio::spawn(async move { worker.run().await });

    // No batches sent -- worker should be idle.
    // Verify no frame is sent within 200ms.
    let result = tokio::time::timeout(Duration::from_millis(200), client_rx.recv()).await;
    assert!(
        result.is_err(),
        "worker should not send while queue is empty"
    );

    // Now send a batch.
    batch_tx
        .send(PushEvent::Batch {
            payload_bytes: b"delayed".to_vec(),
        })
        .await
        .unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(2), client_rx.recv())
        .await
        .expect("worker should send after batch arrives")
        .expect("channel open");
    assert!(frame.payload.is_some());

    // Clean up.
    drop(batch_tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

// ──────────────────────────────────────────────────────────────────────────────
// Test 3: Pending (un-ACKed) batches survive a simulated reconnect.
// ──────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pending_batches_available_after_simulated_disconnect() {
    // This test verifies the SequenceWindow tracks un-ACKed batches
    // correctly. We simulate the scenario: push 3 batches, ACK 1,
    // then verify 2 remain pending (as would be resent on reconnect).
    use druid_metrics::sequence_window::SequenceWindow;

    let mut window = SequenceWindow::new(256);
    let s1 = window.push(b"batch-1".to_vec()).unwrap();
    let s2 = window.push(b"batch-2".to_vec()).unwrap();
    let s3 = window.push(b"batch-3".to_vec()).unwrap();

    // Server ACKs seq 1.
    window.ack(s1).unwrap();

    // On reconnect, pending batches should be seq 2 and 3.
    let pending = window.pending_batches();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].sequence, s2);
    assert_eq!(pending[1].sequence, s3);
    assert_eq!(pending[0].payload_bytes, b"batch-2");
    assert_eq!(pending[1].payload_bytes, b"batch-3");

    // Verify the worker can actually resend these.
    let pair = TransportPair::new(64);
    let (batch_tx, batch_rx) = mpsc::channel::<PushEvent>(16);

    // Enqueue the pending batches as reconnect events.
    for p in &pending {
        batch_tx
            .send(PushEvent::Resend {
                sequence: p.sequence,
                payload_bytes: p.payload_bytes.clone(),
            })
            .await
            .unwrap();
    }

    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 256);
    let handle = tokio::spawn(async move { worker.run().await });

    // Expect two frames with the original sequences.
    for expected_seq in [s2, s3] {
        let frame = tokio::time::timeout(Duration::from_secs(2), client_rx.recv())
            .await
            .expect("worker should send resend frame")
            .expect("channel open");
        match frame.payload {
            Some(druid_metrics::protocol::client_frame::Payload::SnapshotBatch(batch)) => {
                let hdr = batch.header.as_ref().unwrap();
                assert_eq!(hdr.sequence, expected_seq);
            }
            other => panic!("expected SnapshotBatch, got {other:?}"),
        }
    }

    // ACK both.
    server_tx.send(make_ack(s2)).await.unwrap();
    server_tx.send(make_ack(s3)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    drop(batch_tx);
    drop(server_tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}
