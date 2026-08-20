//! Client-side push worker.
//!
//! Consumes [`PushEvent`]s from the metrics aggregator, assigns sequence
//! numbers via a [`SequenceWindow`], and pushes batches to the server
//! through a transport abstraction. Processes ACKs to free window slots.
//!
//! On reconnect the caller re-enqueues pending batches from the
//! [`SequenceWindow`] as [`PushEvent::Resend`] items.

use tokio::sync::mpsc;

use crate::protocol::{client_frame, ClientFrame, FrameHeader, ServerFrame, SnapshotBatch};
use crate::sequence_window::SequenceWindow;

/// Events consumed by the [`PushWorker`].
#[derive(Debug, Clone)]
pub enum PushEvent {
    /// A fresh batch from the aggregator.
    Batch {
        /// Serialized snapshot payload bytes.
        payload_bytes: Vec<u8>,
    },
    /// A batch being retransmitted after reconnect, preserving its original
    /// sequence number.
    Resend {
        /// Original sequence number to reuse.
        sequence: u64,
        /// Payload bytes to retransmit.
        payload_bytes: Vec<u8>,
    },
}

/// Builder for creating the worker and retaining test-side channels.
///
/// # Usage
///
/// ```ignore
/// let mut pair = TransportPair::new(64);
/// let worker = pair.build_worker(batch_rx, 256);
/// // pair.client_rx and pair.server_tx are still available for the test.
/// ```
pub struct TransportPair {
    /// Test reads `ClientFrames` the worker sent.
    pub client_rx: mpsc::Receiver<ClientFrame>,
    /// Test sends `ServerFrames` (simulated ACKs) to the worker.
    pub server_tx: mpsc::Sender<ServerFrame>,
    /// Worker sends `ClientFrames` here.
    client_tx: mpsc::Sender<ClientFrame>,
    /// Worker reads `ServerFrames` from here.
    server_rx: mpsc::Receiver<ServerFrame>,
}

impl TransportPair {
    /// Create a new transport pair with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (client_tx, client_rx) = mpsc::channel(capacity);
        let (server_tx, server_rx) = mpsc::channel(capacity);
        Self {
            client_rx,
            server_tx,
            client_tx,
            server_rx,
        }
    }

    /// Split into test-side channels and a [`PushWorker`].
    ///
    /// Returns `(client_rx, server_tx, worker)` where:
    /// - `client_rx`: test reads `ClientFrames` the worker sent.
    /// - `server_tx`: test sends `ServerFrames` (simulated ACKs) to the worker.
    /// - `worker`: the ready-to-run `PushWorker`.
    pub fn into_worker(
        self,
        batch_rx: mpsc::Receiver<PushEvent>,
        window_capacity: usize,
    ) -> (
        mpsc::Receiver<ClientFrame>,
        mpsc::Sender<ServerFrame>,
        PushWorker,
    ) {
        let worker = PushWorker {
            batch_rx,
            transport_tx: self.client_tx,
            transport_rx: self.server_rx,
            window: SequenceWindow::new(window_capacity),
            service_name: "druid-pool".to_owned(),
            instance_id: "local".to_owned(),
            boot_id: String::new(),
            stream_epoch: 0,
        };
        (self.client_rx, self.server_tx, worker)
    }
}

/// Client-side push worker.
///
/// Drives the send/receive loop:
/// 1. Receives [`PushEvent`]s from the batch channel.
/// 2. For new batches, assigns a sequence number via the window.
/// 3. Sends the frame to the server via the transport.
/// 4. Listens for ACKs and frees window slots.
pub struct PushWorker {
    batch_rx: mpsc::Receiver<PushEvent>,
    transport_tx: mpsc::Sender<ClientFrame>,
    transport_rx: mpsc::Receiver<ServerFrame>,
    window: SequenceWindow,
    /// Protocol-level metadata attached to every frame.
    service_name: String,
    instance_id: String,
    boot_id: String,
    stream_epoch: u64,
}

impl PushWorker {
    /// Create a new push worker with explicit identity fields (for testing).
    pub fn with_identity(
        batch_rx: mpsc::Receiver<PushEvent>,
        transport_tx: mpsc::Sender<ClientFrame>,
        transport_rx: mpsc::Receiver<ServerFrame>,
        window_capacity: usize,
        service_name: String,
        instance_id: String,
        boot_id: String,
        stream_epoch: u64,
    ) -> Self {
        Self {
            batch_rx,
            transport_tx,
            transport_rx,
            window: SequenceWindow::new(window_capacity),
            service_name,
            instance_id,
            boot_id,
            stream_epoch,
        }
    }

    /// Run the push worker until the batch channel is closed.
    ///
    /// Returns when either:
    /// - The batch sender is dropped and all pending events are processed.
    /// - The transport send channel is closed (server disconnected).
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                biased;

                // ── receive ACKs from server ──
                server_frame = self.transport_rx.recv() => {
                    if let Some(frame) = server_frame {
                        if let Some(reply) = self.handle_server_frame(frame) {
                            if self.transport_tx.send(reply).await.is_err() {
                                tracing::error!("transport closed while sending command ACK");
                                break;
                            }
                        }
                    } else {
                        // Server disconnected. Caller must reconnect.
                        tracing::warn!("server disconnected (ACK channel closed)");
                        break;
                    }
                }

                // ── receive batches from aggregator ──
                event = self.batch_rx.recv() => {
                    match event {
                        Some(PushEvent::Batch { payload_bytes }) => {
                            if let Err(e) = self.push_new_batch(payload_bytes).await {
                                tracing::error!(error = %e, "failed to push batch");
                                break;
                            }
                        }
                        Some(PushEvent::Resend { sequence, payload_bytes }) => {
                            if let Err(e) = self.push_resend(sequence, payload_bytes).await {
                                tracing::error!(error = %e, "failed to resend batch");
                                break;
                            }
                        }
                        None => {
                            // Batch channel closed -- normal shutdown.
                            tracing::debug!("batch channel closed, shutting down push worker");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Push a new batch, assigning a sequence number from the window.
    async fn push_new_batch(&mut self, payload_bytes: Vec<u8>) -> Result<(), PushWorkerError> {
        let seq = self
            .window
            .push(payload_bytes.clone())
            .map_err(PushWorkerError::WindowFull)?;

        let frame = self.make_batch_frame(seq, false);
        self.transport_tx
            .send(frame)
            .await
            .map_err(|_| PushWorkerError::TransportClosed)?;
        Ok(())
    }

    /// Resend a batch with an explicit sequence number (reconnect path).
    async fn push_resend(
        &mut self,
        sequence: u64,
        _payload_bytes: Vec<u8>,
    ) -> Result<(), PushWorkerError> {
        let frame = self.make_batch_frame(sequence, false);
        self.transport_tx
            .send(frame)
            .await
            .map_err(|_| PushWorkerError::TransportClosed)?;
        Ok(())
    }

    /// Handle a server frame (ACK, command, error, etc.).
    ///
    /// Returns an optional [`ClientFrame`] to send back to the server
    /// (e.g. a `CommandAck` in response to a command).
    fn handle_server_frame(&mut self, frame: ServerFrame) -> Option<ClientFrame> {
        use crate::protocol::server_frame::Payload;

        match frame.payload {
            Some(Payload::BatchAck(ack)) => {
                if let Err(e) = self.window.ack(ack.accepted_sequence) {
                    tracing::warn!(
                        sequence = ack.accepted_sequence,
                        error = %e,
                        "unexpected ACK"
                    );
                } else {
                    tracing::debug!(sequence = ack.accepted_sequence, "batch acknowledged");
                }
                None
            }
            Some(Payload::HelloAck(_)) => {
                tracing::debug!("received HelloAck");
                None
            }
            Some(Payload::ResyncRequired(resync)) => {
                tracing::warn!(
                    reason = resync.reason,
                    expected = resync.expected_sequence,
                    "server requested resync"
                );
                None
            }
            Some(Payload::Command(cmd)) => {
                tracing::info!(command_id = cmd.command_id, "received server command");
                self.handle_command(cmd)
            }
            Some(Payload::Error(err)) => {
                tracing::error!(code = err.code, message = err.message, "server error");
                None
            }
            None => None,
        }
    }

    /// Process a server command and return a `CommandAck` reply.
    fn handle_command(&self, cmd: crate::protocol::Command) -> Option<ClientFrame> {
        use crate::protocol::command::Payload as CmdPayload;

        let (success, error_message) = match cmd.payload {
            Some(CmdPayload::ResetStats(reset)) => {
                tracing::info!(
                    datasource_ids = ?reset.target_datasource_ids,
                    "executing local reset"
                );
                // V1: local reset is a no-op (no actual datasource references here).
                // In production this would call DataSourceMonitorable::reset_stat()
                // on each registered datasource matching the IDs.
                (true, String::new())
            }
            Some(CmdPayload::RequestFullSnapshot(_)) => {
                tracing::info!("received RequestFullSnapshot command");
                // V1: acknowledge without actually sending a full snapshot.
                (true, String::new())
            }
            None => {
                tracing::warn!("received command with no payload");
                (false, "empty command payload".into())
            }
        };

        Some(ClientFrame {
            payload: Some(client_frame::Payload::CommandAck(
                crate::protocol::CommandAck {
                    header: Some(FrameHeader {
                        protocol_version: 1,
                        service_name: self.service_name.clone(),
                        instance_id: self.instance_id.clone(),
                        boot_id: self.boot_id.clone(),
                        stream_epoch: self.stream_epoch,
                        sequence: 0,
                        emitted_at_unix_ms: now_unix_ms(),
                    }),
                    command_id: cmd.command_id,
                    success,
                    error_message,
                },
            )),
        })
    }

    fn make_batch_frame(&self, seq: u64, is_full_snapshot: bool) -> ClientFrame {
        ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(self.make_header(seq)),
                entries: Vec::new(), // payload_bytes carried externally in V1
                is_full_snapshot,
            })),
        }
    }

    fn make_header(&self, seq: u64) -> FrameHeader {
        FrameHeader {
            protocol_version: 1,
            service_name: self.service_name.clone(),
            instance_id: self.instance_id.clone(),
            boot_id: self.boot_id.clone(),
            stream_epoch: self.stream_epoch,
            sequence: seq,
            emitted_at_unix_ms: now_unix_ms(),
        }
    }
}

/// Errors produced by the push worker.
#[derive(Debug, thiserror::Error)]
pub enum PushWorkerError {
    #[error("sequence window full")]
    WindowFull(#[source] crate::sequence_window::SequenceWindowError),
    #[error("transport channel closed")]
    TransportClosed,
}

/// Returns the current time as milliseconds since Unix epoch.
fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
