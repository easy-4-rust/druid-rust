//! 共享 druid-metrics 协议的 gRPC ingest 服务。

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use druid_metrics::ingest_handler::IngestHandler;
use druid_metrics::protocol::metrics_ingest_server::{MetricsIngest, MetricsIngestServer};
use druid_metrics::protocol::{
    client_frame, server_frame, ClientFrame, FrameHeader, ServerError, ServerFrame, SnapshotBatch,
    SnapshotEntry,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status, Streaming};

use crate::model::dto::{DataSourceContent, SqlListContent, WallResult};
use crate::repository::{DataSourceEntry, DataSourceId, MetricsRepository};
use crate::CommandBroker;

use super::{ActiveIngestStreamGuard, OwnedConnectStream};

const RESPONSE_CAPACITY: usize = 64;
const PROTOCOL_VERSION: u32 = 1;

/// gRPC ingest 服务实现。
#[derive(Clone)]
pub struct IngestService {
    repo: MetricsRepository,
    handler: Arc<IngestHandler>,
    shutdown: CancellationToken,
    command_broker: CommandBroker,
    active_streams: Arc<AtomicUsize>,
}

impl IngestService {
    /// 使用指定指标仓库创建 ingest 服务。
    #[must_use]
    pub fn new(repo: MetricsRepository) -> Self {
        let handler = IngestHandler::new()
            .with_limits(repo.max_services(), repo.max_datasources_per_service());
        Self {
            repo,
            handler: Arc::new(handler),
            shutdown: CancellationToken::new(),
            command_broker: CommandBroker::new(
                RESPONSE_CAPACITY,
                std::time::Duration::from_secs(10),
            ),
            active_streams: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// 注入 Admin server supervisor 的连接级关闭令牌。
    #[must_use]
    pub fn with_shutdown_token(mut self, shutdown: CancellationToken) -> Self {
        self.shutdown = shutdown;
        self
    }

    /// 注入与 authenticated HTTP command entry 共用的生产 broker。
    #[must_use]
    pub fn with_command_broker(mut self, command_broker: CommandBroker) -> Self {
        self.command_broker = command_broker;
        self
    }

    /// 返回底层指标仓库引用。
    pub fn repository(&self) -> &MetricsRepository {
        &self.repo
    }

    /// 返回当前仍由 response stream 拥有的处理任务数。
    #[must_use]
    pub fn active_streams(&self) -> usize {
        self.active_streams.load(Ordering::Acquire)
    }

    /// 将当前服务包装为 Tonic [`MetricsIngestServer`]。
    #[must_use]
    pub fn into_server(self) -> MetricsIngestServer<Self> {
        MetricsIngestServer::new(self)
    }
}

#[tonic::async_trait]
impl MetricsIngest for IngestService {
    type ConnectStream = OwnedConnectStream;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let mut inbound = request.into_inner();
        let (response_tx, response_rx) = mpsc::channel(RESPONSE_CAPACITY);
        let handler = Arc::clone(&self.handler);
        let repository = self.repo.clone();
        let shutdown = self.shutdown.clone();
        let command_broker = self.command_broker.clone();
        let connection_cancel = CancellationToken::new();
        let stream_cancel = connection_cancel.clone();
        let active_streams = Arc::clone(&self.active_streams);

        let task = tokio::spawn(async move {
            let _active_guard = ActiveIngestStreamGuard::new(active_streams);
            let mut stream_identity = None;
            let mut attached_header = None;
            loop {
                let inbound_frame = tokio::select! {
                    biased;
                    () = shutdown.cancelled() => break,
                    () = stream_cancel.cancelled() => break,
                    frame = inbound.message() => frame,
                };
                let frame = match inbound_frame {
                    Ok(Some(frame)) => frame,
                    Ok(None) => break,
                    Err(error) => {
                        let _ = send_response(&response_tx, Err(error), &shutdown, &stream_cancel)
                            .await;
                        break;
                    }
                };
                let Some(header) = frame_header(&frame).cloned() else {
                    let _ = send_protocol_error(
                        &response_tx,
                        None,
                        400,
                        "client frame is missing a header",
                        true,
                        &shutdown,
                        &stream_cancel,
                    )
                    .await;
                    break;
                };
                if header.protocol_version != PROTOCOL_VERSION {
                    let _ = send_protocol_error(
                        &response_tx,
                        Some(&header),
                        426,
                        "unsupported protocol version",
                        true,
                        &shutdown,
                        &stream_cancel,
                    )
                    .await;
                    break;
                }

                match frame.payload.as_ref() {
                    Some(client_frame::Payload::Hello(hello)) => {
                        if stream_identity.is_some() {
                            let _ = send_protocol_error(
                                &response_tx,
                                Some(&header),
                                409,
                                "ClientHello was already received",
                                true,
                                &shutdown,
                                &stream_cancel,
                            )
                            .await;
                            break;
                        }
                        if hello.supported_protocol_version != PROTOCOL_VERSION {
                            let _ = send_protocol_error(
                                &response_tx,
                                Some(&header),
                                426,
                                "client does not support protocol version 1",
                                true,
                                &shutdown,
                                &stream_cancel,
                            )
                            .await;
                            break;
                        }
                        stream_identity = Some(StreamIdentity::from(&header));
                    }
                    Some(_) => {
                        let Some(expected) = stream_identity.as_ref() else {
                            let _ = send_protocol_error(
                                &response_tx,
                                Some(&header),
                                400,
                                "ClientHello must be the first frame",
                                true,
                                &shutdown,
                                &stream_cancel,
                            )
                            .await;
                            break;
                        };
                        if expected != &StreamIdentity::from(&header) {
                            let _ = send_protocol_error(
                                &response_tx,
                                Some(&header),
                                409,
                                "stream identity changed within one bidi connection",
                                true,
                                &shutdown,
                                &stream_cancel,
                            )
                            .await;
                            break;
                        }
                    }
                    None => {
                        let _ = send_protocol_error(
                            &response_tx,
                            Some(&header),
                            400,
                            "client frame payload is empty",
                            true,
                            &shutdown,
                            &stream_cancel,
                        )
                        .await;
                        break;
                    }
                }

                if let Some(client_frame::Payload::SnapshotBatch(batch)) = frame.payload.as_ref() {
                    if batch.entries.len() > repository.max_datasources_per_service()
                        || batch.entries.iter().any(|entry| {
                            entry.sql_stats.len() > repository.max_sql_entries_per_datasource()
                        })
                    {
                        let _ = send_protocol_error(
                            &response_tx,
                            Some(&header),
                            413,
                            "snapshot batch exceeds configured repository capacity",
                            true,
                            &shutdown,
                            &stream_cancel,
                        )
                        .await;
                        break;
                    }
                    if let Err(error) = prepare_batch(&header, batch) {
                        let _ = send_protocol_error(
                            &response_tx,
                            Some(&header),
                            400,
                            &error,
                            true,
                            &shutdown,
                            &stream_cancel,
                        )
                        .await;
                        break;
                    }
                }
                let is_snapshot = matches!(
                    frame.payload.as_ref(),
                    Some(client_frame::Payload::SnapshotBatch(_))
                );
                let is_goodbye = matches!(
                    frame.payload.as_ref(),
                    Some(client_frame::Payload::Goodbye(_))
                );
                if let Some(client_frame::Payload::CommandAck(ack)) = frame.payload.as_ref() {
                    if let Err(error) = command_broker.record_result(&header, ack) {
                        let _ = send_protocol_error(
                            &response_tx,
                            Some(&header),
                            409,
                            &error,
                            true,
                            &shutdown,
                            &stream_cancel,
                        )
                        .await;
                        break;
                    }
                }
                let response = if is_snapshot {
                    let service_id = stream_service_id(&header);
                    handler
                        .handle_frame_with_snapshot_commit(&frame, |entries, replace| {
                            let Ok(prepared) = prepare_entries(&header, entries) else {
                                return false;
                            };
                            if replace {
                                repository.replace_stream(&service_id, prepared)
                            } else {
                                repository.apply_batch(prepared)
                            }
                        })
                        .await
                } else {
                    handler.handle_frame(&frame).await
                };
                let close_after_response = matches!(
                    response.as_ref().and_then(|frame| frame.payload.as_ref()),
                    Some(server_frame::Payload::Error(error)) if error.close_stream
                );
                if let Some(response) = response {
                    if send_response(&response_tx, Ok(response), &shutdown, &stream_cancel).await {
                        break;
                    }
                }
                if matches!(
                    frame.payload.as_ref(),
                    Some(client_frame::Payload::Hello(_))
                ) {
                    if let Err(error) = command_broker.attach_stream(
                        &header,
                        response_tx.clone(),
                        connection_cancel.clone(),
                    ) {
                        let _ = send_protocol_error(
                            &response_tx,
                            Some(&header),
                            503,
                            &error,
                            true,
                            &shutdown,
                            &stream_cancel,
                        )
                        .await;
                        break;
                    }
                    attached_header = Some(header.clone());
                }
                if is_goodbye || close_after_response {
                    break;
                }
            }
            if let Some(header) = attached_header.as_ref() {
                command_broker.detach_stream(header, &response_tx);
            }
        });

        Ok(Response::new(OwnedConnectStream::new(response_rx, task)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StreamIdentity {
    service_name: String,
    instance_id: String,
    boot_id: String,
    stream_epoch: u64,
}

impl From<&FrameHeader> for StreamIdentity {
    fn from(header: &FrameHeader) -> Self {
        Self {
            service_name: header.service_name.clone(),
            instance_id: header.instance_id.clone(),
            boot_id: header.boot_id.clone(),
            stream_epoch: header.stream_epoch,
        }
    }
}

fn frame_header(frame: &ClientFrame) -> Option<&FrameHeader> {
    match frame.payload.as_ref()? {
        client_frame::Payload::Hello(value) => value.header.as_ref(),
        client_frame::Payload::SnapshotBatch(value) => value.header.as_ref(),
        client_frame::Payload::Heartbeat(value) => value.header.as_ref(),
        client_frame::Payload::CommandAck(value) => value.header.as_ref(),
        client_frame::Payload::Goodbye(value) => value.header.as_ref(),
    }
}

fn prepare_batch(
    header: &FrameHeader,
    batch: &SnapshotBatch,
) -> Result<Vec<(DataSourceId, DataSourceEntry)>, String> {
    prepare_entries(header, &batch.entries)
}

fn prepare_entries(
    header: &FrameHeader,
    entries: &[SnapshotEntry],
) -> Result<Vec<(DataSourceId, DataSourceEntry)>, String> {
    entries
        .iter()
        .map(|entry| prepare_entry(header, entry))
        .collect()
}

fn prepare_entry(
    header: &FrameHeader,
    entry: &SnapshotEntry,
) -> Result<(DataSourceId, DataSourceEntry), String> {
    let identity = checked_i64(entry.datasource_id, "datasource_id")?;
    let pool = entry
        .pool
        .as_ref()
        .ok_or_else(|| format!("datasource {} is missing pool metrics", entry.datasource_id))?;
    let service_id = stream_service_id(header);
    let db_type = entry.driver_name.clone();
    let mut sql_stats = HashMap::with_capacity(entry.sql_stats.len());
    for (index, stat) in entry.sql_stats.iter().enumerate() {
        let mut hasher = DefaultHasher::new();
        stat.fingerprint.hash(&mut hasher);
        let hash = hasher.finish() as i64;
        let previous = sql_stats.insert(
            hash,
            SqlListContent {
                service_id: Some(service_id.clone()),
                id: checked_i64(index as u64 + 1, "sql_index")?,
                sql: Some(stat.fingerprint.clone()),
                hash,
                execute_count: checked_i64(stat.exec_count, "sql.exec_count")?,
                total_time: checked_i64(stat.exec_time_millis, "sql.exec_time_millis")?,
                fetch_row_count: checked_i64(stat.fetch_row_count, "sql.fetch_row_count")?,
                effected_row_count: checked_i64(stat.update_count, "sql.update_count")?,
                db_type: db_type.clone(),
                data_source: serde_json::json!(identity),
                ..Default::default()
            },
        );
        if previous.is_some() {
            return Err(format!(
                "datasource {} contains duplicate SQL identity",
                entry.datasource_id
            ));
        }
    }
    let execute_count = entry.sql_stats.iter().try_fold(0_u64, |total, stat| {
        total
            .checked_add(stat.exec_count)
            .ok_or_else(|| "datasource execute count overflow".to_owned())
    })?;
    let mut wall = WallResult::default();
    wall.content.check_count = checked_i64(entry.wall_check_count, "wall.check_count")?;
    wall.content.black_list_hit_count = checked_i64(entry.wall_deny_count, "wall.deny_count")?;
    wall.content.violation_count = checked_i64(entry.wall_violation_count, "wall.violation_count")?;
    let datasource = DataSourceContent {
        service_id: Some(service_id.clone()),
        identity,
        name: Some(entry.datasource_name.clone()),
        db_type: db_type.clone(),
        driver_class_name: db_type,
        active_count: i64::from(pool.active_count),
        pooling_count: i64::from(pool.idle_count),
        max_active: i64::from(pool.max_active),
        wait_thread_count: i64::from(pool.waiting_count),
        execute_count: checked_i64(execute_count, "datasource.execute_count")?,
        ..Default::default()
    };
    Ok((
        DataSourceId {
            service_id,
            identity,
        },
        DataSourceEntry {
            datasource,
            sql_stats,
            wall,
            last_updated_ms: checked_i64(entry.sampling_time_millis, "sampling_time_millis")?,
            sequence: header.sequence,
        },
    ))
}

fn stream_service_id(header: &FrameHeader) -> String {
    format!(
        "{}/{}/{}/{}",
        header.service_name, header.instance_id, header.boot_id, header.stream_epoch
    )
}

fn checked_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds the Admin DTO range"))
}

async fn send_protocol_error(
    response_tx: &mpsc::Sender<Result<ServerFrame, Status>>,
    header: Option<&FrameHeader>,
    code: u32,
    message: &str,
    close_stream: bool,
    shutdown: &CancellationToken,
    stream_cancel: &CancellationToken,
) -> bool {
    send_response(
        response_tx,
        Ok(ServerFrame {
            payload: Some(server_frame::Payload::Error(ServerError {
                header: header.map(reply_header),
                code,
                message: message.to_owned(),
                close_stream,
            })),
        }),
        shutdown,
        stream_cancel,
    )
    .await
}

async fn send_response(
    response_tx: &mpsc::Sender<Result<ServerFrame, Status>>,
    response: Result<ServerFrame, Status>,
    shutdown: &CancellationToken,
    stream_cancel: &CancellationToken,
) -> bool {
    tokio::select! {
        biased;
        () = shutdown.cancelled() => true,
        () = stream_cancel.cancelled() => true,
        result = response_tx.send(response) => result.is_err(),
    }
}

fn reply_header(header: &FrameHeader) -> FrameHeader {
    FrameHeader {
        protocol_version: PROTOCOL_VERSION,
        service_name: header.service_name.clone(),
        instance_id: header.instance_id.clone(),
        boot_id: header.boot_id.clone(),
        stream_epoch: header.stream_epoch,
        sequence: 0,
        emitted_at_unix_ms: now_unix_ms(),
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
