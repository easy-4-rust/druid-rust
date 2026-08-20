#![allow(clippy::match_same_arms)]
//! Server-side ingest handler.
//!
//! Receives [`ClientFrame`] messages from a gRPC stream, deduplicates by
//! stream identity + sequence number, and maintains an in-memory snapshot
//! repository.
//!
//! # Stream identity
//!
//! A stream is uniquely identified by `(service_name, instance_id, boot_id,
//! stream_epoch)`. Each stream tracks its own expected next sequence number.
//!
//! # Deduplication
//!
//! If a batch with an already-seen sequence arrives, it is treated as a
//! duplicate: the handler returns a [`BatchAck`] without re-ingesting the data.
//!
//! # Full snapshot requirement
//!
//! When a new stream identity is first seen, the handler returns a
//! [`HelloAck`] with `require_full_snapshot = true`. The client must send a
//! full snapshot batch (`is_full_snapshot = true`) before normal incremental
//! batches are accepted.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::protocol::{
    client_frame, server_frame, BatchAck, ClientFrame, Command, CommandAck, FrameHeader, HelloAck,
    ResetStatsCmd, ResyncRequired, ServerFrame, SnapshotEntry,
};

/// Stream identity key.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct StreamKey {
    service_name: String,
    instance_id: String,
    boot_id: String,
    stream_epoch: u64,
}

/// Per-stream tracking state.
#[derive(Debug)]
struct StreamState {
    /// Next expected sequence number.
    expected_seq: u64,
    /// Whether the stream has completed its initial full snapshot.
    initialized: bool,
    /// Latest snapshot entries, keyed by `datasource_id`.
    snapshots: HashMap<u64, SnapshotEntry>,
}

impl StreamState {
    fn new() -> Self {
        Self {
            expected_seq: 1,
            initialized: false,
            snapshots: HashMap::new(),
        }
    }
}

/// Server-side ingest handler.
///
/// Thread-safe: all state is behind a [`Mutex`].
pub struct IngestHandler {
    state: Mutex<IngestState>,
    ingest_count: AtomicU64,
    command_counter: AtomicU64,
}

struct IngestState {
    streams: HashMap<StreamKey, StreamState>,
    /// Set of command IDs that have been issued but not yet acknowledged.
    pending_commands: HashSet<String>,
}

impl IngestHandler {
    /// Create a new empty ingest handler.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(IngestState {
                streams: HashMap::new(),
                pending_commands: HashSet::new(),
            }),
            ingest_count: AtomicU64::new(0),
            command_counter: AtomicU64::new(0),
        }
    }

    /// Handle a single client frame, returning an optional server response.
    ///
    /// Returns `None` for frames that don't require a response (e.g.
    /// Heartbeat).
    pub async fn handle_frame(&self, frame: &ClientFrame) -> Option<ServerFrame> {
        match &frame.payload {
            Some(client_frame::Payload::SnapshotBatch(batch)) => {
                self.handle_snapshot_batch(batch).await
            }
            Some(client_frame::Payload::Hello(hello)) => {
                let hdr = hello.header.as_ref()?;
                let key = self.stream_key(hdr);
                let mut state = self.state.lock().expect("ingest lock poisoned");
                state.streams.entry(key).or_insert_with(StreamState::new);
                Some(self.make_hello_ack(hdr, true))
            }
            Some(client_frame::Payload::Heartbeat(_)) => None,
            Some(client_frame::Payload::CommandAck(ack)) => {
                self.handle_command_ack(ack);
                None
            }
            Some(client_frame::Payload::Goodbye(_)) => None,
            None => None,
        }
    }

    /// Handle a snapshot batch: dedup, sequence check, ingest or resync.
    #[allow(clippy::unused_async)]
    async fn handle_snapshot_batch(
        &self,
        batch: &crate::protocol::SnapshotBatch,
    ) -> Option<ServerFrame> {
        let hdr = batch.header.as_ref()?;
        let key = self.stream_key(hdr);
        let seq = hdr.sequence;

        let mut state = self.state.lock().expect("ingest lock poisoned");
        let stream = state
            .streams
            .entry(key.clone())
            .or_insert_with(StreamState::new);

        // ── New stream: require full snapshot ──
        if !stream.initialized {
            if batch.is_full_snapshot {
                // Accept the full snapshot.
                stream.initialized = true;
                stream.expected_seq = seq + 1;
                self.store_snapshots(stream, &batch.entries);
                self.ingest_count.fetch_add(1, Ordering::Relaxed);
                return Some(self.make_batch_ack(hdr, seq));
            }
            // Not a full snapshot -- require one.
            return Some(self.make_hello_ack(hdr, true));
        }

        // ── Duplicate: already ingested this sequence ──
        if seq < stream.expected_seq {
            return Some(self.make_batch_ack(hdr, seq));
        }

        // ── Gap: missing sequences ──
        if seq > stream.expected_seq {
            return Some(self.make_resync_required(hdr, stream.expected_seq));
        }

        // ── Expected sequence: ingest normally ──
        stream.expected_seq = seq + 1;
        self.store_snapshots(stream, &batch.entries);
        self.ingest_count.fetch_add(1, Ordering::Relaxed);
        Some(self.make_batch_ack(hdr, seq))
    }

    /// Store snapshot entries into the stream's in-memory repository.
    fn store_snapshots(&self, stream: &mut StreamState, entries: &[SnapshotEntry]) {
        for entry in entries {
            stream.snapshots.insert(entry.datasource_id, entry.clone());
        }
    }

    fn stream_key(&self, hdr: &FrameHeader) -> StreamKey {
        StreamKey {
            service_name: hdr.service_name.clone(),
            instance_id: hdr.instance_id.clone(),
            boot_id: hdr.boot_id.clone(),
            stream_epoch: hdr.stream_epoch,
        }
    }

    fn make_batch_ack(&self, hdr: &FrameHeader, seq: u64) -> ServerFrame {
        ServerFrame {
            payload: Some(server_frame::Payload::BatchAck(BatchAck {
                header: Some(self.reply_header(hdr)),
                accepted_sequence: seq,
            })),
        }
    }

    fn make_hello_ack(&self, hdr: &FrameHeader, require_full: bool) -> ServerFrame {
        ServerFrame {
            payload: Some(server_frame::Payload::HelloAck(HelloAck {
                header: Some(self.reply_header(hdr)),
                server_protocol_version: 1,
                require_full_snapshot: require_full,
            })),
        }
    }

    fn make_resync_required(&self, hdr: &FrameHeader, expected: u64) -> ServerFrame {
        ServerFrame {
            payload: Some(server_frame::Payload::ResyncRequired(ResyncRequired {
                header: Some(self.reply_header(hdr)),
                reason: "sequence_gap".into(),
                expected_sequence: expected,
            })),
        }
    }

    fn reply_header(&self, hdr: &FrameHeader) -> FrameHeader {
        FrameHeader {
            protocol_version: 1,
            service_name: hdr.service_name.clone(),
            instance_id: hdr.instance_id.clone(),
            boot_id: hdr.boot_id.clone(),
            stream_epoch: hdr.stream_epoch,
            sequence: 0, // server doesn't use client-style sequences
            emitted_at_unix_ms: now_unix_ms(),
        }
    }

    /// Process a `CommandAck` from the client: remove the command from pending.
    fn handle_command_ack(&self, ack: &CommandAck) {
        let mut state = self.state.lock().expect("ingest lock poisoned");
        state.pending_commands.remove(&ack.command_id);
        if ack.success {
            tracing::debug!(command_id = %ack.command_id, "command acknowledged");
        } else {
            tracing::warn!(
                command_id = %ack.command_id,
                error = %ack.error_message,
                "command failed on client"
            );
        }
    }

    /// Issue a `ResetStats` command targeting the given datasource IDs.
    ///
    /// Returns a [`ServerFrame`] containing the command to send to the client.
    /// The command is tracked as pending until a matching [`CommandAck`] arrives.
    pub fn issue_reset_command(&self, datasource_ids: Vec<u64>) -> ServerFrame {
        let cmd_id = self.next_command_id();
        {
            let mut state = self.state.lock().expect("ingest lock poisoned");
            state.pending_commands.insert(cmd_id.clone());
        }
        ServerFrame {
            payload: Some(server_frame::Payload::Command(Command {
                header: Some(FrameHeader {
                    protocol_version: 1,
                    service_name: String::new(),
                    instance_id: String::new(),
                    boot_id: String::new(),
                    stream_epoch: 0,
                    sequence: 0,
                    emitted_at_unix_ms: now_unix_ms(),
                }),
                command_id: cmd_id,
                payload: Some(crate::protocol::command::Payload::ResetStats(
                    ResetStatsCmd {
                        target_datasource_ids: datasource_ids,
                    },
                )),
            })),
        }
    }

    /// Check whether a command with the given ID is still pending.
    pub fn has_pending_command(&self, command_id: &str) -> bool {
        let state = self.state.lock().expect("ingest lock poisoned");
        state.pending_commands.contains(command_id)
    }

    fn next_command_id(&self) -> String {
        let n = self.command_counter.fetch_add(1, Ordering::Relaxed);
        format!("cmd-{n}")
    }

    /// Total number of batches successfully ingested (duplicates that
    /// returned `BatchAck` without re-ingestion are NOT counted).
    pub fn ingest_count(&self) -> u64 {
        self.ingest_count.load(Ordering::Relaxed)
    }

    /// Retrieve the latest snapshots for a given stream identity.
    pub fn latest_snapshots(
        &self,
        service: &str,
        instance: &str,
        boot: &str,
    ) -> Vec<SnapshotEntry> {
        let state = self.state.lock().expect("ingest lock poisoned");
        state
            .streams
            .iter()
            .filter(|(k, _)| {
                k.service_name == service && k.instance_id == instance && k.boot_id == boot
            })
            .flat_map(|(_, v)| v.snapshots.values().cloned())
            .collect()
    }
}

impl Default for IngestHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
