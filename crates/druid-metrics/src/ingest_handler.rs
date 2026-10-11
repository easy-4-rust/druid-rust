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
use std::time::Duration;
use tokio::time::Instant;

use crate::protocol::{
    client_frame, server_frame, BatchAck, ClientFrame, Command, CommandAck, FrameHeader, HelloAck,
    ResetStatsCmd, ResyncRequired, ServerError, ServerFrame, SnapshotEntry, SnapshotTransaction,
};
use crate::snapshot_transaction_state::SnapshotTransactionState;
use crate::ServiceIdentity;

/// Stream identity key.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct StreamKey {
    service_identity: ServiceIdentity,
    stream_epoch: u64,
}

/// Per-stream tracking state.
#[derive(Debug)]
struct StreamState {
    last_seen: Instant,
    /// Next expected sequence number.
    expected_seq: u64,
    /// Whether the stream has completed its initial full snapshot.
    initialized: bool,
    /// Latest snapshot entries, keyed by `datasource_id`.
    snapshots: HashMap<u64, SnapshotEntry>,
    /// 尚未完整到达、不能对 repository 可见的权威快照事务。
    snapshot_transaction: Option<SnapshotTransactionState>,
}

impl StreamState {
    fn new() -> Self {
        Self {
            last_seen: Instant::now(),
            expected_seq: 1,
            initialized: false,
            snapshots: HashMap::new(),
            snapshot_transaction: None,
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
    max_streams: usize,
    max_snapshot_entries: usize,
    stream_ttl: Duration,
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
            max_streams: 1_024,
            max_snapshot_entries: 256,
            stream_ttl: Duration::from_secs(15 * 60),
        }
    }

    /// 设置服务端 stream state 与单个 authoritative transaction 的硬上限。
    #[must_use]
    pub fn with_limits(mut self, max_streams: usize, max_snapshot_entries: usize) -> Self {
        self.max_streams = max_streams.max(1);
        self.max_snapshot_entries = max_snapshot_entries.max(1);
        self
    }

    /// 设置无活动帧的协议 stream 保留期限；零值按一毫秒处理。
    #[must_use]
    pub fn with_stream_ttl(mut self, ttl: Duration) -> Self {
        self.stream_ttl = ttl.max(Duration::from_millis(1));
        self
    }

    /// Handle a single client frame, returning an optional server response.
    ///
    /// Returns `None` for frames that don't require a response (e.g.
    /// Heartbeat).
    pub async fn handle_frame(&self, frame: &ClientFrame) -> Option<ServerFrame> {
        self.handle_frame_with_optional_identity(frame, None, |_, _| true)
            .await
    }

    /// 使用调用方已校验并缓存的类型化服务身份处理一帧，避免重复构造 canonical ID。
    pub async fn handle_frame_with_identity(
        &self,
        frame: &ClientFrame,
        service_identity: &ServiceIdentity,
    ) -> Option<ServerFrame> {
        self.handle_frame_with_optional_identity(frame, Some(service_identity), |_, _| true)
            .await
    }

    /// 在外部 snapshot 存储提交成功后，原子推进协议 sequence 并产生 ACK。
    ///
    /// `commit` 只会对协议上可接受的新批次调用一次；返回 `false` 时保持 stream 状态不变，
    /// 返回相同 expected sequence 的 `ResyncRequired`，允许调用方修正并重试。
    pub async fn handle_frame_with_snapshot_commit(
        &self,
        frame: &ClientFrame,
        commit: impl FnOnce(&[SnapshotEntry], bool) -> bool,
    ) -> Option<ServerFrame> {
        self.handle_frame_with_optional_identity(frame, None, commit)
            .await
    }

    /// 使用已校验的类型化服务身份提交 snapshot，避免按 frame 重复编码身份。
    pub async fn handle_frame_with_identity_and_snapshot_commit(
        &self,
        frame: &ClientFrame,
        service_identity: &ServiceIdentity,
        commit: impl FnOnce(&[SnapshotEntry], bool) -> bool,
    ) -> Option<ServerFrame> {
        self.handle_frame_with_optional_identity(frame, Some(service_identity), commit)
            .await
    }

    async fn handle_frame_with_optional_identity(
        &self,
        frame: &ClientFrame,
        service_identity: Option<&ServiceIdentity>,
        commit: impl FnOnce(&[SnapshotEntry], bool) -> bool,
    ) -> Option<ServerFrame> {
        match &frame.payload {
            Some(client_frame::Payload::SnapshotBatch(batch)) => {
                self.handle_snapshot_batch(batch, service_identity, commit)
                    .await
            }
            Some(client_frame::Payload::Hello(hello)) => {
                let hdr = hello.header.as_ref()?;
                let key = match self.stream_key(hdr, service_identity) {
                    Ok(key) => key,
                    Err(message) => {
                        return Some(self.make_snapshot_transaction_error(hdr, message))
                    }
                };
                let mut state = self.state.lock().expect("ingest lock poisoned");
                self.prune_expired_streams(&mut state);
                if !state.streams.contains_key(&key) {
                    // 新 epoch 接管同一服务身份的容量槽；旧流由上层 broker 取消并围栏。
                    state
                        .streams
                        .retain(|existing, _| existing.service_identity != key.service_identity);
                }
                if !state.streams.contains_key(&key) && state.streams.len() >= self.max_streams {
                    return Some(self.make_snapshot_transaction_error(
                        hdr,
                        "ingest stream capacity is exhausted",
                    ));
                }
                let stream = state.streams.entry(key).or_insert_with(StreamState::new);
                stream.last_seen = Instant::now();
                Some(self.make_hello_ack(hdr, !stream.initialized))
            }
            Some(client_frame::Payload::Heartbeat(heartbeat)) => {
                let hdr = heartbeat.header.as_ref()?;
                let key = self.stream_key(hdr, service_identity).ok()?;
                let mut state = self.state.lock().expect("ingest lock poisoned");
                self.prune_expired_streams(&mut state);
                if let Some(stream) = state.streams.get_mut(&key) {
                    stream.last_seen = Instant::now();
                }
                None
            }
            Some(client_frame::Payload::CommandAck(ack)) => {
                self.handle_command_ack(ack);
                None
            }
            Some(client_frame::Payload::Goodbye(goodbye)) => {
                let hdr = goodbye.header.as_ref()?;
                let key = match self.stream_key(hdr, service_identity) {
                    Ok(key) => key,
                    Err(message) => {
                        return Some(self.make_snapshot_transaction_error(hdr, message))
                    }
                };
                // Goodbye 结束该协议流的 sequence/transaction 所有权，释放容量；
                // 已提交的 Admin 最新值仍由 repository TTL 独立治理。
                self.state
                    .lock()
                    .expect("ingest lock poisoned")
                    .streams
                    .remove(&key);
                None
            }
            None => None,
        }
    }

    /// Handle a snapshot batch: dedup, sequence check, ingest or resync.
    #[allow(clippy::unused_async)]
    async fn handle_snapshot_batch(
        &self,
        batch: &crate::protocol::SnapshotBatch,
        service_identity: Option<&ServiceIdentity>,
        commit: impl FnOnce(&[SnapshotEntry], bool) -> bool,
    ) -> Option<ServerFrame> {
        let hdr = batch.header.as_ref()?;
        let transaction = match validate_snapshot_transaction(batch) {
            Ok(transaction) => transaction,
            Err(message) => return Some(self.make_snapshot_transaction_error(hdr, message)),
        };
        let key = match self.stream_key(hdr, service_identity) {
            Ok(key) => key,
            Err(message) => return Some(self.make_snapshot_transaction_error(hdr, message)),
        };
        let seq = hdr.sequence;
        let Some(next_sequence) = seq.checked_add(1).filter(|_| seq != 0) else {
            return Some(self.make_sequence_error(hdr));
        };

        let mut state = self.state.lock().expect("ingest lock poisoned");
        self.prune_expired_streams(&mut state);
        if !state.streams.contains_key(&key) && state.streams.len() >= self.max_streams {
            return Some(
                self.make_snapshot_transaction_error(hdr, "ingest stream capacity is exhausted"),
            );
        }
        let stream = state
            .streams
            .entry(key.clone())
            .or_insert_with(StreamState::new);
        stream.last_seen = Instant::now();

        // ── Duplicate: already ingested this sequence ──
        if seq < stream.expected_seq {
            return Some(self.make_batch_ack(hdr, seq));
        }

        // ── Gap: only a new authoritative transaction at index 0 may reset it. ──
        if seq > stream.expected_seq
            && transaction.is_none_or(|transaction| transaction.chunk_index != 0)
        {
            return Some(self.make_resync_required(hdr, stream.expected_seq));
        }

        if let Some(transaction) = transaction {
            if transaction.chunk_count as usize > self.max_snapshot_entries {
                return Some(self.make_snapshot_transaction_error(
                    hdr,
                    "snapshot transaction chunk count exceeds the configured capacity",
                ));
            }
            if transaction.chunk_index == 0 {
                stream.snapshot_transaction = Some(SnapshotTransactionState::new(
                    transaction.transaction_id.clone(),
                    transaction.chunk_count,
                ));
            }
            let Some(staged) = stream.snapshot_transaction.as_mut() else {
                return Some(self.make_snapshot_transaction_error(
                    hdr,
                    "full snapshot transaction must start at chunk index 0",
                ));
            };
            if !staged.accepts(
                &transaction.transaction_id,
                transaction.chunk_index,
                transaction.chunk_count,
            ) {
                return Some(self.make_snapshot_transaction_error(
                    hdr,
                    "full snapshot transaction chunk is out of order or conflicts with the active transaction",
                ));
            }
            if staged.entry_count().saturating_add(batch.entries.len()) > self.max_snapshot_entries
            {
                return Some(self.make_snapshot_transaction_error(
                    hdr,
                    "snapshot transaction entries exceed the configured capacity",
                ));
            }
            if transaction.end {
                let candidate = staged.complete_candidate(&batch.entries);
                if !commit(&candidate, true) {
                    return Some(self.make_repository_rejected(hdr, stream.expected_seq));
                }
                stream.snapshots = candidate
                    .into_iter()
                    .map(|entry| (entry.datasource_id, entry))
                    .collect();
                stream.snapshot_transaction = None;
                stream.initialized = true;
            } else {
                staged.stage(&batch.entries);
            }
            stream.expected_seq = next_sequence;
            self.ingest_count.fetch_add(1, Ordering::Relaxed);
            return Some(self.make_batch_ack(hdr, seq));
        }

        if stream.snapshot_transaction.is_some() {
            return Some(self.make_snapshot_transaction_error(
                hdr,
                "incremental snapshot cannot interleave with an incomplete full snapshot transaction",
            ));
        }
        if !stream.initialized {
            return Some(self.make_hello_ack(hdr, true));
        }
        if !commit(&batch.entries, false) {
            return Some(self.make_repository_rejected(hdr, stream.expected_seq));
        }
        stream.expected_seq = next_sequence;
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

    fn prune_expired_streams(&self, state: &mut IngestState) {
        let now = Instant::now();
        state
            .streams
            .retain(|_, stream| now.saturating_duration_since(stream.last_seen) < self.stream_ttl);
    }

    fn stream_key(
        &self,
        hdr: &FrameHeader,
        service_identity: Option<&ServiceIdentity>,
    ) -> Result<StreamKey, &'static str> {
        let service_identity = if let Some(service_identity) = service_identity {
            if service_identity.service_name() != hdr.service_name
                || service_identity.instance_id() != hdr.instance_id
                || service_identity.boot_id() != hdr.boot_id
            {
                return Err("typed service identity does not match frame header");
            }
            service_identity.clone()
        } else {
            ServiceIdentity::try_new(&hdr.service_name, &hdr.instance_id, &hdr.boot_id)?
        };
        Ok(StreamKey {
            service_identity,
            stream_epoch: hdr.stream_epoch,
        })
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

    fn make_repository_rejected(&self, hdr: &FrameHeader, expected: u64) -> ServerFrame {
        ServerFrame {
            payload: Some(server_frame::Payload::ResyncRequired(ResyncRequired {
                header: Some(self.reply_header(hdr)),
                reason: "repository_rejected".to_owned(),
                expected_sequence: expected,
            })),
        }
    }

    fn make_sequence_error(&self, hdr: &FrameHeader) -> ServerFrame {
        ServerFrame {
            payload: Some(server_frame::Payload::Error(ServerError {
                header: Some(self.reply_header(hdr)),
                code: 400,
                message: "snapshot sequence must be between 1 and u64::MAX - 1".to_owned(),
                close_stream: true,
            })),
        }
    }

    fn make_snapshot_transaction_error(&self, hdr: &FrameHeader, message: &str) -> ServerFrame {
        ServerFrame {
            payload: Some(server_frame::Payload::Error(ServerError {
                header: Some(self.reply_header(hdr)),
                code: 400,
                message: message.to_owned(),
                close_stream: true,
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
        let Ok(service_identity) = ServiceIdentity::try_new(service, instance, boot) else {
            return Vec::new();
        };
        let mut state = self.state.lock().expect("ingest lock poisoned");
        self.prune_expired_streams(&mut state);
        state
            .streams
            .iter()
            .filter(|(key, _)| key.service_identity == service_identity)
            .flat_map(|(_, v)| v.snapshots.values().cloned())
            .collect()
    }
}

fn validate_snapshot_transaction(
    batch: &crate::protocol::SnapshotBatch,
) -> Result<Option<&SnapshotTransaction>, &'static str> {
    if !batch.is_full_snapshot {
        return if batch.snapshot_transaction.is_none() {
            Ok(None)
        } else {
            Err("incremental snapshot must not contain a snapshot transaction")
        };
    }
    let transaction = batch
        .snapshot_transaction
        .as_ref()
        .ok_or("full snapshot requires a snapshot transaction")?;
    if transaction.transaction_id.trim().is_empty() {
        return Err("snapshot transaction ID must be non-empty");
    }
    let is_last = transaction
        .chunk_index
        .checked_add(1)
        .is_some_and(|next| next == transaction.chunk_count);
    if transaction.chunk_count == 0
        || transaction.chunk_index >= transaction.chunk_count
        || transaction.end != is_last
    {
        return Err("snapshot transaction chunk index/count/end combination is invalid");
    }
    Ok(Some(transaction))
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
