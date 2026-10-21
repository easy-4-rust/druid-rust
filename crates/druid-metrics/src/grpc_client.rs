//! Tonic 指标客户端与有界重连 supervisor。

use std::fmt;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Channel, Endpoint};

use crate::aggregator::AggregatedBatch;
use crate::command_journal::CommandJournal;
use crate::error::MetricsError;
use crate::pipeline_ownership::CommandWorkKey;
use crate::pipeline_ownership::PipelineOwnership;
use crate::protocol::metrics_ingest_client::MetricsIngestClient;
use crate::protocol::{
    client_frame, server_frame, ClientFrame, ClientGoodbye, ClientHello, FrameHeader, ServerFrame,
};
use crate::push_worker::{
    FullSnapshotCommandState, PushEvent, PushProgress, PushWorker, PushWorkerExit,
};
use crate::registry::RegistryCommandExecutor;
use crate::sequence_window::PendingFrame;

mod transport_helpers;

pub use crate::grpc_authorization::GrpcAuthorization;

const INITIAL_BACKOFF: Duration = Duration::from_millis(250);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// ACK window 与内部 transport channel 的生产默认容量。
pub const DEFAULT_ACK_WINDOW_CAPACITY: usize = 256;

/// 真实 Tonic 客户端配置。
#[derive(Clone)]
pub struct GrpcClientConfig {
    /// Admin gRPC endpoint。
    pub endpoint: Endpoint,
    /// 协议流服务名。
    pub service_name: String,
    /// 当前进程实例 ID。
    pub instance_id: String,
    /// 当前启动 ID。
    pub boot_id: String,
    /// 未 ACK frame 窗口与内部 transport 通道容量。
    pub ack_window_capacity: usize,
    /// 握手、transport 清理与默认关闭期限。
    pub shutdown_timeout: Duration,
    /// 初连与每次重连都必须注入的 Authorization metadata。
    pub authorization: Option<GrpcAuthorization>,
}

impl fmt::Debug for GrpcClientConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrpcClientConfig")
            .field("endpoint", &"[CONFIGURED]")
            .field("service_name", &self.service_name)
            .field("instance_id", &self.instance_id)
            .field("boot_id", &self.boot_id)
            .field("ack_window_capacity", &self.ack_window_capacity)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("authorization", &self.authorization)
            .finish()
    }
}

impl GrpcClientConfig {
    /// 创建默认 ACK window 为 256 的验证 builder。
    pub fn builder(
        endpoint: Endpoint,
        service_name: impl Into<String>,
        instance_id: impl Into<String>,
        boot_id: impl Into<String>,
    ) -> crate::grpc_client_config_builder::GrpcClientConfigBuilder {
        crate::grpc_client_config_builder::GrpcClientConfigBuilder::new(
            endpoint,
            service_name,
            instance_id,
            boot_id,
        )
    }

    /// 在任何 endpoint connect 或 Authorization 注入前校验安全与容量边界。
    pub fn validate(&self) -> Result<(), MetricsError> {
        validate_config(self)
    }
}

/// 持有推送、transport、重连和取消生命周期的真实 gRPC 指标客户端。
pub struct GrpcMetricsClient {
    batch_tx: Option<mpsc::Sender<PushEvent>>,
    cancellation: CancellationToken,
    supervisor_handle: Option<JoinHandle<Result<(), MetricsError>>>,
    progress: PushProgress,
    generation: Arc<AtomicU64>,
    shutdown_timeout: Duration,
}

impl GrpcMetricsClient {
    /// 建立首个真实 Tonic bidi 连接并启动受监管推送任务。
    pub async fn connect(config: GrpcClientConfig) -> Result<Self, MetricsError> {
        Self::connect_with_command_executor(config, None).await
    }

    /// 建立连接，并向 PushWorker 注入现有 datasource registry 命令执行器。
    pub async fn connect_with_command_executor(
        config: GrpcClientConfig,
        command_executor: Option<RegistryCommandExecutor>,
    ) -> Result<Self, MetricsError> {
        Self::connect_inner(config, command_executor, Arc::new(PipelineOwnership::new())).await
    }

    pub(crate) async fn connect_with_command_executor_and_ownership(
        config: GrpcClientConfig,
        command_executor: Option<RegistryCommandExecutor>,
        ownership: Arc<PipelineOwnership>,
    ) -> Result<Self, MetricsError> {
        Self::connect_inner(config, command_executor, ownership).await
    }

    async fn connect_inner(
        config: GrpcClientConfig,
        command_executor: Option<RegistryCommandExecutor>,
        ownership: Arc<PipelineOwnership>,
    ) -> Result<Self, MetricsError> {
        validate_config(&config)?;
        let stream_epoch = next_stream_epoch();
        let initial_transport = connect_transport(&config, stream_epoch, None, 0).await?;
        let (batch_tx, batch_rx) = mpsc::channel(config.ack_window_capacity);
        let cancellation = CancellationToken::new();
        let supervisor_cancel = cancellation.clone();
        let supervisor_config = config.clone();
        let progress = PushProgress::with_ownership(ownership);
        let generation = Arc::new(AtomicU64::new(0));
        let supervisor_progress = progress.clone();
        let supervisor_generation = Arc::clone(&generation);
        let command_journal = Arc::new(CommandJournal::new(config.ack_window_capacity));
        let supervisor_handle = tokio::spawn(async move {
            run_supervisor(
                supervisor_config,
                stream_epoch,
                batch_rx,
                initial_transport,
                command_executor,
                supervisor_cancel,
                supervisor_progress,
                supervisor_generation,
                command_journal,
            )
            .await
        });

        Ok(Self {
            batch_tx: Some(batch_tx),
            cancellation,
            supervisor_handle: Some(supervisor_handle),
            progress,
            generation,
            shutdown_timeout: config.shutdown_timeout,
        })
    }

    /// 将一个已经类型化、隐私处理完成的聚合批次提交给有界推送通道。
    pub async fn send_batch(&self, batch: AggregatedBatch) -> Result<(), MetricsError> {
        let batch_tx = self
            .batch_tx
            .as_ref()
            .ok_or_else(|| MetricsError::ChannelSend("gRPC client is shutting down".to_owned()))?;
        let permit = batch_tx
            .reserve()
            .await
            .map_err(|_| MetricsError::ChannelSend("gRPC push worker stopped".to_owned()))?;
        let work_id = self.progress.queue_batch().map_err(other_error)?;
        let generation = self.generation.load(Ordering::Acquire);
        permit.send(PushEvent::BatchAt {
            work_id,
            generation,
            batch,
        });
        Ok(())
    }

    /// 从 runtime exporter 原子接管一个 batch，并提交给有界客户端队列。
    pub(crate) async fn send_batch_from_exporter(
        &self,
        batch: AggregatedBatch,
    ) -> Result<(), MetricsError> {
        let batch_tx = self
            .batch_tx
            .as_ref()
            .ok_or_else(|| MetricsError::ChannelSend("gRPC client is shutting down".to_owned()))?;
        let permit = batch_tx
            .reserve()
            .await
            .map_err(|_| MetricsError::ChannelSend("gRPC push worker stopped".to_owned()))?;
        let work_id = self
            .progress
            .transfer_exporter_batch_to_client()
            .map_err(other_error)?;
        let generation = self.generation.load(Ordering::Acquire);
        permit.send(PushEvent::BatchAt {
            work_id,
            generation,
            batch,
        });
        Ok(())
    }

    /// 等待当前已排队批次全部编码并获得服务端 ACK。
    pub async fn flush(&self, deadline: Duration) -> Result<(), MetricsError> {
        let deadline = tokio::time::Instant::now() + deadline;
        loop {
            let unflushed = self.progress.unflushed();
            if unflushed == 0 {
                return Ok(());
            }
            let changed = self.progress.registered_change();
            if self.progress.unflushed() == 0 {
                return Ok(());
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                return Err(MetricsError::ShutdownTimeout {
                    unflushed: self.progress.unflushed(),
                });
            }
        }
    }

    /// 返回 runtime shutdown 汇总使用的共享推送进度。
    #[must_use]
    pub fn progress(&self) -> PushProgress {
        self.progress.clone()
    }

    /// 停止接收新批次，等待所有已发送 frame 获得 ACK，并 join 全部自有任务。
    pub async fn shutdown(mut self, deadline: Duration) -> Result<(), MetricsError> {
        self.batch_tx.take();
        let Some(mut supervisor_handle) = self.supervisor_handle.take() else {
            return Ok(());
        };
        let effective_deadline = deadline.min(self.shutdown_timeout);
        if let Ok(result) = tokio::time::timeout(effective_deadline, &mut supervisor_handle).await {
            flatten_join(result)
        } else {
            self.cancellation.cancel();
            let unflushed = self.progress.unflushed();
            supervisor_handle.abort();
            let _ = supervisor_handle.await;
            Err(MetricsError::ShutdownTimeout { unflushed })
        }
    }
}

impl Drop for GrpcMetricsClient {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(handle) = self.supervisor_handle.take() {
            handle.abort();
        }
    }
}

struct ConnectedTransport {
    outbound_tx: mpsc::Sender<ClientFrame>,
    inbound: tonic::Streaming<ServerFrame>,
    require_full_snapshot: bool,
}

#[allow(clippy::too_many_arguments)]
async fn run_supervisor(
    config: GrpcClientConfig,
    stream_epoch: u64,
    mut batch_rx: mpsc::Receiver<PushEvent>,
    mut transport: ConnectedTransport,
    command_executor: Option<RegistryCommandExecutor>,
    cancellation: CancellationToken,
    progress: PushProgress,
    generation: Arc<AtomicU64>,
    command_journal: Arc<CommandJournal>,
) -> Result<(), MetricsError> {
    let mut pending_frames = Vec::new();
    let mut next_sequence = Some(1);
    let mut backoff = INITIAL_BACKOFF;
    let mut completed_session = false;
    let mut authoritative_batches = Vec::new();
    let mut resync_required = false;
    let mut full_snapshot_command = None;
    let mut completed_full_snapshot_commands = Vec::new();
    let mut preserved_live_events = Vec::new();
    let mut successor_commands = Vec::new();
    let mut snapshot_transaction = None;

    loop {
        if (transport.require_full_snapshot && completed_session) || resync_required {
            let cutoff_generation = generation
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.checked_add(1)
                })
                .map_err(|_| {
                    MetricsError::ChannelSend("resync generation space exhausted".to_owned())
                })?
                + 1;
            let executor = command_executor.as_ref().ok_or_else(|| {
                MetricsError::ChannelSend(
                    "server requires a registry-derived full snapshot, but no registry is configured"
                        .to_owned(),
                )
            })?;
            for _ in authoritative_batches.drain(..) {
                let id = progress.client_front().map_err(other_error)?;
                progress.drop_client_batch(id).map_err(other_error)?;
            }
            for event in preserved_live_events.drain(..) {
                if let PushEvent::BatchAt { work_id, .. } = event {
                    progress.drop_client_batch(work_id).map_err(other_error)?;
                }
            }
            authoritative_batches = executor.full_snapshot().map_err(other_error)?;
            snapshot_transaction = None;
            progress
                .discard_pending_frames(pending_frames.len())
                .map_err(other_error)?;
            pending_frames.clear();
            while let Ok(event) = batch_rx.try_recv() {
                match event {
                    PushEvent::BatchAt {
                        work_id,
                        generation,
                        batch,
                    } if generation >= cutoff_generation => {
                        preserved_live_events.push(PushEvent::BatchAt {
                            work_id,
                            generation,
                            batch,
                        });
                    }
                    PushEvent::BatchAt { work_id, .. } => {
                        progress.drop_client_batch(work_id).map_err(other_error)?;
                    }
                    PushEvent::Batch(_) | PushEvent::Resend(_) => {}
                }
            }
            progress
                .queue_batches_front(authoritative_batches.len())
                .map_err(other_error)?;
        }
        let exit = run_connected_session(
            &config,
            stream_epoch,
            batch_rx,
            pending_frames,
            next_sequence,
            transport,
            command_executor.clone(),
            cancellation.clone(),
            progress.clone(),
            authoritative_batches,
            full_snapshot_command,
            completed_full_snapshot_commands,
            preserved_live_events,
            successor_commands,
            snapshot_transaction,
            Arc::clone(&command_journal),
        )
        .await?;
        batch_rx = exit.batch_rx;
        pending_frames = exit.pending_frames;
        next_sequence = exit.next_sequence;
        authoritative_batches = exit.authoritative_batches;
        resync_required = exit.resync_required;
        full_snapshot_command = exit.full_snapshot_command;
        completed_full_snapshot_commands = exit.completed_full_snapshot_commands;
        preserved_live_events = exit.preserved_live_events;
        successor_commands = exit.successor_commands;
        snapshot_transaction = exit.snapshot_transaction;
        completed_session = true;

        if batch_rx.is_closed()
            && batch_rx.is_empty()
            && pending_frames.is_empty()
            && authoritative_batches.is_empty()
            && !resync_required
            && full_snapshot_command.is_none()
            && preserved_live_events.is_empty()
            && successor_commands.is_empty()
        {
            return Ok(());
        }
        if cancellation.is_cancelled() {
            return Err(MetricsError::ShutdownTimeout {
                unflushed: progress.unflushed(),
            });
        }

        loop {
            wait_for_reconnect(backoff, &cancellation).await?;
            match connect_transport(
                &config,
                stream_epoch,
                Some(&cancellation),
                progress.unflushed(),
            )
            .await
            {
                Ok(next_transport) => {
                    transport = next_transport;
                    backoff = INITIAL_BACKOFF;
                    break;
                }
                Err(error) if cancellation.is_cancelled() => return Err(error),
                Err(error) => {
                    tracing::warn!(%error, "gRPC metrics transport setup failed; retrying");
                }
            }
            backoff = next_backoff(backoff);
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_connected_session(
    config: &GrpcClientConfig,
    stream_epoch: u64,
    batch_rx: mpsc::Receiver<PushEvent>,
    pending_frames: Vec<PendingFrame>,
    next_sequence: Option<u64>,
    transport: ConnectedTransport,
    command_executor: Option<RegistryCommandExecutor>,
    cancellation: CancellationToken,
    progress: PushProgress,
    authoritative_batches: Vec<AggregatedBatch>,
    full_snapshot_command: Option<FullSnapshotCommandState>,
    completed_full_snapshot_commands: Vec<String>,
    preserved_live_events: Vec<PushEvent>,
    successor_commands: Vec<ServerFrame>,
    snapshot_transaction: Option<(String, u32, u32)>,
    command_journal: Arc<CommandJournal>,
) -> Result<PushWorkerExit, MetricsError> {
    let ConnectedTransport {
        outbound_tx,
        mut inbound,
        require_full_snapshot,
    } = transport;
    let goodbye_tx = outbound_tx.clone();
    let (server_tx, server_rx) = mpsc::channel(config.ack_window_capacity);
    let reader_cancellation = CancellationToken::new();
    let reader_task_cancellation = reader_cancellation.clone();
    let reader_config = config.clone();
    let reader_progress = progress.clone();
    let mut reader_handle = AbortOnDrop::new(tokio::spawn(async move {
        loop {
            let message = tokio::select! {
                biased;
                () = reader_task_cancellation.cancelled() => return Ok(None),
                result = inbound.message() => result,
            };
            match message {
                Ok(Some(frame)) => {
                    validate_server_frame(&frame, &reader_config, stream_epoch)?;
                    let command_key = if let Some(server_frame::Payload::Command(command)) =
                        frame.payload.as_ref()
                    {
                        Some(
                            reader_progress
                                .queue_transport_command(command)
                                .map_err(other_error)?,
                        )
                    } else {
                        None
                    };
                    let permit = tokio::select! {
                        biased;
                        () = reader_task_cancellation.cancelled() => return Ok(Some(frame)),
                        result = server_tx.reserve() => result,
                    };
                    let Ok(permit) = permit else {
                        if let Some(key) = command_key.as_ref() {
                            reader_progress
                                .drop_transport_command(key)
                                .map_err(other_error)?;
                        }
                        return Ok(None);
                    };
                    permit.send(frame);
                }
                Ok(None) => return Ok(None),
                Err(error) => {
                    return Err(other_error(error));
                }
            }
        }
    }));

    let force_first_full = require_full_snapshot && authoritative_batches.is_empty();
    let session_progress = progress.clone();
    let mut worker = PushWorker::with_identity(
        batch_rx,
        outbound_tx,
        server_rx,
        config.ack_window_capacity,
        config.service_name.clone(),
        config.instance_id.clone(),
        config.boot_id.clone(),
        stream_epoch,
    )
    .with_cancellation_token(cancellation)
    .with_next_sequence(next_sequence)
    .with_pending_frames(pending_frames)
    .map_err(other_error)?
    .with_progress(progress)
    .map_err(other_error)?
    .with_initial_batches(authoritative_batches)
    .with_snapshot_transaction(snapshot_transaction)
    .with_command_journal(command_journal)
    .with_first_batch_full_snapshot(force_first_full)
    .with_full_snapshot_command_state(full_snapshot_command, completed_full_snapshot_commands);
    worker = worker.with_preserved_live_events(preserved_live_events);
    worker = worker
        .with_successor_commands(successor_commands)
        .map_err(other_error)?;
    if let Some(executor) = command_executor {
        worker = worker.with_command_executor(executor);
    }
    let mut exit = worker.run_session().await;

    let transport_clean = exit.pending_frames.is_empty()
        && exit.authoritative_batches.is_empty()
        && exit.preserved_live_events.is_empty()
        && !exit.resync_required;
    let reply_confirmation = transport_clean
        && exit.reply_sent_this_session
        && matches!(
            exit.full_snapshot_command,
            Some(FullSnapshotCommandState::ReplyReady { .. })
        );
    let clean_shutdown = transport_clean
        && exit.batch_rx.is_closed()
        && exit.batch_rx.is_empty()
        && exit.full_snapshot_command.is_none();
    let goodbye_required = clean_shutdown || reply_confirmation;
    let reader_closed_before_goodbye = goodbye_required && reader_handle.is_finished();
    let goodbye_result = if goodbye_required && !reader_closed_before_goodbye {
        goodbye_tx
            .send(ClientFrame {
                payload: Some(client_frame::Payload::Goodbye(ClientGoodbye {
                    header: Some(make_header(config, stream_epoch, 0)),
                    reason: "client_shutdown".to_owned(),
                })),
            })
            .await
            .map_err(|_| MetricsError::ChannelSend("failed to send ClientGoodbye".to_owned()))
    } else {
        Ok(())
    };
    drop(goodbye_tx);
    let reader_result = join_reader_and_collect_successor_commands(
        &mut reader_handle,
        &reader_cancellation,
        &mut exit,
        &session_progress,
        config.shutdown_timeout,
        config.ack_window_capacity.saturating_mul(2).max(1),
    )
    .await?;
    if reply_confirmation {
        if reader_closed_before_goodbye || goodbye_result.is_err() || reader_result.is_err() {
            tracing::warn!(
                "command reply was not confirmed by an ordered ClientGoodbye response close"
            );
            return Ok(exit);
        }
        let command_id = exit
            .full_snapshot_command
            .as_ref()
            .map(FullSnapshotCommandState::command_id)
            .ok_or_else(|| {
                MetricsError::ChannelSend("full snapshot command ownership is missing".to_owned())
            })?
            .to_owned();
        let key = CommandWorkKey::from_parts(
            &config.service_name,
            &config.instance_id,
            &config.boot_id,
            &command_id,
            &Some(crate::protocol::command::Payload::RequestFullSnapshot(
                crate::protocol::RequestFullSnapshotCmd {},
            )),
        )
        .map_err(other_error)?;
        exit.confirm_full_snapshot_command_reply(config.ack_window_capacity);
        session_progress.finish_command(&key).map_err(other_error)?;
    } else if clean_shutdown {
        if reader_closed_before_goodbye {
            return Err(MetricsError::ChannelSend(
                "server response stream closed before ClientGoodbye".to_owned(),
            ));
        }
        goodbye_result?;
        reader_result?;
    } else if let Err(error) = reader_result {
        tracing::warn!(%error, "gRPC metrics transport replaced after reader failure");
    }
    Ok(exit)
}
