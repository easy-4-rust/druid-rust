//! 客户端指标推送 worker。
//!
//! Worker 消费类型化 [`AggregatedBatch`]，分配流内 sequence，把完整
//! [`ClientFrame`] 只编码一次后交给 [`SequenceWindow`] 保存。连接断开时返回未 ACK
//! frame；调用方重连后通过 [`PushEvent::Resend`] 恢复并发送完全相同的字节。

use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::aggregator::AggregatedBatch;
use crate::command_journal::CommandJournal;
use crate::command_payload::canonical_command_payload;
use crate::pipeline_ownership::{
    CommandTransition, CommandWorkKey, PipelineOwnership, PipelineOwnershipError,
};
use crate::protocol::{
    client_frame, decode_client_frame, encode_client_frame, ClientFrame, FrameHeader, ServerFrame,
    SnapshotBatch, SnapshotTransaction,
};
use crate::registry::RegistryCommandExecutor;
use crate::sequence_window::{PendingFrame, SequenceWindow, SequenceWindowError};

#[doc(hidden)]
pub use crate::pipeline_ownership::BatchWorkId;

/// PushWorker 消费的新批次或重连 frame。
#[derive(Debug, Clone)]
pub enum PushEvent {
    /// 来自 aggregator 的新类型化批次。
    Batch(AggregatedBatch),
    /// 带 supervisor cutoff generation 的 live 聚合批次。
    BatchAt {
        /// 与 ownership ledger 中 batch 一一对应的单调工作标识。
        work_id: BatchWorkId,
        /// 入队时观察到的 resync generation。
        generation: u64,
        /// 已类型化、隐私处理的批次。
        batch: AggregatedBatch,
    },
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
        let deferred_server_capacity = self.server_rx.max_capacity().saturating_mul(2).max(1);
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
            replay_frames: VecDeque::new(),
            force_first_batch_full_snapshot: false,
            initial_batches: VecDeque::new(),
            progress: None,
            resync_required: false,
            full_snapshot_command: None,
            completed_full_snapshot_commands: VecDeque::new(),
            reply_sent_this_session: false,
            preserved_live_events: VecDeque::new(),
            snapshot_transaction: None,
            command_journal: Arc::new(CommandJournal::new(window_capacity)),
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
    replay_frames: VecDeque<PendingFrame>,
    force_first_batch_full_snapshot: bool,
    initial_batches: VecDeque<AggregatedBatch>,
    progress: Option<PushProgress>,
    resync_required: bool,
    full_snapshot_command: Option<FullSnapshotCommandState>,
    completed_full_snapshot_commands: VecDeque<String>,
    reply_sent_this_session: bool,
    preserved_live_events: VecDeque<PushEvent>,
    snapshot_transaction: Option<(String, u32, u32)>,
    command_journal: Arc<CommandJournal>,
}

/// 一次 transport 会话退出后交还给重连 supervisor 的全部状态。
pub struct PushWorkerExit {
    /// 仍未 ACK、必须按原字节重发的 frame。
    pub pending_frames: Vec<PendingFrame>,
    /// 尚未消费的聚合批次接收端；重连后继续由同一单写者状态机消费。
    pub batch_rx: mpsc::Receiver<PushEvent>,
    /// 下一网络会话必须继续使用的 sequence 分配位置。
    pub next_sequence: Option<u64>,
    /// 当前连接尚未来得及发送的权威全量批次。
    pub authoritative_batches: Vec<AggregatedBatch>,
    /// 服务端是否要求 supervisor 生成 registry 权威全量替换。
    pub resync_required: bool,
    /// 尚未在全部全量 chunk ACK 后成功回复的远程命令 ID。
    pub full_snapshot_command: Option<FullSnapshotCommandState>,
    /// 已由有序 Goodbye/EOF 确认的命令 ID replay cache。
    pub completed_full_snapshot_commands: Vec<String>,
    /// 当前 transport 是否至少写入过一次 ReplyReady 成功回复。
    pub reply_sent_this_session: bool,
    /// cutoff 后入队、必须在权威替换后继续发送的 live events。
    pub preserved_live_events: Vec<PushEvent>,
    /// 旧 transport 尚未被 worker 消费的有界服务端帧接收端。
    pub transport_rx: mpsc::Receiver<ServerFrame>,
    /// 已校验且允许跨连接迁移的远程命令帧。
    pub successor_commands: Vec<ServerFrame>,
    /// 当前尚未完成的 `(transaction_id, next_chunk_index, chunk_count)`。
    #[cfg(feature = "client")]
    pub(crate) snapshot_transaction: Option<(String, u32, u32)>,
}

/// 远程全量快照命令跨连接耐久生命周期。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FullSnapshotCommandState {
    /// 仍有 chunk 未 admitted 或未 ACK。
    PendingChunks { command_id: String },
    /// 所有 chunk 已 ACK，成功 CommandAck 必须重发直到有序关闭确认。
    ReplyReady { command_id: String },
}

impl FullSnapshotCommandState {
    pub(crate) fn command_id(&self) -> &str {
        match self {
            Self::PendingChunks { command_id } | Self::ReplyReady { command_id } => command_id,
        }
    }
}

impl PushWorkerExit {
    /// 在 CommandAck 之后的有序 Goodbye/EOF 完成时确认回复，并写入有界 replay cache。
    pub fn confirm_full_snapshot_command_reply(&mut self, replay_capacity: usize) {
        if !self.reply_sent_this_session {
            return;
        }
        let Some(FullSnapshotCommandState::ReplyReady { command_id }) =
            self.full_snapshot_command.take()
        else {
            return;
        };
        self.completed_full_snapshot_commands.push(command_id);
        let capacity = replay_capacity.max(1);
        if self.completed_full_snapshot_commands.len() > capacity {
            let excess = self.completed_full_snapshot_commands.len() - capacity;
            self.completed_full_snapshot_commands.drain(..excess);
        }
        self.reply_sent_this_session = false;
    }
}

/// 跨 transport 会话共享的真实 queued/pending 进度。
#[derive(Clone)]
pub struct PushProgress {
    ownership: Arc<PipelineOwnership>,
}

impl PushProgress {
    /// 创建空进度计数器。
    #[must_use]
    pub fn new() -> Self {
        Self::with_ownership(Arc::new(PipelineOwnership::new()))
    }

    pub(crate) fn with_ownership(ownership: Arc<PipelineOwnership>) -> Self {
        Self { ownership }
    }

    /// 返回尚未被 worker 消费的批次数。
    #[must_use]
    pub fn queued_batches(&self) -> usize {
        self.ownership.client_queued_batches()
    }

    /// 返回已编码发送但尚未 ACK 的 frame 数。
    #[must_use]
    pub fn pending_frames(&self) -> usize {
        self.ownership.pending_frames()
    }

    /// 返回当前真实未刷新总数。
    #[must_use]
    pub fn unflushed(&self) -> usize {
        self.ownership.client_unflushed()
    }

    /// 等待进度发生变化。
    pub async fn changed(&self) {
        self.ownership.changed().await;
    }

    /// 在批次进入有界客户端队列前记账。
    #[cfg(feature = "client")]
    pub(crate) fn queue_batch(&self) -> Result<BatchWorkId, PipelineOwnershipError> {
        self.ownership.queue_client_batch()
    }

    /// 把一组权威批次放到 live 队列之前，并保留批次顺序。
    pub(crate) fn queue_batches_front(&self, count: usize) -> Result<(), PipelineOwnershipError> {
        self.ownership.queue_client_batches_front(count)
    }

    pub(crate) fn client_front(&self) -> Result<BatchWorkId, PipelineOwnershipError> {
        self.ownership
            .client_front()
            .ok_or(PipelineOwnershipError::InvalidTransition(
                "client ownership is missing",
            ))
    }

    #[cfg(feature = "client")]
    pub(crate) fn drop_client_batch(&self, id: BatchWorkId) -> Result<(), PipelineOwnershipError> {
        self.ownership.drop_client_batch(id)
    }

    fn transfer_queued_batch_to_pending_frames(
        &self,
        id: BatchWorkId,
    ) -> Result<(), PipelineOwnershipError> {
        self.ownership.transfer_client_to_pending_frames(id)
    }

    pub(crate) fn verify_pending_frames(
        &self,
        pending: usize,
    ) -> Result<(), PipelineOwnershipError> {
        self.ownership.verify_pending_frames(pending)
    }

    #[cfg(feature = "client")]
    pub(crate) fn discard_pending_frames(
        &self,
        pending: usize,
    ) -> Result<(), PipelineOwnershipError> {
        self.ownership.discard_pending_frames(pending)
    }

    fn finish_pending_frame(&self) -> Result<(), PipelineOwnershipError> {
        let id =
            self.ownership
                .pending_front()
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "pending-frame ownership is missing",
                ))?;
        self.ownership.finish_pending_frame(id)
    }

    /// 返回已进入处理或 durable confirmation、尚未完成的远程命令数。
    #[must_use]
    pub fn pending_commands(&self) -> usize {
        self.ownership.processing_commands()
    }

    /// 返回已经验证并进入 transport queue、尚未开始处理的命令数。
    #[must_use]
    pub fn transport_queued_commands(&self) -> usize {
        self.ownership.transport_commands()
    }

    fn begin_command(
        &self,
        command: &crate::protocol::Command,
    ) -> Result<(CommandWorkKey, CommandTransition), PipelineOwnershipError> {
        let key = CommandWorkKey::from_command(command)?;
        let transition = self.ownership.start_command(&key)?;
        Ok((key, transition))
    }

    pub(crate) fn finish_command(
        &self,
        key: &CommandWorkKey,
    ) -> Result<(), PipelineOwnershipError> {
        self.ownership.finish_command(key)
    }

    #[cfg(feature = "client")]
    pub(crate) fn queue_transport_command(
        &self,
        command: &crate::protocol::Command,
    ) -> Result<CommandWorkKey, PipelineOwnershipError> {
        let key = CommandWorkKey::from_command(command)?;
        self.ownership.queue_command(key.clone())?;
        Ok(key)
    }

    #[cfg(feature = "client")]
    pub(crate) fn drop_transport_command(
        &self,
        key: &CommandWorkKey,
    ) -> Result<(), PipelineOwnershipError> {
        self.ownership.drop_command(key)
    }

    #[cfg(feature = "client")]
    pub(crate) fn transfer_exporter_batch_to_client(
        &self,
    ) -> Result<BatchWorkId, PipelineOwnershipError> {
        let id =
            self.ownership
                .exporter_front()
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "exporter ownership is missing",
                ))?;
        self.ownership.transfer_exporter_to_client(id)?;
        Ok(id)
    }
}

impl Default for PushProgress {
    fn default() -> Self {
        Self::new()
    }
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
        let deferred_server_capacity = transport_rx.max_capacity().saturating_mul(2).max(1);
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
            replay_frames: VecDeque::new(),
            force_first_batch_full_snapshot: false,
            initial_batches: VecDeque::new(),
            progress: None,
            resync_required: false,
            full_snapshot_command: None,
            completed_full_snapshot_commands: VecDeque::new(),
            reply_sent_this_session: false,
            preserved_live_events: VecDeque::new(),
            snapshot_transaction: None,
            command_journal: Arc::new(CommandJournal::new(window_capacity)),
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

    /// 注入由 gRPC supervisor 跨连接共享的有界命令日志。
    #[must_use]
    #[cfg(feature = "client")]
    pub(crate) fn with_command_journal(mut self, command_journal: Arc<CommandJournal>) -> Self {
        self.command_journal = command_journal;
        self
    }

    /// 恢复上一次网络会话留下的完整未 ACK frame。
    ///
    /// 所有 frame 会在任何网络发送前完成解码、sequence 一致性和窗口容量校验。
    pub fn with_pending_frames(
        mut self,
        pending_frames: Vec<PendingFrame>,
    ) -> Result<Self, PushWorkerError> {
        let mut validated = VecDeque::with_capacity(pending_frames.len());
        for pending in pending_frames {
            let frame = Self::decode_pending_frame(&pending)?;
            if let Some(client_frame::Payload::SnapshotBatch(batch)) = frame.payload.as_ref() {
                if batch.is_full_snapshot {
                    let transaction = batch.snapshot_transaction.as_ref().ok_or_else(|| {
                        PushWorkerError::InvalidSnapshotTransaction(
                            "stored full snapshot is missing transaction metadata".to_owned(),
                        )
                    })?;
                    validate_snapshot_transaction(transaction)?;
                    self.snapshot_transaction = if transaction.end {
                        None
                    } else {
                        Some((
                            transaction.transaction_id.clone(),
                            transaction.chunk_index.saturating_add(1),
                            transaction.chunk_count,
                        ))
                    };
                }
            }
            validated.push_back(pending);
        }
        for pending in &validated {
            self.window
                .restore(pending.clone())
                .map_err(PushWorkerError::Window)?;
        }
        self.replay_frames = validated;
        Ok(self)
    }

    /// 要求当前会话消费到的第一个新批次使用全量快照标记。
    #[must_use]
    pub fn with_first_batch_full_snapshot(mut self, required: bool) -> Self {
        self.force_first_batch_full_snapshot = required;
        self
    }

    /// 恢复 supervisor 保存的下一个 sequence，即使当前没有 pending frame。
    #[must_use]
    pub fn with_next_sequence(mut self, next_sequence: Option<u64>) -> Self {
        self.window = SequenceWindow::with_next_sequence(self.window.capacity(), next_sequence);
        self
    }

    /// 在恢复普通队列前发送权威 registry 全量快照批次。
    #[must_use]
    pub fn with_initial_batches(mut self, batches: Vec<AggregatedBatch>) -> Self {
        self.initial_batches = batches.into();
        self
    }

    /// 恢复尚未编码完的权威快照 transaction 进度。
    #[must_use]
    #[cfg(feature = "client")]
    pub(crate) fn with_snapshot_transaction(
        mut self,
        transaction: Option<(String, u32, u32)>,
    ) -> Self {
        if transaction.is_some() {
            self.snapshot_transaction = transaction;
        }
        self
    }

    /// 注入跨连接共享的真实进度计数器。
    #[must_use]
    pub fn with_progress(mut self, progress: PushProgress) -> Result<Self, PushWorkerError> {
        progress
            .verify_pending_frames(self.window.len())
            .map_err(ownership_error)?;
        self.progress = Some(progress);
        Ok(self)
    }

    /// 恢复跨 transport 会话仍待完成的远程全量快照命令。
    #[must_use]
    pub fn with_full_snapshot_command_state(
        mut self,
        state: Option<FullSnapshotCommandState>,
        completed_commands: Vec<String>,
    ) -> Self {
        self.full_snapshot_command = state;
        self.completed_full_snapshot_commands = completed_commands.into();
        self
    }

    /// 恢复 authoritative cutoff 后保留的 live events。
    #[must_use]
    pub fn with_preserved_live_events(mut self, events: Vec<PushEvent>) -> Self {
        self.preserved_live_events = events.into();
        self
    }

    /// 恢复旧连接 teardown 前已经接收并校验的远程命令。
    pub fn with_successor_commands(
        mut self,
        commands: Vec<ServerFrame>,
    ) -> Result<Self, PushWorkerError> {
        if commands.len() > self.deferred_server_capacity {
            return Err(PushWorkerError::SuccessorCommandOverflow {
                capacity: self.deferred_server_capacity,
            });
        }
        if commands.iter().any(|frame| {
            !matches!(
                frame.payload.as_ref(),
                Some(crate::protocol::server_frame::Payload::Command(_))
            )
        }) {
            return Err(PushWorkerError::InvalidSuccessorFrame);
        }
        self.deferred_server_frames.extend(commands);
        Ok(self)
    }

    /// 运行 worker，直到取消、输入关闭或 transport 断开。
    ///
    /// 返回仍未 ACK 的完整 frame，供下一连接按原 sequence、原字节恢复。
    pub async fn run(self) -> Vec<PendingFrame> {
        self.run_session().await.pending_frames
    }

    /// 运行一个 transport 会话，并把批次接收端与未 ACK frame 一并交回 supervisor。
    pub async fn run_session(mut self) -> PushWorkerExit {
        let mut batch_input_closed = false;
        while let Some(pending) = self.replay_frames.pop_front() {
            if let Err(error) = self.send_encoded_frame(&pending.encoded_frame).await {
                tracing::warn!(%error, "failed to replay pending metrics frame");
                return self.into_exit();
            }
        }
        if let Err(error) = self.drain_authoritative_batches().await {
            tracing::warn!(%error, "failed to send authoritative full snapshot");
            return self.into_exit();
        }
        if !self.advance_full_snapshot_command().await {
            return self.into_exit();
        }
        if self.reply_confirmation_ready() {
            return self.into_exit();
        }
        if let Err(error) = self.drain_preserved_live_events().await {
            tracing::warn!(%error, "failed to replay post-cutoff live events");
            return self.into_exit();
        }

        loop {
            if batch_input_closed && self.window.is_empty() {
                break;
            }
            let deferred_index = if self.window.is_full() {
                self.deferred_server_frames.iter().position(|frame| {
                    matches!(
                        frame.payload.as_ref(),
                        Some(crate::protocol::server_frame::Payload::ResyncRequired(_))
                            | Some(crate::protocol::server_frame::Payload::Error(
                                crate::protocol::ServerError {
                                    close_stream: true,
                                    ..
                                }
                            ))
                    )
                })
            } else {
                (!self.deferred_server_frames.is_empty()).then_some(0)
            };
            if let Some(index) = deferred_index {
                if let Some(frame) = self.deferred_server_frames.remove(index) {
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
                batch = self.batch_rx.recv(), if !self.window.is_full() && !batch_input_closed => Event::Batch(batch),
            };

            match event {
                Event::Cancelled => break,
                Event::Batch(None) => batch_input_closed = true,
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
                    let retry_event = event.clone();
                    let pending_before = self.window.len();
                    if let Err(error) = self.process_push_event(event).await {
                        if self.window.len() == pending_before {
                            self.preserved_live_events.push_front(retry_event);
                        }
                        tracing::error!(%error, "failed to push metrics frame");
                        break;
                    }
                }
            }
        }

        self.into_exit()
    }

    fn into_exit(self) -> PushWorkerExit {
        let successor_commands = self
            .deferred_server_frames
            .into_iter()
            .filter(|frame| {
                matches!(
                    frame.payload.as_ref(),
                    Some(crate::protocol::server_frame::Payload::Command(_))
                )
            })
            .collect();
        PushWorkerExit {
            pending_frames: self.window.pending_frames(),
            batch_rx: self.batch_rx,
            next_sequence: self.window.next_sequence(),
            authoritative_batches: self.initial_batches.into(),
            resync_required: self.resync_required,
            full_snapshot_command: self.full_snapshot_command,
            completed_full_snapshot_commands: self.completed_full_snapshot_commands.into(),
            reply_sent_this_session: self.reply_sent_this_session,
            preserved_live_events: self.preserved_live_events.into(),
            transport_rx: self.transport_rx,
            successor_commands,
            #[cfg(feature = "client")]
            snapshot_transaction: self.snapshot_transaction,
        }
    }

    async fn process_push_event(&mut self, event: PushEvent) -> Result<(), PushWorkerError> {
        match event {
            PushEvent::Batch(mut batch) => {
                if self.force_first_batch_full_snapshot {
                    batch.is_full_snapshot = true;
                    self.force_first_batch_full_snapshot = false;
                }
                self.push_new_batch(batch, None).await
            }
            PushEvent::BatchAt {
                work_id, mut batch, ..
            } => {
                if self.force_first_batch_full_snapshot {
                    batch.is_full_snapshot = true;
                    self.force_first_batch_full_snapshot = false;
                }
                self.push_new_batch(batch, Some(work_id)).await
            }
            PushEvent::Resend(pending) => self.push_resend(pending).await,
        }
    }

    async fn push_new_batch(
        &mut self,
        batch: AggregatedBatch,
        work_id: Option<BatchWorkId>,
    ) -> Result<(), PushWorkerError> {
        let work_id = match (&self.progress, work_id) {
            (Some(_), Some(work_id)) => Some(work_id),
            (Some(progress), None) => Some(progress.client_front().map_err(ownership_error)?),
            (None, None) => None,
            (None, Some(_)) => {
                return Err(PushWorkerError::Ownership(
                    "tracked batch requires pipeline progress".to_owned(),
                ));
            }
        };
        let is_full_snapshot = batch.is_full_snapshot;
        if is_full_snapshot && self.snapshot_transaction.is_none() {
            self.start_snapshot_transaction(1)?;
        }
        let snapshot_transaction =
            self.snapshot_transaction
                .as_ref()
                .map(
                    |(transaction_id, chunk_index, chunk_count)| SnapshotTransaction {
                        transaction_id: transaction_id.clone(),
                        chunk_index: *chunk_index,
                        chunk_count: *chunk_count,
                        end: chunk_index.saturating_add(1) == *chunk_count,
                    },
                );
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
                        is_full_snapshot,
                        snapshot_transaction,
                    })),
                })
            })
            .map_err(PushWorkerError::Window)?;
        if let (Some(progress), Some(work_id)) = (&self.progress, work_id) {
            progress
                .transfer_queued_batch_to_pending_frames(work_id)
                .map_err(ownership_error)?;
        }
        if is_full_snapshot {
            let Some((_, chunk_index, chunk_count)) = self.snapshot_transaction.as_mut() else {
                return Err(PushWorkerError::InvalidSnapshotTransaction(
                    "full snapshot transaction progress disappeared".to_owned(),
                ));
            };
            *chunk_index = chunk_index.saturating_add(1);
            if *chunk_index == *chunk_count {
                self.snapshot_transaction = None;
            }
        }
        self.send_encoded_frame(&pending.encoded_frame).await
    }

    async fn push_resend(&mut self, pending: PendingFrame) -> Result<(), PushWorkerError> {
        let frame = Self::decode_pending_frame(&pending)?;
        self.window
            .restore(pending)
            .map_err(PushWorkerError::Window)?;
        if let Some(progress) = &self.progress {
            progress
                .verify_pending_frames(self.window.len())
                .map_err(ownership_error)?;
        }
        self.send_outbound(frame).await
    }

    fn decode_pending_frame(pending: &PendingFrame) -> Result<ClientFrame, PushWorkerError> {
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
        Ok(frame)
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
            if let Err(error) = self.acknowledge(ack.accepted_sequence) {
                tracing::warn!(sequence = ack.accepted_sequence, %error, "unexpected ACK");
            }
            return Ok(());
        }
        if self.deferred_server_frames.len() >= self.deferred_server_capacity {
            #[cfg(feature = "client")]
            if let (
                Some(progress),
                Some(crate::protocol::server_frame::Payload::Command(command)),
            ) = (&self.progress, frame.payload.as_ref())
            {
                let key = CommandWorkKey::from_command(command).map_err(ownership_error)?;
                progress
                    .drop_transport_command(&key)
                    .map_err(ownership_error)?;
            }
            return Err(PushWorkerError::ServerInboundOverload {
                capacity: self.deferred_server_capacity,
            });
        }
        self.deferred_server_frames.push_back(frame);
        Ok(())
    }

    fn begin_command_ownership(
        &self,
        command: &crate::protocol::Command,
    ) -> Result<Option<CommandWorkKey>, PushWorkerError> {
        self.progress
            .as_ref()
            .map(|progress| {
                progress
                    .begin_command(command)
                    .map(|(key, _transition)| key)
                    .map_err(ownership_error)
            })
            .transpose()
    }

    fn finish_command_ownership(
        &self,
        key: Option<&CommandWorkKey>,
    ) -> Result<(), PushWorkerError> {
        match (&self.progress, key) {
            (Some(progress), Some(key)) => progress.finish_command(key).map_err(ownership_error),
            (None, None) => Ok(()),
            _ => Err(PushWorkerError::Ownership(
                "command progress and work key must be present together".to_owned(),
            )),
        }
    }

    fn finish_command_after_reply(&self, sent: bool, key: Option<&CommandWorkKey>) -> bool {
        if !sent {
            return false;
        }
        if let Err(error) = self.finish_command_ownership(key) {
            tracing::error!(%error, "failed to finish command ownership");
            return false;
        }
        true
    }

    async fn process_server_frame(&mut self, frame: ServerFrame) -> bool {
        use crate::protocol::server_frame::Payload;

        match frame.payload {
            Some(Payload::BatchAck(ack)) => {
                if let Err(error) = self.acknowledge(ack.accepted_sequence) {
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
                    let key = match self.begin_command_ownership(&command) {
                        Ok(key) => key,
                        Err(error) => {
                            tracing::error!(%error, "failed to start command ownership");
                            return false;
                        }
                    };
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("ACK window capacity is zero".to_owned()),
                    );
                    let sent = self.send_outbound(reply).await.is_ok();
                    return self.finish_command_after_reply(sent, key.as_ref());
                }
                if self.deferred_server_frames.is_empty() {
                    self.deferred_server_frames.push_back(ServerFrame {
                        payload: Some(Payload::Command(command)),
                    });
                    true
                } else {
                    let key = match self.begin_command_ownership(&command) {
                        Ok(key) => key,
                        Err(error) => {
                            tracing::error!(%error, "failed to start command ownership");
                            return false;
                        }
                    };
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("another full snapshot command is waiting for ACK capacity".to_owned()),
                    );
                    let sent = self.send_outbound(reply).await.is_ok();
                    self.finish_command_after_reply(sent, key.as_ref())
                }
            }
            Some(Payload::Command(command))
                if matches!(
                    command.payload.as_ref(),
                    Some(crate::protocol::command::Payload::RequestFullSnapshot(_))
                ) =>
            {
                let key = match self.begin_command_ownership(&command) {
                    Ok(key) => key,
                    Err(error) => {
                        tracing::error!(%error, "failed to start command ownership");
                        return false;
                    }
                };
                self.handle_full_snapshot_command(command, key).await
            }
            Some(Payload::Command(command)) => {
                let key = match self.begin_command_ownership(&command) {
                    Ok(key) => key,
                    Err(error) => {
                        tracing::error!(%error, "failed to start command ownership");
                        return false;
                    }
                };
                let reply = self.handle_command(command);
                let sent = self.send_outbound(reply).await.is_ok();
                self.finish_command_after_reply(sent, key.as_ref())
            }
            Some(Payload::HelloAck(_)) | None => true,
            Some(Payload::ResyncRequired(resync)) => {
                tracing::warn!(
                    reason = resync.reason,
                    expected = resync.expected_sequence,
                    "server requested resync"
                );
                self.resync_required = true;
                false
            }
            Some(Payload::Error(error)) => {
                tracing::error!(code = error.code, message = error.message, "server error");
                !error.close_stream
            }
        }
    }

    fn handle_command(&mut self, command: crate::protocol::Command) -> ClientFrame {
        use crate::protocol::command::Payload;

        let command_id = command.command_id;
        let canonical = canonical_command_payload(&command.payload).map_err(str::to_owned);
        let result = canonical.and_then(|canonical| {
            match self.command_journal.begin(&command_id, canonical)? {
                (false, Some(result)) => result,
                (false, None) => Err("command is already executing".to_owned()),
                (true, None) => {
                    let result = match command.payload {
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
                            Err("full snapshot command must use the durable path".to_owned())
                        }
                        None => Err("empty command payload".to_owned()),
                    };
                    self.command_journal.complete(&command_id, result.clone());
                    result
                }
                (true, Some(_)) => Err("new command unexpectedly had a result".to_owned()),
            }
        });
        self.make_command_ack(command_id, result)
    }

    async fn handle_full_snapshot_command(
        &mut self,
        command: crate::protocol::Command,
        ownership_key: Option<CommandWorkKey>,
    ) -> bool {
        let command_id = command.command_id;
        let canonical = match canonical_command_payload(&command.payload) {
            Ok(canonical) => canonical,
            Err(error) => {
                let reply = self.make_command_ack(command_id, Err(error.to_owned()));
                let sent = self.send_outbound(reply).await.is_ok();
                return self.finish_command_after_reply(sent, ownership_key.as_ref());
            }
        };
        let (execute, existing_result) = match self.command_journal.begin(&command_id, canonical) {
            Ok(journal) => journal,
            Err(error) => {
                let reply = self.make_command_ack(command_id, Err(error));
                let sent = self.send_outbound(reply).await.is_ok();
                return self.finish_command_after_reply(sent, ownership_key.as_ref());
            }
        };
        if let Some(pending) = self.full_snapshot_command.as_ref() {
            if pending.command_id() == command_id {
                // 同一 durable lifecycle 的重投帧只合并；keyed ledger 已消费 replay
                // queued token，但保留原 processing owner，直到有序 Goodbye/EOF 确认。
                return match pending {
                    FullSnapshotCommandState::ReplyReady { .. } => {
                        let reply = self.make_command_ack(command_id, Ok(()));
                        if self.send_outbound(reply).await.is_ok() {
                            self.reply_sent_this_session = true;
                        }
                        false
                    }
                    FullSnapshotCommandState::PendingChunks { .. } => true,
                };
            }
            let reply = self.make_command_ack(
                command_id,
                Err(format!(
                    "full snapshot command {} is still pending",
                    pending.command_id()
                )),
            );
            let sent = self.send_outbound(reply).await.is_ok();
            return self.finish_command_after_reply(sent, ownership_key.as_ref());
        }
        if !execute {
            let result = existing_result
                .unwrap_or_else(|| Err("full snapshot command is already executing".to_owned()));
            let reply = self.make_command_ack(command_id, result);
            let sent = self.send_outbound(reply).await.is_ok();
            return self.finish_command_after_reply(sent, ownership_key.as_ref());
        }
        let batches = self
            .command_executor
            .as_ref()
            .ok_or_else(|| "datasource registry is not configured".to_owned())
            .and_then(|executor| executor.full_snapshot().map_err(|error| error.to_string()));
        let batches = match batches {
            Ok(batches) => batches,
            Err(error) => {
                let result = Err(error);
                self.command_journal.complete(&command_id, result.clone());
                let reply = self.make_command_ack(command_id, result);
                let sent = self.send_outbound(reply).await.is_ok();
                return self.finish_command_after_reply(sent, ownership_key.as_ref());
            }
        };
        if let Some(progress) = &self.progress {
            if let Err(error) = progress.queue_batches_front(batches.len()) {
                tracing::error!(%error, "failed to own authoritative command batches");
                return false;
            }
        }
        self.full_snapshot_command = Some(FullSnapshotCommandState::PendingChunks {
            command_id: command_id.clone(),
        });
        self.initial_batches.extend(batches);
        let advanced = self.advance_full_snapshot_command().await;
        advanced && !self.reply_confirmation_ready()
    }

    async fn advance_full_snapshot_command(&mut self) -> bool {
        let Some(state) = self.full_snapshot_command.clone() else {
            return true;
        };
        let command_id = state.command_id().to_owned();
        if matches!(state, FullSnapshotCommandState::PendingChunks { .. }) {
            if self.drain_authoritative_batches().await.is_err()
                || self.wait_for_pending_frames_empty().await.is_err()
            {
                return false;
            }
            self.full_snapshot_command = Some(FullSnapshotCommandState::ReplyReady {
                command_id: command_id.clone(),
            });
            self.command_journal.complete(&command_id, Ok(()));
        }
        let reply = self.make_command_ack(command_id, Ok(()));
        if self.send_outbound(reply).await.is_err() {
            return false;
        }
        self.reply_sent_this_session = true;
        true
    }

    fn reply_confirmation_ready(&self) -> bool {
        self.reply_sent_this_session
            && matches!(
                self.full_snapshot_command,
                Some(FullSnapshotCommandState::ReplyReady { .. })
            )
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

    async fn drain_authoritative_batches(&mut self) -> Result<(), PushWorkerError> {
        if !self.initial_batches.is_empty() {
            let remaining = self.initial_batches.len();
            if self.snapshot_transaction.is_none() {
                self.start_snapshot_transaction(remaining)?;
            } else if self.snapshot_transaction.as_ref().is_some_and(
                |(_, next_chunk_index, chunk_count)| {
                    chunk_count.saturating_sub(*next_chunk_index) as usize != remaining
                },
            ) {
                return Err(PushWorkerError::InvalidSnapshotTransaction(
                    "restored snapshot transaction does not match remaining chunks".to_owned(),
                ));
            }
        }
        while !self.initial_batches.is_empty() {
            self.wait_for_window_capacity().await?;
            let batch = self
                .initial_batches
                .pop_front()
                .expect("authoritative batch queue was checked as non-empty");
            let retry_batch = batch.clone();
            let pending_before = self.window.len();
            if let Err(error) = self.push_new_batch(batch, None).await {
                if self.window.len() == pending_before {
                    self.initial_batches.push_front(retry_batch);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    fn start_snapshot_transaction(&mut self, chunk_count: usize) -> Result<(), PushWorkerError> {
        let chunk_count = u32::try_from(chunk_count).map_err(|_| {
            PushWorkerError::InvalidSnapshotTransaction(
                "snapshot transaction has too many chunks".to_owned(),
            )
        })?;
        if chunk_count == 0 {
            return Err(PushWorkerError::InvalidSnapshotTransaction(
                "snapshot transaction must have at least one chunk".to_owned(),
            ));
        }
        let first_sequence = self.window.next_sequence().ok_or_else(|| {
            PushWorkerError::InvalidSnapshotTransaction(
                "snapshot transaction cannot allocate a sequence".to_owned(),
            )
        })?;
        self.snapshot_transaction = Some((
            format!("{}-{first_sequence}", self.stream_epoch),
            0,
            chunk_count,
        ));
        Ok(())
    }

    async fn drain_preserved_live_events(&mut self) -> Result<(), PushWorkerError> {
        while !self.preserved_live_events.is_empty() {
            self.wait_for_window_capacity().await?;
            let event = self
                .preserved_live_events
                .pop_front()
                .expect("preserved live queue was checked as non-empty");
            let retry_event = event.clone();
            let pending_before = self.window.len();
            if let Err(error) = self.process_push_event(event).await {
                if self.window.len() == pending_before {
                    self.preserved_live_events.push_front(retry_event);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    async fn wait_for_pending_frames_empty(&mut self) -> Result<(), PushWorkerError> {
        use crate::protocol::server_frame::Payload;

        while !self.window.is_empty() {
            self.wait_for_window_capacity().await?;
            if !self.window.is_empty() {
                let frame = tokio::select! {
                    biased;
                    () = self.cancellation.cancelled() => return Err(PushWorkerError::Cancelled),
                    frame = self.transport_rx.recv() => frame.ok_or(PushWorkerError::TransportClosed)?,
                };
                match frame.payload {
                    Some(Payload::BatchAck(ack)) => {
                        self.acknowledge(ack.accepted_sequence)?;
                    }
                    Some(Payload::ResyncRequired(resync)) => {
                        self.resync_required = true;
                        return Err(PushWorkerError::ServerInterrupted(resync.reason));
                    }
                    Some(Payload::Error(error)) if error.close_stream => {
                        return Err(PushWorkerError::ServerInterrupted(error.message));
                    }
                    Some(Payload::Command(command)) => {
                        let key = self.begin_command_ownership(&command)?;
                        let reply = self.make_command_ack(
                            command.command_id,
                            Err("authoritative full snapshot is waiting for ACKs".to_owned()),
                        );
                        self.send_outbound(reply).await?;
                        self.finish_command_ownership(key.as_ref())?;
                    }
                    Some(Payload::HelloAck(_) | Payload::Error(_)) | None => {}
                }
            }
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
                    self.acknowledge(ack.accepted_sequence)?;
                }
                Some(Payload::Command(command)) => {
                    let key = self.begin_command_ownership(&command)?;
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("full snapshot command is waiting for ACK capacity".to_owned()),
                    );
                    self.send_outbound(reply).await?;
                    self.finish_command_ownership(key.as_ref())?;
                }
                Some(Payload::Error(error)) if error.close_stream => {
                    return Err(PushWorkerError::ServerInterrupted(error.message));
                }
                Some(Payload::ResyncRequired(resync)) => {
                    self.resync_required = true;
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

    fn acknowledge(&mut self, sequence: u64) -> Result<(), PushWorkerError> {
        self.window.ack(sequence).map_err(PushWorkerError::Window)?;
        if let Some(progress) = &self.progress {
            progress.finish_pending_frame().map_err(ownership_error)?;
        }
        Ok(())
    }
}

fn ownership_error(error: PipelineOwnershipError) -> PushWorkerError {
    PushWorkerError::Ownership(error.to_string())
}

/// PushWorker 的窗口、编码或 transport 错误。
#[derive(Debug, thiserror::Error)]
pub enum PushWorkerError {
    /// Pipeline ownership transition violated its keyed lifecycle.
    #[error("pipeline ownership transition failed: {0}")]
    Ownership(String),
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
    /// 跨连接命令队列超过当前有界 transport 容量。
    #[error("successor command queue full: capacity {capacity}")]
    SuccessorCommandOverflow {
        /// 允许迁移的最大命令数。
        capacity: usize,
    },
    /// successor 队列只允许已经校验的远程 Command frame。
    #[error("successor queue contains a non-command server frame")]
    InvalidSuccessorFrame,
    /// 权威快照 transaction 元数据缺失、冲突或无法恢复。
    #[error("invalid snapshot transaction: {0}")]
    InvalidSnapshotTransaction(String),
}

fn validate_snapshot_transaction(transaction: &SnapshotTransaction) -> Result<(), PushWorkerError> {
    let is_last = transaction
        .chunk_index
        .checked_add(1)
        .is_some_and(|next| next == transaction.chunk_count);
    if transaction.transaction_id.trim().is_empty()
        || transaction.chunk_count == 0
        || transaction.chunk_index >= transaction.chunk_count
        || transaction.end != is_last
    {
        return Err(PushWorkerError::InvalidSnapshotTransaction(
            "transaction ID/index/count/end combination is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
