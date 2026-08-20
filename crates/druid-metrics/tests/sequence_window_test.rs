//! Tests for the client-side sequence/ACK window.
//!
//! The SequenceWindow tracks batches that have been sent but not yet
//! acknowledged by the server. On reconnect only the pending (un-ACKed)
//! batches are retransmitted in original sequence order.

use druid_metrics::sequence_window::{PendingBatch, SequenceWindow};

// ---------- helpers ----------

fn make_batch(seq: u64) -> PendingBatch {
    PendingBatch {
        sequence: seq,
        payload_bytes: format!("batch-{seq}").into_bytes(),
    }
}

// ---------- construction ----------

#[test]
fn new_window_is_empty() {
    let w = SequenceWindow::new(256);
    assert_eq!(w.len(), 0);
    assert!(w.is_empty());
    assert_eq!(w.capacity(), 256);
}

// ---------- push & ack ----------

#[test]
fn push_assigns_sequential_numbers() {
    let mut w = SequenceWindow::new(256);

    let seq1 = w.push(b"a".to_vec()).unwrap();
    let seq2 = w.push(b"b".to_vec()).unwrap();
    let seq3 = w.push(b"c".to_vec()).unwrap();

    assert_eq!(seq1, 1);
    assert_eq!(seq2, 2);
    assert_eq!(seq3, 3);
    assert_eq!(w.len(), 3);
}

#[test]
fn ack_removes_entry() {
    let mut w = SequenceWindow::new(256);
    w.push(b"a".to_vec());
    w.push(b"b".to_vec());
    w.push(b"c".to_vec());

    w.ack(2).expect("ack(2) should succeed");
    assert_eq!(w.len(), 2);

    // seq 1 and 3 should still be pending
    let pending = w.pending_batches();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].sequence, 1);
    assert_eq!(pending[1].sequence, 3);
}

#[test]
fn ack_unknown_sequence_returns_error() {
    let mut w = SequenceWindow::new(256);
    w.push(b"a".to_vec());

    let result = w.ack(999);
    assert!(
        result.is_err(),
        "ack of unknown sequence should return error"
    );
}

#[test]
fn duplicate_ack_returns_error() {
    let mut w = SequenceWindow::new(256);
    w.push(b"a".to_vec());

    w.ack(1).expect("first ack");
    let result = w.ack(1);
    assert!(result.is_err(), "duplicate ack should return error");
}

// ---------- capacity ----------

#[test]
fn push_beyond_capacity_returns_error() {
    let mut w = SequenceWindow::new(2);
    w.push(b"a".to_vec());
    w.push(b"b".to_vec());

    let result = w.push(b"c".to_vec());
    assert!(result.is_err(), "push beyond capacity should return error");
}

#[test]
fn ack_frees_capacity_for_new_push() {
    let mut w = SequenceWindow::new(2);
    w.push(b"a".to_vec());
    w.push(b"b".to_vec());

    // Window full.
    assert!(w.push(b"c".to_vec()).is_err());

    // Free one slot.
    w.ack(1).expect("ack(1)");
    assert_eq!(w.len(), 1);

    // Now push should succeed.
    let seq = w.push(b"c".to_vec()).unwrap();
    assert_eq!(seq, 3);
    assert_eq!(w.len(), 2);
}

// ---------- pending_batches / resend ----------

#[test]
fn pending_batches_returns_unacked_in_order() {
    let mut w = SequenceWindow::new(256);
    w.push(b"a".to_vec()); // seq 1
    w.push(b"b".to_vec()); // seq 2
    w.push(b"c".to_vec()); // seq 3

    w.ack(2).expect("remove middle");

    let pending = w.pending_batches();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].sequence, 1);
    assert_eq!(pending[1].sequence, 3);
}

#[test]
fn resend_after_partial_ack() {
    // Scenario: send 3 batches, ACK seq 1, reconnect should resend seq 2 and 3.
    let mut w = SequenceWindow::new(256);
    w.push(b"batch-1".to_vec()); // seq 1
    w.push(b"batch-2".to_vec()); // seq 2
    w.push(b"batch-3".to_vec()); // seq 3

    // Server ACKs seq 1.
    w.ack(1).expect("ack seq 1");

    // Simulate reconnect: collect pending batches for retransmit.
    let to_resend = w.pending_batches();
    assert_eq!(to_resend.len(), 2);
    assert_eq!(to_resend[0].sequence, 2);
    assert_eq!(to_resend[1].sequence, 3);
    assert_eq!(to_resend[0].payload_bytes, b"batch-2");
    assert_eq!(to_resend[1].payload_bytes, b"batch-3");
}

#[test]
fn ack_all_leaves_window_empty() {
    let mut w = SequenceWindow::new(256);
    w.push(b"a".to_vec());
    w.push(b"b".to_vec());

    w.ack(1).unwrap();
    w.ack(2).unwrap();

    assert!(w.is_empty());
    assert!(w.pending_batches().is_empty());
}

// ---------- boundary: first sequence is 1 ----------

#[test]
fn first_push_returns_sequence_one() {
    let mut w = SequenceWindow::new(256);
    let seq = w.push(b"first".to_vec()).unwrap();
    assert_eq!(seq, 1, "first sequence number should be 1");
}
