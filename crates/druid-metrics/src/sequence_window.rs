//! Client-side sequence/ACK window for reliable delivery.
//!
//! Tracks batches that have been sent to the server but not yet acknowledged.
//! On reconnect only the pending (un-ACKed) batches are retransmitted in
//! their original sequence order.
//!
//! # Invariants
//!
//! - Sequence numbers start at **1** and are monotonically increasing.
//! - The window holds at most `capacity` pending batches.
//! - `ACKing` an unknown or already-ACKed sequence is an error.

use std::collections::BTreeMap;

use thiserror::Error;

/// Errors produced by [`SequenceWindow`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SequenceWindowError {
    /// The window is at capacity; the caller must wait for an ACK before
    /// pushing more batches.
    #[error("window full: capacity {capacity}, pending {pending}")]
    WindowFull {
        /// Maximum number of pending batches.
        capacity: usize,
        /// Current number of pending batches.
        pending: usize,
    },

    /// The acknowledged sequence number is not present in the window.
    /// This covers both unknown sequences and duplicate ACKs.
    #[error("unknown sequence: {0}")]
    UnknownSequence(u64),
}

/// A batch that has been sent but not yet acknowledged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingBatch {
    /// Monotonically increasing sequence number assigned at push time.
    pub sequence: u64,
    /// Serialized payload bytes ready for retransmission.
    pub payload_bytes: Vec<u8>,
}

/// Bounded sliding window tracking un-ACKed batches.
///
/// # Usage
///
/// ```rust
/// use druid_metrics::sequence_window::SequenceWindow;
///
/// let mut w = SequenceWindow::new(256);
/// let seq = w.push(b"hello".to_vec()).unwrap();
/// assert_eq!(seq, 1);
///
/// w.ack(seq).unwrap();
/// assert!(w.is_empty());
/// ```
#[derive(Debug)]
pub struct SequenceWindow {
    /// Pending batches keyed by sequence number.
    pending: BTreeMap<u64, PendingBatch>,
    /// Maximum number of un-ACKed batches.
    capacity: usize,
    /// Next sequence number to assign.
    next_seq: u64,
}

impl SequenceWindow {
    /// Create a new empty window with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            pending: BTreeMap::new(),
            capacity,
            next_seq: 1,
        }
    }

    /// Push a batch into the window, assigning it the next sequence number.
    ///
    /// Returns the assigned sequence number, or an error if the window is full.
    pub fn push(&mut self, payload_bytes: Vec<u8>) -> Result<u64, SequenceWindowError> {
        if self.pending.len() >= self.capacity {
            return Err(SequenceWindowError::WindowFull {
                capacity: self.capacity,
                pending: self.pending.len(),
            });
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.pending.insert(
            seq,
            PendingBatch {
                sequence: seq,
                payload_bytes,
            },
        );
        Ok(seq)
    }

    /// Acknowledge a batch, removing it from the window.
    ///
    /// Returns an error if the sequence is unknown (never pushed or already
    /// acknowledged).
    pub fn ack(&mut self, seq: u64) -> Result<(), SequenceWindowError> {
        self.pending
            .remove(&seq)
            .map(|_| ())
            .ok_or(SequenceWindowError::UnknownSequence(seq))
    }

    /// Return all pending (un-ACKed) batches in sequence order.
    ///
    /// This is used on reconnect to determine which batches need
    /// retransmission.
    pub fn pending_batches(&self) -> Vec<PendingBatch> {
        self.pending.values().cloned().collect()
    }

    /// Number of pending batches in the window.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Returns `true` if the window has no pending batches.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Maximum number of pending batches the window can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_ack_basic() {
        let mut w = SequenceWindow::new(10);
        let s1 = w.push(b"a".to_vec()).unwrap();
        let s2 = w.push(b"b".to_vec()).unwrap();
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        assert_eq!(w.len(), 2);

        w.ack(1).unwrap();
        assert_eq!(w.len(), 1);

        let pending = w.pending_batches();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].sequence, 2);
    }

    #[test]
    fn ack_unknown_returns_error() {
        let mut w = SequenceWindow::new(10);
        w.push(b"a".to_vec()).unwrap();
        assert!(w.ack(999).is_err());
    }

    #[test]
    fn duplicate_ack_returns_error() {
        let mut w = SequenceWindow::new(10);
        w.push(b"a".to_vec()).unwrap();
        w.ack(1).unwrap();
        assert!(w.ack(1).is_err());
    }

    #[test]
    fn full_window_rejects_push() {
        let mut w = SequenceWindow::new(1);
        w.push(b"a".to_vec()).unwrap();
        assert!(w.push(b"b".to_vec()).is_err());
    }
}
