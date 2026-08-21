//! Tests for the immutable client-side sequence/ACK frame window.

use druid_metrics::sequence_window::{PendingFrame, SequenceWindow};

#[test]
fn push_encodes_once_and_retains_exact_frame_bytes() {
    let mut window = SequenceWindow::new(2);

    let inserted = window
        .push_with(|sequence| format!("complete-frame-{sequence}").into_bytes())
        .expect("first frame fits");

    assert_eq!(inserted.sequence, 1);
    assert_eq!(inserted.encoded_frame, b"complete-frame-1");
    assert_eq!(window.pending_frames(), vec![inserted]);
}

#[test]
fn ack_removes_only_the_matching_frame_and_frees_capacity() {
    let mut window = SequenceWindow::new(2);
    window.push_with(|_| b"frame-1".to_vec()).unwrap();
    window.push_with(|_| b"frame-2".to_vec()).unwrap();

    assert!(window.is_full());
    assert!(window.push_with(|_| b"must-not-run".to_vec()).is_err());

    window.ack(1).expect("known ACK succeeds");
    let third = window.push_with(|_| b"frame-3".to_vec()).unwrap();

    assert_eq!(third.sequence, 3);
    assert_eq!(
        window
            .pending_frames()
            .into_iter()
            .map(|frame| frame.sequence)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
}

#[test]
fn restore_preserves_bytes_and_advances_the_stream_sequence() {
    let mut window = SequenceWindow::new(3);
    let restored = PendingFrame {
        sequence: 7,
        encoded_frame: b"wire-identical-frame".to_vec(),
    };

    window.restore(restored.clone()).expect("restore fits");
    let next = window
        .push_with(|sequence| format!("frame-{sequence}").into_bytes())
        .unwrap();

    assert_eq!(window.pending_frames()[0], restored);
    assert_eq!(next.sequence, 8);
}

#[test]
fn restore_rejects_zero_duplicate_and_full_sequences() {
    let mut window = SequenceWindow::new(1);
    assert!(window
        .restore(PendingFrame {
            sequence: 0,
            encoded_frame: vec![0],
        })
        .is_err());

    let frame = PendingFrame {
        sequence: 3,
        encoded_frame: vec![3],
    };
    window.restore(frame.clone()).unwrap();
    assert!(window.restore(frame).is_err());
    assert!(window
        .restore(PendingFrame {
            sequence: 4,
            encoded_frame: vec![4],
        })
        .is_err());
}

#[test]
fn unknown_and_duplicate_ack_are_errors() {
    let mut window = SequenceWindow::new(1);
    window.push_with(|_| b"frame".to_vec()).unwrap();

    assert!(window.ack(99).is_err());
    window.ack(1).unwrap();
    assert!(window.ack(1).is_err());
    assert!(window.is_empty());
}
