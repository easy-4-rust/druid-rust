//! Round-trip tests for the gRPC protocol message types.
//!
//! Every ClientFrame and ServerFrame variant must survive Prost encode/decode
//! with all fields intact.

use druid_metrics::protocol::*;

// ---------- helpers ----------

fn sample_header() -> FrameHeader {
    FrameHeader {
        protocol_version: 1,
        service_name: "druid-pool".to_owned(),
        instance_id: "inst-abc".to_owned(),
        boot_id: "boot-001".to_owned(),
        stream_epoch: 1_700_000_000_000,
        sequence: 42,
        emitted_at_unix_ms: 1_700_000_001_000,
    }
}

fn sample_pool_snapshot() -> PoolSnapshotMsg {
    PoolSnapshotMsg {
        active_count: 3,
        idle_count: 7,
        max_active: 20,
        max_idle: 10,
        waiting_count: 1,
    }
}

fn sample_sql_stat() -> SqlStatMsg {
    SqlStatMsg {
        fingerprint: "SELECT * FROM t WHERE id = ?".to_owned(),
        exec_count: 100,
        exec_time_millis: 500,
        fetch_row_count: 99,
        update_count: 0,
    }
}

fn sample_snapshot_entry() -> SnapshotEntry {
    SnapshotEntry {
        datasource_id: 1,
        datasource_name: "orders-db".to_owned(),
        driver_name: Some("postgres".to_owned()),
        pool: Some(sample_pool_snapshot()),
        sql_stats: vec![sample_sql_stat()],
        wall_check_count: 1000,
        wall_deny_count: 2,
        wall_violation_count: 1,
        sampling_time_millis: 1_700_000_000_500,
    }
}

// ---------- ClientFrame round-trip ----------

#[test]
fn client_hello_roundtrip() {
    let msg = ClientHello {
        header: Some(sample_header()),
        supported_protocol_version: 1,
        full_snapshot_requested: false,
    };
    let frame = ClientFrame {
        payload: Some(client_frame::Payload::Hello(msg.clone())),
    };

    let bytes = encode_client_frame(&frame);
    let decoded = decode_client_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(client_frame::Payload::Hello(h)) => {
            assert_eq!(h.supported_protocol_version, 1);
            assert!(!h.full_snapshot_requested);
            let hdr = h.header.unwrap();
            assert_eq!(hdr.protocol_version, 1);
            assert_eq!(hdr.service_name, "druid-pool");
            assert_eq!(hdr.instance_id, "inst-abc");
            assert_eq!(hdr.sequence, 42);
        }
        other => panic!("expected Hello, got {other:?}"),
    }
}

#[test]
fn client_snapshot_batch_roundtrip() {
    let msg = SnapshotBatch {
        header: Some(sample_header()),
        entries: vec![sample_snapshot_entry(), sample_snapshot_entry()],
        is_full_snapshot: true,
    };
    let frame = ClientFrame {
        payload: Some(client_frame::Payload::SnapshotBatch(msg)),
    };

    let bytes = encode_client_frame(&frame);
    let decoded = decode_client_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(client_frame::Payload::SnapshotBatch(batch)) => {
            assert_eq!(batch.entries.len(), 2);
            assert!(batch.is_full_snapshot);
            assert_eq!(batch.entries[0].datasource_name, "orders-db");
            assert_eq!(batch.entries[0].sql_stats.len(), 1);
            assert_eq!(
                batch.entries[0].sql_stats[0].fingerprint,
                "SELECT * FROM t WHERE id = ?"
            );
            assert_eq!(batch.entries[0].pool.as_ref().unwrap().active_count, 3);
        }
        other => panic!("expected SnapshotBatch, got {other:?}"),
    }
}

#[test]
fn client_heartbeat_roundtrip() {
    let msg = ClientHeartbeat {
        header: Some(sample_header()),
        pending_ack_count: 5,
    };
    let frame = ClientFrame {
        payload: Some(client_frame::Payload::Heartbeat(msg)),
    };

    let bytes = encode_client_frame(&frame);
    let decoded = decode_client_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(client_frame::Payload::Heartbeat(hb)) => {
            assert_eq!(hb.pending_ack_count, 5);
            assert_eq!(hb.header.unwrap().sequence, 42);
        }
        other => panic!("expected Heartbeat, got {other:?}"),
    }
}

#[test]
fn client_command_ack_roundtrip() {
    let msg = CommandAck {
        header: Some(sample_header()),
        command_id: "cmd-reset-001".to_owned(),
        success: true,
        error_message: String::new(),
    };
    let frame = ClientFrame {
        payload: Some(client_frame::Payload::CommandAck(msg)),
    };

    let bytes = encode_client_frame(&frame);
    let decoded = decode_client_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(client_frame::Payload::CommandAck(ack)) => {
            assert_eq!(ack.command_id, "cmd-reset-001");
            assert!(ack.success);
            assert!(ack.error_message.is_empty());
        }
        other => panic!("expected CommandAck, got {other:?}"),
    }
}

#[test]
fn client_command_ack_with_error_roundtrip() {
    let msg = CommandAck {
        header: Some(sample_header()),
        command_id: "cmd-xyz".to_owned(),
        success: false,
        error_message: "reset failed: datasource busy".to_owned(),
    };
    let frame = ClientFrame {
        payload: Some(client_frame::Payload::CommandAck(msg)),
    };

    let bytes = encode_client_frame(&frame);
    let decoded = decode_client_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(client_frame::Payload::CommandAck(ack)) => {
            assert!(!ack.success);
            assert_eq!(ack.error_message, "reset failed: datasource busy");
        }
        other => panic!("expected CommandAck, got {other:?}"),
    }
}

#[test]
fn client_goodbye_roundtrip() {
    let msg = ClientGoodbye {
        header: Some(sample_header()),
        reason: "shutdown".to_owned(),
    };
    let frame = ClientFrame {
        payload: Some(client_frame::Payload::Goodbye(msg)),
    };

    let bytes = encode_client_frame(&frame);
    let decoded = decode_client_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(client_frame::Payload::Goodbye(bye)) => {
            assert_eq!(bye.reason, "shutdown");
        }
        other => panic!("expected Goodbye, got {other:?}"),
    }
}

// ---------- ServerFrame round-trip ----------

#[test]
fn server_hello_ack_roundtrip() {
    let msg = HelloAck {
        header: Some(sample_header()),
        server_protocol_version: 1,
        require_full_snapshot: false,
    };
    let frame = ServerFrame {
        payload: Some(server_frame::Payload::HelloAck(msg)),
    };

    let bytes = encode_server_frame(&frame);
    let decoded = decode_server_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(server_frame::Payload::HelloAck(ack)) => {
            assert_eq!(ack.server_protocol_version, 1);
            assert!(!ack.require_full_snapshot);
        }
        other => panic!("expected HelloAck, got {other:?}"),
    }
}

#[test]
fn server_batch_ack_roundtrip() {
    let msg = BatchAck {
        header: Some(sample_header()),
        accepted_sequence: 42,
    };
    let frame = ServerFrame {
        payload: Some(server_frame::Payload::BatchAck(msg)),
    };

    let bytes = encode_server_frame(&frame);
    let decoded = decode_server_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(server_frame::Payload::BatchAck(ack)) => {
            assert_eq!(ack.accepted_sequence, 42);
        }
        other => panic!("expected BatchAck, got {other:?}"),
    }
}

#[test]
fn server_resync_required_roundtrip() {
    let msg = ResyncRequired {
        header: Some(sample_header()),
        reason: "sequence_gap".to_owned(),
        expected_sequence: 43,
    };
    let frame = ServerFrame {
        payload: Some(server_frame::Payload::ResyncRequired(msg)),
    };

    let bytes = encode_server_frame(&frame);
    let decoded = decode_server_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(server_frame::Payload::ResyncRequired(resync)) => {
            assert_eq!(resync.reason, "sequence_gap");
            assert_eq!(resync.expected_sequence, 43);
        }
        other => panic!("expected ResyncRequired, got {other:?}"),
    }
}

#[test]
fn server_command_reset_stats_roundtrip() {
    let msg = Command {
        header: Some(sample_header()),
        command_id: "cmd-rst-001".to_owned(),
        payload: Some(command::Payload::ResetStats(ResetStatsCmd {
            target_datasource_ids: vec![1, 2, 3],
        })),
    };
    let frame = ServerFrame {
        payload: Some(server_frame::Payload::Command(msg)),
    };

    let bytes = encode_server_frame(&frame);
    let decoded = decode_server_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(server_frame::Payload::Command(cmd)) => {
            assert_eq!(cmd.command_id, "cmd-rst-001");
            match cmd.payload {
                Some(command::Payload::ResetStats(reset)) => {
                    assert_eq!(reset.target_datasource_ids, vec![1, 2, 3]);
                }
                other => panic!("expected ResetStats, got {other:?}"),
            }
        }
        other => panic!("expected Command, got {other:?}"),
    }
}

#[test]
fn server_command_request_full_snapshot_roundtrip() {
    let msg = Command {
        header: Some(sample_header()),
        command_id: "cmd-fs-002".to_owned(),
        payload: Some(command::Payload::RequestFullSnapshot(
            RequestFullSnapshotCmd {},
        )),
    };
    let frame = ServerFrame {
        payload: Some(server_frame::Payload::Command(msg)),
    };

    let bytes = encode_server_frame(&frame);
    let decoded = decode_server_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(server_frame::Payload::Command(cmd)) => {
            assert_eq!(cmd.command_id, "cmd-fs-002");
            match cmd.payload {
                Some(command::Payload::RequestFullSnapshot(_)) => { /* ok */ }
                other => panic!("expected RequestFullSnapshot, got {other:?}"),
            }
        }
        other => panic!("expected Command, got {other:?}"),
    }
}

#[test]
fn server_error_roundtrip() {
    let msg = ServerError {
        header: Some(sample_header()),
        code: 400,
        message: "invalid sequence".to_owned(),
        close_stream: true,
    };
    let frame = ServerFrame {
        payload: Some(server_frame::Payload::Error(msg)),
    };

    let bytes = encode_server_frame(&frame);
    let decoded = decode_server_frame(&bytes).expect("decode failed");

    match decoded.payload {
        Some(server_frame::Payload::Error(err)) => {
            assert_eq!(err.code, 400);
            assert_eq!(err.message, "invalid sequence");
            assert!(err.close_stream);
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

// ---------- empty/default frame ----------

#[test]
fn empty_client_frame_roundtrip() {
    let frame = ClientFrame { payload: None };
    let bytes = encode_client_frame(&frame);
    let decoded = decode_client_frame(&bytes).expect("decode failed");
    assert!(decoded.payload.is_none());
}

#[test]
fn empty_server_frame_roundtrip() {
    let frame = ServerFrame { payload: None };
    let bytes = encode_server_frame(&frame);
    let decoded = decode_server_frame(&bytes).expect("decode failed");
    assert!(decoded.payload.is_none());
}
