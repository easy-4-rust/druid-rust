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

mod push_worker_a;
mod push_worker_b;

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

    #[cfg(feature = "client")]
    pub(crate) fn registered_change(&self) -> impl std::future::Future<Output = ()> + '_ {
        self.ownership.registered_change()
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
