//! Behavioral tests for payload-preserving push and ACK-window backpressure.

use std::time::Duration;

use druid_metrics::aggregator::AggregatedBatch;
use druid_metrics::protocol::{
    client_frame, BatchAck, ClientFrame, FrameHeader, PoolSnapshotMsg, ServerFrame, SnapshotBatch,
    SnapshotEntry,
};
use druid_metrics::push_worker::{PushEvent, TransportPair};
use druid_metrics::sequence_window::PendingFrame;
use prost::Message;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn batch(datasource_id: u64) -> AggregatedBatch {
    AggregatedBatch {
        entries: vec![SnapshotEntry {
            datasource_id,
            datasource_name: format!("datasource-{datasource_id}"),
            driver_name: Some("test-driver".to_owned()),
            pool: Some(PoolSnapshotMsg {
                active_count: 1,
                idle_count: 2,
                max_active: 8,
                max_idle: 4,
                waiting_count: 0,
            }),
            sql_stats: Vec::new(),
            wall_check_count: 0,
            wall_deny_count: 0,
            wall_violation_count: 0,
            sampling_time_millis: 42,
        }],
        is_full_snapshot: false,
    }
}

fn make_ack(sequence: u64) -> ServerFrame {
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
                accepted_sequence: sequence,
            },
        )),
    }
}

fn snapshot_batch(frame: &ClientFrame) -> &druid_metrics::protocol::SnapshotBatch {
    match frame.payload.as_ref() {
        Some(client_frame::Payload::SnapshotBatch(batch)) => batch,
        other => panic!("expected SnapshotBatch, got {other:?}"),
    }
}

#[tokio::test]
async fn disconnect_and_reconnect_resend_the_complete_frame_byte_identically() {
    let pair = TransportPair::new(8);
    let (batch_tx, batch_rx) = mpsc::channel(4);
    let (mut first_rx, first_server_tx, worker) = pair.into_worker(batch_rx, 4);
    let first_handle = tokio::spawn(worker.run());

    batch_tx.send(PushEvent::Batch(batch(17))).await.unwrap();
    let first = first_rx
        .recv()
        .await
        .expect("first connection receives frame");
    assert!(!snapshot_batch(&first).entries.is_empty());
    assert_eq!(snapshot_batch(&first).header.as_ref().unwrap().sequence, 1);

    drop(first_server_tx);
    let pending = first_handle.await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].encoded_frame, first.encode_to_vec());

    let reconnect_pair = TransportPair::new(8);
    let (resend_tx, resend_rx) = mpsc::channel(4);
    resend_tx
        .send(PushEvent::Resend(pending[0].clone()))
        .await
        .unwrap();
    let (mut reconnect_rx, reconnect_server_tx, reconnect_worker) =
        reconnect_pair.into_worker(resend_rx, 4);
    let reconnect_handle = tokio::spawn(reconnect_worker.run());

    let resent = reconnect_rx.recv().await.expect("reconnect resends frame");
    assert_eq!(first.encode_to_vec(), resent.encode_to_vec());
    reconnect_server_tx.send(make_ack(1)).await.unwrap();

    resend_tx.send(PushEvent::Batch(batch(18))).await.unwrap();
    let next = reconnect_rx
        .recv()
        .await
        .expect("stream continues after retry");
    assert_eq!(snapshot_batch(&next).header.as_ref().unwrap().sequence, 2);

    drop(resend_tx);
    drop(reconnect_server_tx);
    let _ = reconnect_handle.await.unwrap();
    drop(batch_tx);
}

#[tokio::test]
async fn full_window_pauses_batch_admission_until_ack() {
    let pair = TransportPair::new(8);
    let (batch_tx, batch_rx) = mpsc::channel(4);
    let (mut client_rx, server_tx, worker) = pair.into_worker(batch_rx, 1);
    let handle = tokio::spawn(worker.run());

    batch_tx.send(PushEvent::Batch(batch(1))).await.unwrap();
    batch_tx.send(PushEvent::Batch(batch(2))).await.unwrap();
    let first = client_rx.recv().await.unwrap();
    assert_eq!(snapshot_batch(&first).header.as_ref().unwrap().sequence, 1);

    assert!(
        tokio::time::timeout(Duration::from_millis(50), client_rx.recv())
            .await
            .is_err(),
        "second batch must remain queued while the ACK window is full"
    );
    assert!(!handle.is_finished(), "window saturation is not terminal");

    server_tx.send(make_ack(1)).await.unwrap();
    let second = tokio::time::timeout(Duration::from_secs(1), client_rx.recv())
        .await
        .expect("ACK releases admission")
        .unwrap();
    assert_eq!(snapshot_batch(&second).header.as_ref().unwrap().sequence, 2);

    drop(batch_tx);
    drop(server_tx);
    let _ = handle.await.unwrap();
}

#[tokio::test]
async fn cancellation_stops_a_worker_while_the_window_is_saturated() {
    let pair = TransportPair::new(8);
    let (batch_tx, batch_rx) = mpsc::channel(4);
    let cancellation = CancellationToken::new();
    let (_client_rx, _server_tx, worker) = pair.into_worker(batch_rx, 1);
    let worker = worker.with_cancellation_token(cancellation.clone());
    let handle = tokio::spawn(worker.run());

    batch_tx.send(PushEvent::Batch(batch(1))).await.unwrap();
    batch_tx.send(PushEvent::Batch(batch(2))).await.unwrap();
    tokio::task::yield_now().await;
    cancellation.cancel();

    let pending = tokio::time::timeout(Duration::from_secs(1), handle)
        .await
        .expect("cancellation terminates a saturated worker")
        .unwrap();
    assert_eq!(pending.len(), 1);
}

#[tokio::test]
async fn resend_rejects_a_pending_sequence_that_disagrees_with_the_encoded_header() {
    let encoded_frame = ClientFrame {
        payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
            header: Some(FrameHeader {
                protocol_version: 1,
                service_name: "druid-pool".into(),
                instance_id: "local".into(),
                boot_id: String::new(),
                stream_epoch: 0,
                sequence: 1,
                emitted_at_unix_ms: 1,
            }),
            entries: batch(9).entries,
            is_full_snapshot: false,
        })),
    }
    .encode_to_vec();
    let pair = TransportPair::new(8);
    let (batch_tx, batch_rx) = mpsc::channel(1);
    let (mut client_rx, _server_tx, worker) = pair.into_worker(batch_rx, 4);
    let handle = tokio::spawn(worker.run());

    batch_tx
        .send(PushEvent::Resend(PendingFrame {
            sequence: 7,
            encoded_frame,
        }))
        .await
        .unwrap();

    assert!(
        client_rx.recv().await.is_none(),
        "invalid resend is not sent"
    );
    assert!(
        handle.await.unwrap().is_empty(),
        "invalid resend is not stored"
    );
}

#[tokio::test]
async fn resend_rejects_malformed_and_non_snapshot_frames_without_storing_them() {
    let non_snapshot = ClientFrame {
        payload: Some(client_frame::Payload::CommandAck(
            druid_metrics::protocol::CommandAck {
                header: None,
                command_id: "not-a-snapshot".to_owned(),
                success: true,
                error_message: String::new(),
            },
        )),
    }
    .encode_to_vec();

    for encoded_frame in [vec![0xff], non_snapshot] {
        let pair = TransportPair::new(1);
        let (batch_tx, batch_rx) = mpsc::channel(1);
        let (mut client_rx, _server_tx, worker) = pair.into_worker(batch_rx, 2);
        let handle = tokio::spawn(worker.run());
        batch_tx
            .send(PushEvent::Resend(PendingFrame {
                sequence: 1,
                encoded_frame,
            }))
            .await
            .unwrap();

        assert!(
            client_rx.recv().await.is_none(),
            "invalid frame is not sent"
        );
        assert!(
            handle.await.unwrap().is_empty(),
            "invalid frame is not stored"
        );
    }
}

#[tokio::test]
async fn saturated_outbound_processes_the_oldest_ack_before_cancellation() {
    let pair = TransportPair::new(1);
    let (batch_tx, batch_rx) = mpsc::channel(4);
    let cancellation = CancellationToken::new();
    let (client_rx, server_tx, worker) = pair.into_worker(batch_rx, 4);
    let worker = worker.with_cancellation_token(cancellation.clone());
    let handle = tokio::spawn(worker.run());

    batch_tx.send(PushEvent::Batch(batch(101))).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while client_rx.len() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first frame fills the outbound transport");

    batch_tx.send(PushEvent::Batch(batch(102))).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while batch_tx.capacity() != 4 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker consumes and stores the second frame before blocking on send");

    server_tx.send(make_ack(1)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while server_tx.capacity() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker processes ACK while the outbound send is blocked");

    cancellation.cancel();
    let pending = tokio::time::timeout(Duration::from_secs(1), handle)
        .await
        .expect("cancellation stops the saturated worker")
        .unwrap();
    assert_eq!(
        pending
            .into_iter()
            .map(|frame| frame.sequence)
            .collect::<Vec<_>>(),
        vec![2]
    );
}
