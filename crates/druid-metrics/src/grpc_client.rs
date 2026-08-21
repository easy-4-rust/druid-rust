//! Tonic 指标客户端与有界重连 supervisor。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Channel, Endpoint};

use crate::aggregator::AggregatedBatch;
use crate::error::MetricsError;
use crate::protocol::metrics_ingest_client::MetricsIngestClient;
use crate::protocol::{
    client_frame, server_frame, ClientFrame, ClientGoodbye, ClientHello, FrameHeader, ServerFrame,
};
use crate::push_worker::{PushEvent, PushProgress, PushWorker, PushWorkerExit};
use crate::registry::RegistryCommandExecutor;
use crate::sequence_window::PendingFrame;

const INITIAL_BACKOFF: Duration = Duration::from_millis(250);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// 真实 Tonic 客户端配置。
#[derive(Debug, Clone)]
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
}

/// 持有推送、transport、重连和取消生命周期的真实 gRPC 指标客户端。
pub struct GrpcMetricsClient {
    batch_tx: Option<mpsc::Sender<PushEvent>>,
    cancellation: CancellationToken,
    supervisor_handle: Option<JoinHandle<Result<(), MetricsError>>>,
    progress: PushProgress,
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
        validate_config(&config)?;
        let stream_epoch = next_stream_epoch();
        let initial_channel = config
            .endpoint
            .clone()
            .connect()
            .await
            .map_err(other_error)?;
        let initial_transport = open_transport(initial_channel, &config, stream_epoch).await?;
        let (batch_tx, batch_rx) = mpsc::channel(config.ack_window_capacity);
        let cancellation = CancellationToken::new();
        let supervisor_cancel = cancellation.clone();
        let supervisor_config = config.clone();
        let progress = PushProgress::new();
        let supervisor_progress = progress.clone();
        let supervisor_handle = tokio::spawn(async move {
            run_supervisor(
                supervisor_config,
                stream_epoch,
                batch_rx,
                initial_transport,
                command_executor,
                supervisor_cancel,
                supervisor_progress,
            )
            .await
        });

        Ok(Self {
            batch_tx: Some(batch_tx),
            cancellation,
            supervisor_handle: Some(supervisor_handle),
            progress,
            shutdown_timeout: config.shutdown_timeout,
        })
    }

    /// 将一个已经类型化、隐私处理完成的聚合批次提交给有界推送通道。
    pub async fn send_batch(&self, batch: AggregatedBatch) -> Result<(), MetricsError> {
        let batch_tx = self
            .batch_tx
            .as_ref()
            .ok_or_else(|| MetricsError::ChannelSend("gRPC client is shutting down".to_owned()))?;
        self.progress.queue_batch();
        if batch_tx.send(PushEvent::Batch(batch)).await.is_err() {
            self.progress.undo_queue_batch();
            return Err(MetricsError::ChannelSend(
                "gRPC push worker stopped".to_owned(),
            ));
        }
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
            let changed = self.progress.changed();
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
) -> Result<(), MetricsError> {
    let mut pending_frames = Vec::new();
    let mut next_sequence = Some(1);
    let mut backoff = INITIAL_BACKOFF;
    let mut completed_session = false;
    let mut authoritative_batches = Vec::new();

    loop {
        if transport.require_full_snapshot && completed_session {
            let executor = command_executor.as_ref().ok_or_else(|| {
                MetricsError::ChannelSend(
                    "server requires a registry-derived full snapshot, but no registry is configured"
                        .to_owned(),
                )
            })?;
            for _ in authoritative_batches.drain(..) {
                progress.undo_queue_batch();
            }
            authoritative_batches = executor.full_snapshot().map_err(other_error)?;
            pending_frames.clear();
            progress.set_pending_frames(0);
            while let Ok(event) = batch_rx.try_recv() {
                if matches!(event, PushEvent::Batch(_)) {
                    progress.undo_queue_batch();
                }
            }
            for _ in &authoritative_batches {
                progress.queue_batch();
            }
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
        )
        .await?;
        batch_rx = exit.batch_rx;
        pending_frames = exit.pending_frames;
        next_sequence = exit.next_sequence;
        authoritative_batches = exit.authoritative_batches;
        completed_session = true;

        if batch_rx.is_closed() && batch_rx.is_empty() && pending_frames.is_empty() {
            return Ok(());
        }
        if cancellation.is_cancelled() {
            return Err(MetricsError::ShutdownTimeout {
                unflushed: pending_frames.len().saturating_add(batch_rx.len()),
            });
        }

        loop {
            wait_for_reconnect(backoff, &cancellation).await?;
            match config.endpoint.clone().connect().await {
                Ok(channel) => match open_transport(channel, &config, stream_epoch).await {
                    Ok(next_transport) => {
                        transport = next_transport;
                        backoff = INITIAL_BACKOFF;
                        break;
                    }
                    Err(error) => {
                        tracing::warn!(%error, "gRPC metrics handshake failed; retrying");
                    }
                },
                Err(error) => {
                    tracing::warn!(%error, "gRPC metrics reconnect failed; retrying");
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
) -> Result<PushWorkerExit, MetricsError> {
    let ConnectedTransport {
        outbound_tx,
        mut inbound,
        require_full_snapshot,
    } = transport;
    let goodbye_tx = outbound_tx.clone();
    let (server_tx, server_rx) = mpsc::channel(config.ack_window_capacity);
    let reader_config = config.clone();
    let mut reader_handle = AbortOnDrop::new(tokio::spawn(async move {
        loop {
            match inbound.message().await {
                Ok(Some(frame)) => {
                    validate_server_frame(&frame, &reader_config, stream_epoch)?;
                    if server_tx.send(frame).await.is_err() {
                        return Ok(());
                    }
                }
                Ok(None) => return Ok(()),
                Err(error) => {
                    return Err(other_error(error));
                }
            }
        }
    }));

    let force_first_full = require_full_snapshot && authoritative_batches.is_empty();
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
    .with_initial_batches(authoritative_batches)
    .with_first_batch_full_snapshot(force_first_full);
    if let Some(executor) = command_executor {
        worker = worker.with_command_executor(executor);
    }
    let exit = worker.run_session().await;

    let clean_shutdown =
        exit.pending_frames.is_empty() && exit.batch_rx.is_closed() && exit.batch_rx.is_empty();
    let reader_closed_before_goodbye = clean_shutdown && reader_handle.is_finished();
    let goodbye_result = if clean_shutdown {
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
    let reader_result = join_reader(&mut reader_handle, config.shutdown_timeout).await;
    if clean_shutdown {
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

async fn open_transport(
    channel: Channel,
    config: &GrpcClientConfig,
    stream_epoch: u64,
) -> Result<ConnectedTransport, MetricsError> {
    let mut client = MetricsIngestClient::new(channel);
    let (outbound_tx, outbound_rx) = mpsc::channel(config.ack_window_capacity);
    let response = client
        .connect(ReceiverStream::new(outbound_rx))
        .await
        .map_err(other_error)?;
    let mut inbound = response.into_inner();
    let hello_header = make_header(config, stream_epoch, 0);
    outbound_tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::Hello(ClientHello {
                header: Some(hello_header.clone()),
                supported_protocol_version: 1,
                full_snapshot_requested: false,
            })),
        })
        .await
        .map_err(|_| MetricsError::ChannelSend("gRPC hello transport closed".to_owned()))?;
    let hello = tokio::time::timeout(config.shutdown_timeout, inbound.message())
        .await
        .map_err(|_| MetricsError::ShutdownTimeout { unflushed: 0 })?
        .map_err(other_error)?
        .ok_or_else(|| MetricsError::ChannelSend("gRPC hello response stream closed".to_owned()))?;
    let require_full_snapshot = match hello.payload {
        Some(server_frame::Payload::HelloAck(ack)) => {
            let response_header = ack.header.as_ref().ok_or_else(|| {
                MetricsError::ChannelSend("HelloAck is missing its header".to_owned())
            })?;
            if ack.server_protocol_version != 1
                || response_header.protocol_version != 1
                || !same_stream_identity(&hello_header, response_header)
            {
                return Err(MetricsError::ChannelSend(
                    "HelloAck stream identity or protocol version does not match ClientHello"
                        .to_owned(),
                ));
            }
            ack.require_full_snapshot
        }
        other => {
            return Err(MetricsError::ChannelSend(format!(
                "expected HelloAck, received {other:?}"
            )));
        }
    };
    Ok(ConnectedTransport {
        outbound_tx,
        inbound,
        require_full_snapshot,
    })
}

struct AbortOnDrop(Option<JoinHandle<Result<(), MetricsError>>>);

impl AbortOnDrop {
    fn new(handle: JoinHandle<Result<(), MetricsError>>) -> Self {
        Self(Some(handle))
    }

    fn take(&mut self) -> Option<JoinHandle<Result<(), MetricsError>>> {
        self.0.take()
    }

    fn is_finished(&self) -> bool {
        self.0.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

async fn join_reader(reader_task: &mut AbortOnDrop, timeout: Duration) -> Result<(), MetricsError> {
    let Some(mut reader_handle) = reader_task.take() else {
        return Ok(());
    };
    match tokio::time::timeout(timeout, &mut reader_handle).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(MetricsError::Join(error)),
        Err(_) => {
            reader_handle.abort();
            let _ = reader_handle.await;
            Err(MetricsError::ShutdownTimeout { unflushed: 0 })
        }
    }
}

async fn wait_for_reconnect(
    backoff: Duration,
    cancellation: &CancellationToken,
) -> Result<(), MetricsError> {
    tokio::select! {
        () = cancellation.cancelled() => Err(MetricsError::ShutdownTimeout { unflushed: 1 }),
        () = tokio::time::sleep(jittered(backoff)) => Ok(()),
    }
}

fn validate_config(config: &GrpcClientConfig) -> Result<(), MetricsError> {
    if config.ack_window_capacity == 0 {
        return Err(MetricsError::ChannelSend(
            "gRPC ACK window capacity must be greater than zero".to_owned(),
        ));
    }
    if config.shutdown_timeout.is_zero() {
        return Err(MetricsError::ChannelSend(
            "gRPC shutdown timeout must be non-zero".to_owned(),
        ));
    }
    if config.service_name.trim().is_empty()
        || config.instance_id.trim().is_empty()
        || config.boot_id.trim().is_empty()
    {
        return Err(MetricsError::ChannelSend(
            "gRPC stream identity fields must be non-empty".to_owned(),
        ));
    }
    Ok(())
}

fn flatten_join(
    result: Result<Result<(), MetricsError>, tokio::task::JoinError>,
) -> Result<(), MetricsError> {
    match result {
        Ok(result) => result,
        Err(error) => Err(MetricsError::Join(error)),
    }
}

fn make_header(config: &GrpcClientConfig, stream_epoch: u64, sequence: u64) -> FrameHeader {
    FrameHeader {
        protocol_version: 1,
        service_name: config.service_name.clone(),
        instance_id: config.instance_id.clone(),
        boot_id: config.boot_id.clone(),
        stream_epoch,
        sequence,
        emitted_at_unix_ms: now_unix_ms(),
    }
}

fn same_stream_identity(expected: &FrameHeader, actual: &FrameHeader) -> bool {
    expected.service_name == actual.service_name
        && expected.instance_id == actual.instance_id
        && expected.boot_id == actual.boot_id
        && expected.stream_epoch == actual.stream_epoch
}

fn validate_server_frame(
    frame: &ServerFrame,
    config: &GrpcClientConfig,
    stream_epoch: u64,
) -> Result<(), MetricsError> {
    let payload = frame
        .payload
        .as_ref()
        .ok_or_else(|| MetricsError::ChannelSend("server frame payload is missing".to_owned()))?;
    let header = match payload {
        server_frame::Payload::HelloAck(value) => {
            if value.server_protocol_version != 1 {
                return Err(MetricsError::ChannelSend(format!(
                    "unsupported server protocol version {}",
                    value.server_protocol_version
                )));
            }
            value.header.as_ref()
        }
        server_frame::Payload::BatchAck(value) => value.header.as_ref(),
        server_frame::Payload::ResyncRequired(value) => value.header.as_ref(),
        server_frame::Payload::Command(value) => value.header.as_ref(),
        server_frame::Payload::Error(value) => value.header.as_ref(),
    }
    .ok_or_else(|| MetricsError::ChannelSend("server frame header is missing".to_owned()))?;
    let expected = make_header(config, stream_epoch, 0);
    if header.protocol_version != 1 || !same_stream_identity(&expected, header) {
        return Err(MetricsError::ChannelSend(
            "server frame protocol version or stream identity does not match the client stream"
                .to_owned(),
        ));
    }
    Ok(())
}

fn next_stream_epoch() -> u64 {
    static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);
    now_unix_ms()
        .saturating_mul(1_000)
        .saturating_add(NEXT_EPOCH.fetch_add(1, Ordering::Relaxed))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn next_backoff(current: Duration) -> Duration {
    current.saturating_mul(2).min(MAX_BACKOFF)
}

fn jittered(base: Duration) -> Duration {
    static JITTER: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);
    let mut value = JITTER.fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed);
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;
    let percent = 100_u128 + u128::from(value % 26);
    let nanos = base.as_nanos().saturating_mul(percent) / 100;
    Duration::from_nanos(nanos.min(MAX_BACKOFF.as_nanos()) as u64)
}

fn other_error(error: impl std::error::Error + Send + Sync + 'static) -> MetricsError {
    MetricsError::Other(Box::new(error))
}
