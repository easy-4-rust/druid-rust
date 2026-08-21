//! 客户端指标推送 worker。
//!
//! Worker 消费类型化 [`AggregatedBatch`]，分配流内 sequence，把完整
//! [`ClientFrame`] 只编码一次后交给 [`SequenceWindow`] 保存。连接断开时返回未 ACK
//! frame；调用方重连后通过 [`PushEvent::Resend`] 恢复并发送完全相同的字节。

use std::collections::VecDeque;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::aggregator::AggregatedBatch;
use crate::protocol::{
    client_frame, decode_client_frame, encode_client_frame, ClientFrame, FrameHeader, ServerFrame,
    SnapshotBatch,
};
use crate::registry::RegistryCommandExecutor;
use crate::sequence_window::{PendingFrame, SequenceWindow, SequenceWindowError};

/// PushWorker 消费的新批次或重连 frame。
#[derive(Debug, Clone)]
pub enum PushEvent {
    /// 来自 aggregator 的新类型化批次。
    Batch(AggregatedBatch),
    /// 上一连接留下的未 ACK 完整 frame。
    Resend(PendingFrame),
}

/// 为测试及后续真实 transport 适配提供成对的有界 frame 通道。
pub struct TransportPair {
    /// 测试侧接收 worker 发出的客户端 frame。
    pub client_rx: mpsc::Receiver<ClientFrame>,
    /// 测试侧向 worker 发送服务端 frame。
    pub server_tx: mpsc::Sender<ServerFrame>,
    client_tx: mpsc::Sender<ClientFrame>,
    server_rx: mpsc::Receiver<ServerFrame>,
}

impl TransportPair {
    /// 创建指定通道容量的 transport pair。
    #[must_use]
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

    /// 拆分测试侧通道并创建使用默认本地身份的 PushWorker。
    pub fn into_worker(
        self,
        batch_rx: mpsc::Receiver<PushEvent>,
        window_capacity: usize,
    ) -> (
        mpsc::Receiver<ClientFrame>,
        mpsc::Sender<ServerFrame>,
        PushWorker,
    ) {
        let deferred_server_capacity = self.server_rx.max_capacity().max(1);
        let worker = PushWorker {
            batch_rx,
            transport_tx: self.client_tx,
            transport_rx: self.server_rx,
            window: SequenceWindow::new(window_capacity),
            service_name: "druid-pool".to_owned(),
            instance_id: "local".to_owned(),
            boot_id: String::new(),
            stream_epoch: 0,
            cancellation: CancellationToken::new(),
            command_executor: None,
            deferred_server_frames: VecDeque::new(),
            deferred_server_capacity,
        };
        (self.client_rx, self.server_tx, worker)
    }
}

/// 单写者客户端推送状态机。
///
/// 窗口饱和时停止从 batch channel 接收新事件，但继续接收 ACK 和取消信号。
pub struct PushWorker {
    batch_rx: mpsc::Receiver<PushEvent>,
    transport_tx: mpsc::Sender<ClientFrame>,
    transport_rx: mpsc::Receiver<ServerFrame>,
    window: SequenceWindow,
    service_name: String,
    instance_id: String,
    boot_id: String,
    stream_epoch: u64,
    cancellation: CancellationToken,
    command_executor: Option<RegistryCommandExecutor>,
    deferred_server_frames: VecDeque<ServerFrame>,
    deferred_server_capacity: usize,
}

impl PushWorker {
    /// 使用显式协议身份创建 PushWorker。
    #[allow(clippy::too_many_arguments)]
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
        let deferred_server_capacity = transport_rx.max_capacity().max(1);
        Self {
            batch_rx,
            transport_tx,
            transport_rx,
            window: SequenceWindow::new(window_capacity),
            service_name,
            instance_id,
            boot_id,
            stream_epoch,
            cancellation: CancellationToken::new(),
            command_executor: None,
            deferred_server_frames: VecDeque::new(),
            deferred_server_capacity,
        }
    }

    /// 注入由 supervisor 持有的取消令牌。
    #[must_use]
    pub fn with_cancellation_token(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = cancellation;
        self
    }

    /// 注入真实 datasource registry 命令执行器。
    #[must_use]
    pub fn with_command_executor(mut self, command_executor: RegistryCommandExecutor) -> Self {
        self.command_executor = Some(command_executor);
        self
    }

    /// 运行 worker，直到取消、输入关闭或 transport 断开。
    ///
    /// 返回仍未 ACK 的完整 frame，供下一连接按原 sequence、原字节恢复。
    pub async fn run(mut self) -> Vec<PendingFrame> {
        loop {
            if !self.window.is_full() {
                if let Some(frame) = self.deferred_server_frames.pop_front() {
                    if !self.process_server_frame(frame).await {
                        break;
                    }
                    continue;
                }
            }

            enum Event {
                Cancelled,
                Server(Option<ServerFrame>),
                Batch(Option<PushEvent>),
            }

            let event = tokio::select! {
                biased;
                () = self.cancellation.cancelled() => Event::Cancelled,
                frame = self.transport_rx.recv() => Event::Server(frame),
                batch = self.batch_rx.recv(), if !self.window.is_full() => Event::Batch(batch),
            };

            match event {
                Event::Cancelled | Event::Batch(None) => break,
                Event::Server(Some(frame)) => {
                    if !self.process_server_frame(frame).await {
                        break;
                    }
                }
                Event::Server(None) => {
                    tracing::warn!("server disconnected");
                    break;
                }
                Event::Batch(Some(event)) => {
                    if let Err(error) = self.process_push_event(event).await {
                        tracing::error!(%error, "failed to push metrics frame");
                        break;
                    }
                }
            }
        }

        self.window.pending_frames()
    }

    async fn process_push_event(&mut self, event: PushEvent) -> Result<(), PushWorkerError> {
        match event {
            PushEvent::Batch(batch) => self.push_new_batch(batch).await,
            PushEvent::Resend(pending) => self.push_resend(pending).await,
        }
    }

    async fn push_new_batch(&mut self, batch: AggregatedBatch) -> Result<(), PushWorkerError> {
        let service_name = self.service_name.clone();
        let instance_id = self.instance_id.clone();
        let boot_id = self.boot_id.clone();
        let stream_epoch = self.stream_epoch;
        let pending = self
            .window
            .push_with(move |sequence| {
                encode_client_frame(&ClientFrame {
                    payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                        header: Some(FrameHeader {
                            protocol_version: 1,
                            service_name,
                            instance_id,
                            boot_id,
                            stream_epoch,
                            sequence,
                            emitted_at_unix_ms: now_unix_ms(),
                        }),
                        entries: batch.entries,
                        is_full_snapshot: batch.is_full_snapshot,
                    })),
                })
            })
            .map_err(PushWorkerError::Window)?;
        self.send_encoded_frame(&pending.encoded_frame).await
    }

    async fn push_resend(&mut self, pending: PendingFrame) -> Result<(), PushWorkerError> {
        let frame = decode_client_frame(&pending.encoded_frame).map_err(PushWorkerError::Decode)?;
        let encoded_sequence = match frame.payload.as_ref() {
            Some(client_frame::Payload::SnapshotBatch(batch)) => batch
                .header
                .as_ref()
                .map(|header| header.sequence)
                .ok_or(PushWorkerError::ResendWithoutSnapshotHeader)?,
            _ => return Err(PushWorkerError::ResendWithoutSnapshotHeader),
        };
        if encoded_sequence != pending.sequence {
            return Err(PushWorkerError::ResendSequenceMismatch {
                pending_sequence: pending.sequence,
                encoded_sequence,
            });
        }
        self.window
            .restore(pending)
            .map_err(PushWorkerError::Window)?;
        self.send_outbound(frame).await
    }

    async fn send_encoded_frame(&mut self, encoded_frame: &[u8]) -> Result<(), PushWorkerError> {
        let frame = decode_client_frame(encoded_frame).map_err(PushWorkerError::Decode)?;
        self.send_outbound(frame).await
    }

    async fn send_outbound(&mut self, frame: ClientFrame) -> Result<(), PushWorkerError> {
        let transport_tx = self.transport_tx.clone();
        let send = transport_tx.send(frame);
        tokio::pin!(send);

        loop {
            tokio::select! {
                biased;
                () = self.cancellation.cancelled() => return Err(PushWorkerError::Cancelled),
                result = &mut send => {
                    return result.map_err(|_| PushWorkerError::TransportClosed);
                }
                server_frame = self.transport_rx.recv() => {
                    let frame = server_frame.ok_or(PushWorkerError::TransportClosed)?;
                    self.process_server_while_sending(frame)?;
                }
            }
        }
    }

    fn process_server_while_sending(&mut self, frame: ServerFrame) -> Result<(), PushWorkerError> {
        use crate::protocol::server_frame::Payload;

        if let Some(Payload::BatchAck(ack)) = frame.payload.as_ref() {
            if let Err(error) = self.window.ack(ack.accepted_sequence) {
                tracing::warn!(sequence = ack.accepted_sequence, %error, "unexpected ACK");
            }
            return Ok(());
        }
        if self.deferred_server_frames.len() >= self.deferred_server_capacity {
            return Err(PushWorkerError::ServerInboundOverload {
                capacity: self.deferred_server_capacity,
            });
        }
        self.deferred_server_frames.push_back(frame);
        Ok(())
    }

    async fn process_server_frame(&mut self, frame: ServerFrame) -> bool {
        use crate::protocol::server_frame::Payload;

        match frame.payload {
            Some(Payload::BatchAck(ack)) => {
                if let Err(error) = self.window.ack(ack.accepted_sequence) {
                    tracing::warn!(sequence = ack.accepted_sequence, %error, "unexpected ACK");
                }
                true
            }
            Some(Payload::Command(command))
                if self.window.is_full()
                    && matches!(
                        command.payload.as_ref(),
                        Some(crate::protocol::command::Payload::RequestFullSnapshot(_))
                    ) =>
            {
                if self.window.capacity() == 0 {
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("ACK window capacity is zero".to_owned()),
                    );
                    return self.send_outbound(reply).await.is_ok();
                }
                if self.deferred_server_frames.is_empty() {
                    self.deferred_server_frames.push_back(ServerFrame {
                        payload: Some(Payload::Command(command)),
                    });
                    true
                } else {
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("another full snapshot command is waiting for ACK capacity".to_owned()),
                    );
                    self.send_outbound(reply).await.is_ok()
                }
            }
            Some(Payload::Command(command)) => {
                let reply = self.handle_command(command).await;
                self.send_outbound(reply).await.is_ok()
            }
            Some(Payload::HelloAck(_)) | None => true,
            Some(Payload::ResyncRequired(resync)) => {
                tracing::warn!(
                    reason = resync.reason,
                    expected = resync.expected_sequence,
                    "server requested resync"
                );
                true
            }
            Some(Payload::Error(error)) => {
                tracing::error!(code = error.code, message = error.message, "server error");
                !error.close_stream
            }
        }
    }

    async fn handle_command(&mut self, command: crate::protocol::Command) -> ClientFrame {
        use crate::protocol::command::Payload;

        let command_id = command.command_id;
        let result = if command_id.trim().is_empty() {
            Err("empty command ID".to_owned())
        } else {
            match command.payload {
                Some(Payload::ResetStats(reset)) => self
                    .command_executor
                    .as_ref()
                    .ok_or_else(|| "datasource registry is not configured".to_owned())
                    .and_then(|executor| {
                        executor
                            .reset_stats(&reset.target_datasource_ids)
                            .map_err(|error| error.to_string())
                    }),
                Some(Payload::RequestFullSnapshot(_)) => {
                    let batches = self
                        .command_executor
                        .as_ref()
                        .ok_or_else(|| "datasource registry is not configured".to_owned())
                        .and_then(|executor| {
                            executor.full_snapshot().map_err(|error| error.to_string())
                        });
                    match batches {
                        Ok(batches) => self
                            .push_full_snapshot_batches(batches)
                            .await
                            .map_err(|error| error.to_string()),
                        Err(error) => Err(error),
                    }
                }
                None => Err("empty command payload".to_owned()),
            }
        };

        self.make_command_ack(command_id, result)
    }

    fn make_command_ack(&self, command_id: String, result: Result<(), String>) -> ClientFrame {
        ClientFrame {
            payload: Some(client_frame::Payload::CommandAck(
                crate::protocol::CommandAck {
                    header: Some(self.make_header(0)),
                    command_id,
                    success: result.is_ok(),
                    error_message: result.err().unwrap_or_default(),
                },
            )),
        }
    }

    async fn push_full_snapshot_batches(
        &mut self,
        batches: Vec<AggregatedBatch>,
    ) -> Result<(), PushWorkerError> {
        for batch in batches {
            self.wait_for_window_capacity().await?;
            self.push_new_batch(batch).await?;
        }
        Ok(())
    }

    async fn wait_for_window_capacity(&mut self) -> Result<(), PushWorkerError> {
        use crate::protocol::server_frame::Payload;

        while self.window.is_full() {
            let frame = tokio::select! {
                biased;
                () = self.cancellation.cancelled() => return Err(PushWorkerError::Cancelled),
                frame = self.transport_rx.recv() => frame.ok_or(PushWorkerError::TransportClosed)?,
            };

            match frame.payload {
                Some(Payload::BatchAck(ack)) => {
                    self.window
                        .ack(ack.accepted_sequence)
                        .map_err(PushWorkerError::Window)?;
                }
                Some(Payload::Command(command)) => {
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("full snapshot command is waiting for ACK capacity".to_owned()),
                    );
                    self.send_outbound(reply).await?;
                }
                Some(Payload::Error(error)) if error.close_stream => {
                    return Err(PushWorkerError::ServerInterrupted(error.message));
                }
                Some(Payload::ResyncRequired(resync)) => {
                    return Err(PushWorkerError::ServerInterrupted(resync.reason));
                }
                Some(Payload::HelloAck(_) | Payload::Error(_)) | None => {}
            }
        }
        Ok(())
    }

    fn make_header(&self, sequence: u64) -> FrameHeader {
        FrameHeader {
            protocol_version: 1,
            service_name: self.service_name.clone(),
            instance_id: self.instance_id.clone(),
            boot_id: self.boot_id.clone(),
            stream_epoch: self.stream_epoch,
            sequence,
            emitted_at_unix_ms: now_unix_ms(),
        }
    }
}

/// PushWorker 的窗口、编码或 transport 错误。
#[derive(Debug, thiserror::Error)]
pub enum PushWorkerError {
    /// SequenceWindow 拒绝 frame。
    #[error(transparent)]
    Window(#[from] SequenceWindowError),
    /// 保存的 protobuf frame 无法解码。
    #[error("stored client frame is invalid: {0}")]
    Decode(#[source] prost::DecodeError),
    /// 重连 frame 不是带 header 的 SnapshotBatch。
    #[error("stored resend frame is not a snapshot batch with a header")]
    ResendWithoutSnapshotHeader,
    /// PendingFrame 的索引 sequence 与编码 header 不一致。
    #[error(
        "stored resend sequence mismatch: pending {pending_sequence}, encoded {encoded_sequence}"
    )]
    ResendSequenceMismatch {
        /// PendingFrame 对外声明的 sequence。
        pending_sequence: u64,
        /// 编码在 SnapshotBatch header 中的 sequence。
        encoded_sequence: u64,
    },
    /// 当前 transport 已关闭。
    #[error("transport channel closed")]
    TransportClosed,
    /// Worker 在等待 ACK 容量时被 supervisor 取消。
    #[error("push worker cancelled")]
    Cancelled,
    /// 服务端在全量快照分块过程中中断当前发送序列。
    #[error("server interrupted full snapshot: {0}")]
    ServerInterrupted(String),
    /// transport 饱和期间服务端继续灌入非 ACK frame，超过了有界延迟槽。
    #[error("server inbound deferred queue full: capacity {capacity}")]
    ServerInboundOverload {
        /// 最多延迟处理的服务端 frame 数。
        capacity: usize,
    },
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
