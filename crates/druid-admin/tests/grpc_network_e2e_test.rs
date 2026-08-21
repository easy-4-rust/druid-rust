//! Real TCP boundary tests for the shared `druid_metrics.v1` bidi protocol.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use druid::stats::{
    DataSourceIdentity, DataSourceMonitorable, DruidTelemetrySnapshot, PoolSnapshot,
    SnapshotUnavailable,
};
use druid_admin::ingest::IngestService;
use druid_admin::MetricsRepository;
use druid_metrics::aggregator::AggregatedBatch;
use druid_metrics::grpc_client::{GrpcClientConfig, GrpcMetricsClient};
use druid_metrics::protocol::metrics_ingest_client::MetricsIngestClient;
use druid_metrics::protocol::metrics_ingest_server::{MetricsIngest, MetricsIngestServer};
use druid_metrics::protocol::{
    client_frame, server_frame, BatchAck, ClientFrame, ClientGoodbye, ClientHeartbeat, ClientHello,
    FrameHeader, HelloAck, PoolSnapshotMsg, ServerFrame, SnapshotBatch, SnapshotEntry, SqlStatMsg,
};
use druid_metrics::{DruidMetricsConfig, DruidMetricsRuntime};
use prost::Message;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Notify};
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tokio_util::sync::CancellationToken;
use tonic::transport::{Endpoint, Server};
use tonic::{Request, Response, Status, Streaming};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct StreamIdentity {
    service_name: String,
    instance_id: String,
    boot_id: String,
    stream_epoch: u64,
}

impl StreamIdentity {
    fn new(suffix: &str, stream_epoch: u64) -> Self {
        Self {
            service_name: "orders-runtime".to_owned(),
            instance_id: format!("instance-{suffix}"),
            boot_id: format!("boot-{suffix}"),
            stream_epoch,
        }
    }

    fn header(&self, sequence: u64) -> FrameHeader {
        FrameHeader {
            protocol_version: 1,
            service_name: self.service_name.clone(),
            instance_id: self.instance_id.clone(),
            boot_id: self.boot_id.clone(),
            stream_epoch: self.stream_epoch,
            sequence,
            emitted_at_unix_ms: 1_755_734_400_000,
        }
    }
}

struct AdminFixture {
    http_addr: SocketAddr,
    grpc_endpoint: String,
    shutdown: CancellationToken,
    http_handle: tokio::task::JoinHandle<Result<(), std::io::Error>>,
    grpc_handle: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl AdminFixture {
    async fn start() -> Self {
        let repository = MetricsRepository::new();
        let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind HTTP loopback listener");
        let grpc_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind gRPC loopback listener");
        let http_addr = http_listener.local_addr().expect("HTTP local address");
        let grpc_addr = grpc_listener.local_addr().expect("gRPC local address");
        let shutdown = CancellationToken::new();
        let http_shutdown = shutdown.clone();
        let grpc_shutdown = shutdown.clone();
        let http_app = druid_admin::standalone_router(repository.clone());
        let ingest = IngestService::new(repository);

        let http_handle = tokio::spawn(async move {
            axum::serve(http_listener, http_app)
                .with_graceful_shutdown(http_shutdown.cancelled_owned())
                .await
        });
        let grpc_handle = tokio::spawn(async move {
            Server::builder()
                .add_service(ingest.into_server())
                .serve_with_incoming_shutdown(
                    TcpListenerStream::new(grpc_listener),
                    grpc_shutdown.cancelled_owned(),
                )
                .await
        });

        Self {
            http_addr,
            grpc_endpoint: format!("http://{grpc_addr}"),
            shutdown,
            http_handle,
            grpc_handle,
        }
    }

    async fn shutdown(self) {
        self.shutdown.cancel();
        tokio::time::timeout(TEST_TIMEOUT, async {
            self.http_handle
                .await
                .expect("HTTP task joins")
                .expect("HTTP server stops cleanly");
            self.grpc_handle
                .await
                .expect("gRPC task joins")
                .expect("gRPC server stops cleanly");
        })
        .await
        .expect("all fixture tasks stop before timeout");
    }
}

struct NetworkStream {
    tx: mpsc::Sender<ClientFrame>,
    inbound: tonic::Streaming<ServerFrame>,
}

async fn open_generated_stream(endpoint: &str, identity: &StreamIdentity) -> NetworkStream {
    let channel = Endpoint::from_shared(endpoint.to_owned())
        .expect("valid test endpoint")
        .connect()
        .await
        .expect("connect generated client over TCP");
    let mut client = MetricsIngestClient::new(channel);
    let (tx, rx) = mpsc::channel(8);
    let response = client
        .connect(ReceiverStream::new(rx))
        .await
        .expect("open generated bidi Connect RPC");
    let mut stream = NetworkStream {
        tx,
        inbound: response.into_inner(),
    };
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::Hello(ClientHello {
                header: Some(identity.header(0)),
                supported_protocol_version: 1,
                full_snapshot_requested: false,
            })),
        })
        .await
        .expect("send ClientHello");
    let hello = next_server_frame(&mut stream.inbound).await;
    assert!(
        matches!(hello.payload, Some(server_frame::Payload::HelloAck(_))),
        "first response must acknowledge ClientHello: {hello:?}"
    );
    stream
}

async fn next_server_frame(inbound: &mut tonic::Streaming<ServerFrame>) -> ServerFrame {
    tokio::time::timeout(TEST_TIMEOUT, inbound.message())
        .await
        .expect("server response before timeout")
        .expect("valid gRPC response")
        .expect("response stream remains open")
}

async fn send_batch_and_expect_ack(
    stream: &mut NetworkStream,
    identity: &StreamIdentity,
    sequence: u64,
    entry: SnapshotEntry,
    is_full_snapshot: bool,
) -> u64 {
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(identity.header(sequence)),
                entries: vec![entry],
                is_full_snapshot,
            })),
        })
        .await
        .expect("send SnapshotBatch");
    match next_server_frame(&mut stream.inbound).await.payload {
        Some(server_frame::Payload::BatchAck(ack)) => ack.accepted_sequence,
        other => panic!("expected BatchAck, got {other:?}"),
    }
}

fn orders_entry(active: u32, idle: u32) -> SnapshotEntry {
    SnapshotEntry {
        datasource_id: 42,
        datasource_name: "orders".to_owned(),
        driver_name: Some("postgresql".to_owned()),
        pool: Some(PoolSnapshotMsg {
            active_count: active,
            idle_count: idle,
            max_active: 32,
            max_idle: 24,
            waiting_count: 3,
        }),
        sql_stats: vec![SqlStatMsg {
            fingerprint: "fp:orders-by-id".to_owned(),
            exec_count: 50,
            exec_time_millis: 250,
            fetch_row_count: 50,
            update_count: 0,
        }],
        wall_check_count: 5000,
        wall_deny_count: 3,
        wall_violation_count: 10,
        sampling_time_millis: 1_755_734_400_000,
    }
}

fn grpc_config(endpoint: &str, suffix: &str) -> GrpcClientConfig {
    GrpcClientConfig {
        endpoint: Endpoint::from_shared(endpoint.to_owned()).expect("valid gRPC endpoint"),
        service_name: "orders-runtime".to_owned(),
        instance_id: format!("instance-{suffix}"),
        boot_id: format!("boot-{suffix}"),
        ack_window_capacity: 8,
        shutdown_timeout: TEST_TIMEOUT,
    }
}

async fn get_json(http_addr: SocketAddr, path: &str) -> Value {
    reqwest::get(format!("http://{http_addr}{path}"))
        .await
        .expect("HTTP request succeeds")
        .error_for_status()
        .expect("HTTP response is successful")
        .json()
        .await
        .expect("HTTP response is JSON")
}

async fn datasource_content(http_addr: SocketAddr) -> Vec<Value> {
    get_json(http_addr, "/druid/datasource.json")
        .await
        .get("Content")
        .and_then(Value::as_array)
        .expect("datasource Content is an array")
        .clone()
}

#[tokio::test]
async fn grpc_snapshot_is_visible_through_datasource_sql_and_wall_rest() {
    let fixture = AdminFixture::start().await;
    let client = GrpcMetricsClient::connect(grpc_config(&fixture.grpc_endpoint, "projection"))
        .await
        .expect("production metrics client connects");
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(7, 15)],
            is_full_snapshot: true,
        })
        .await
        .expect("production client accepts full snapshot");
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("client waits for the network ACK and joins");

    let datasources = datasource_content(fixture.http_addr).await;
    assert_eq!(datasources.len(), 1);
    assert_eq!(datasources[0]["Identity"], 42);
    assert_eq!(datasources[0]["DbType"], "postgresql");
    assert_eq!(datasources[0]["ActiveCount"], 7);
    assert_eq!(datasources[0]["PoolingCount"], 15);

    let sql = get_json(fixture.http_addr, "/druid/sql.json").await;
    let sql_content = sql["Content"].as_array().expect("SQL Content array");
    assert_eq!(sql_content.len(), 1);
    assert_eq!(sql_content[0]["SQL"], "fp:orders-by-id");
    assert_eq!(sql_content[0]["ExecuteCount"], 50);
    let encoded_sql = serde_json::to_string(&sql).expect("serialize SQL response");
    assert!(!encoded_sql.contains("select * from orders"));
    assert!(!encoded_sql.contains("customer_id=42"));

    let wall = get_json(fixture.http_addr, "/druid/wall.json").await;
    assert_eq!(wall["Content"]["checkCount"], 5000);
    assert_eq!(wall["Content"]["violationCount"], 10);

    fixture.shutdown().await;
}

#[tokio::test]
async fn grpc_newer_sequence_updates_rest_state() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("newer", 1);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;

    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 1, orders_entry(1, 2), true).await,
        1
    );
    let first = datasource_content(fixture.http_addr).await;
    assert_eq!(first[0]["ActiveCount"], 1);
    assert_eq!(first[0]["PoolingCount"], 2);

    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 2, orders_entry(7, 15), false).await,
        2
    );
    let second = datasource_content(fixture.http_addr).await;
    assert_eq!(second.len(), 1);
    assert_eq!(second[0]["ActiveCount"], 7);
    assert_eq!(second[0]["PoolingCount"], 15);

    drop(stream.tx);
    fixture.shutdown().await;
}

#[tokio::test]
async fn grpc_duplicate_and_increasing_sequences_are_handled_over_network() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("duplicate", 10);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;

    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 10, orders_entry(10, 20), true).await,
        10
    );
    let original = datasource_content(fixture.http_addr).await;
    assert_eq!(original.len(), 1);
    assert_eq!(original[0]["ActiveCount"], 10);

    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 10, orders_entry(99, 99), false).await,
        10
    );
    let after_duplicate = datasource_content(fixture.http_addr).await;
    assert_eq!(after_duplicate.len(), 1);
    assert_eq!(after_duplicate[0]["ActiveCount"], 10);
    assert_eq!(after_duplicate[0]["PoolingCount"], 20);

    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 11, orders_entry(11, 21), false).await,
        11
    );
    let after_increase = datasource_content(fixture.http_addr).await;
    assert_eq!(after_increase.len(), 1);
    assert_eq!(after_increase[0]["ActiveCount"], 11);
    assert_eq!(after_increase[0]["PoolingCount"], 21);

    drop(stream.tx);
    fixture.shutdown().await;
}

#[tokio::test]
async fn health_and_metrics_remain_truthful_after_grpc_ingest() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("health", 20);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 1, orders_entry(7, 15), true).await,
        1
    );

    let health = reqwest::get(format!("http://{}/health", fixture.http_addr))
        .await
        .expect("GET /health");
    assert_eq!(health.status(), 200);
    assert_eq!(
        health.json::<Value>().await.expect("health JSON"),
        json!({"status": "ok"})
    );

    let metrics = reqwest::get(format!("http://{}/metrics", fixture.http_addr))
        .await
        .expect("GET /metrics");
    assert_eq!(metrics.status(), 200);
    let metrics = metrics.text().await.expect("metrics text");
    assert!(metrics.contains("druid_admin_datasource_count 1"));
    assert!(metrics.contains("druid_admin_ingest_total 1"));
    assert!(metrics.contains(
        "druid_admin_datasource_active_count{service=\"orders\",datasource=\"42\",db_type=\"postgresql\"} 7"
    ));

    drop(stream.tx);
    fixture.shutdown().await;
}

#[tokio::test]
async fn grpc_stream_enforces_identity_full_snapshot_resync_and_control_frames() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("protocol-guards", 30);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::Heartbeat(ClientHeartbeat {
                header: Some(identity.header(0)),
                pending_ack_count: 0,
            })),
        })
        .await
        .expect("send heartbeat");
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(identity.header(1)),
                entries: vec![orders_entry(1, 2)],
                is_full_snapshot: false,
            })),
        })
        .await
        .expect("send non-full initial snapshot");
    let full_required = next_server_frame(&mut stream.inbound).await;
    assert!(matches!(
        full_required.payload,
        Some(server_frame::Payload::HelloAck(HelloAck {
            require_full_snapshot: true,
            ..
        }))
    ));
    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 5, orders_entry(5, 6), true).await,
        5
    );

    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(identity.header(7)),
                entries: vec![orders_entry(7, 8)],
                is_full_snapshot: false,
            })),
        })
        .await
        .expect("send sequence gap");
    let resync = next_server_frame(&mut stream.inbound).await;
    assert!(matches!(
        resync.payload,
        Some(server_frame::Payload::ResyncRequired(ref required))
            if required.expected_sequence == 6
    ));

    let changed_identity = StreamIdentity::new("changed-mid-stream", 30);
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(changed_identity.header(6)),
                entries: vec![orders_entry(99, 99)],
                is_full_snapshot: false,
            })),
        })
        .await
        .expect("send mismatched identity");
    let identity_error = next_server_frame(&mut stream.inbound).await;
    assert!(matches!(
        identity_error.payload,
        Some(server_frame::Payload::Error(ref error)) if error.close_stream
    ));

    let goodbye_identity = StreamIdentity::new("goodbye", 31);
    let mut goodbye_stream = open_generated_stream(&fixture.grpc_endpoint, &goodbye_identity).await;
    goodbye_stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::Goodbye(ClientGoodbye {
                header: Some(goodbye_identity.header(0)),
                reason: "test_complete".to_owned(),
            })),
        })
        .await
        .expect("send goodbye");
    let closed = tokio::time::timeout(TEST_TIMEOUT, goodbye_stream.inbound.message())
        .await
        .expect("goodbye closes response stream")
        .expect("valid close status");
    assert!(
        closed.is_none(),
        "goodbye must end the bidi response stream"
    );

    fixture.shutdown().await;
}

#[derive(Clone)]
struct ReconnectProbe {
    connections: Arc<AtomicUsize>,
    first_frame: Arc<Mutex<Option<Vec<u8>>>>,
    replay_results: mpsc::UnboundedSender<bool>,
}

#[tonic::async_trait]
impl MetricsIngest for ReconnectProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
        let first_frame = Arc::clone(&self.first_frame);
        let replay_results = self.replay_results.clone();
        tokio::spawn(async move {
            let Some(hello) = inbound.message().await.ok().flatten() else {
                return;
            };
            let header = frame_header(&hello).expect("hello header").clone();
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::HelloAck(HelloAck {
                        header: Some(reply_header(&header)),
                        server_protocol_version: 1,
                        require_full_snapshot: connection == 0,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            let Some(frame) = inbound.message().await.ok().flatten() else {
                return;
            };
            let encoded = frame.encode_to_vec();
            if connection == 0 {
                *first_frame.lock().expect("first-frame lock") = Some(encoded);
                return;
            }
            let same = first_frame
                .lock()
                .expect("first-frame lock")
                .as_ref()
                .is_some_and(|first| first == &encoded);
            let _ = replay_results.send(same);
            let sequence = frame_header(&frame).expect("snapshot header").sequence;
            let _ = tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&header)),
                        accepted_sequence: sequence,
                    })),
                }))
                .await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn grpc_client_reconnects_and_resends_only_pending_frame_bytes() {
    let (replay_tx, mut replay_rx) = mpsc::unbounded_channel();
    let service = ReconnectProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        first_frame: Arc::new(Mutex::new(None)),
        replay_results: replay_tx,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let client = GrpcMetricsClient::connect(grpc_config(&endpoint, "reconnect"))
        .await
        .expect("client initially connects");
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(7, 15)],
            is_full_snapshot: true,
        })
        .await
        .expect("queue snapshot");
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("shutdown waits through reconnect and ACK");
    assert!(
        tokio::time::timeout(TEST_TIMEOUT, replay_rx.recv())
            .await
            .expect("replay observed before timeout")
            .expect("replay result exists"),
        "reconnect must resend the exact immutable pending frame"
    );
    shutdown.cancel();
    server_handle
        .await
        .expect("probe server joins")
        .expect("probe server stops cleanly");
}

#[derive(Clone)]
struct CleanupProbe {
    active_streams: Arc<AtomicUsize>,
    idle: Arc<Notify>,
}

struct ActiveStreamGuard {
    active_streams: Arc<AtomicUsize>,
    idle: Arc<Notify>,
}

impl Drop for ActiveStreamGuard {
    fn drop(&mut self) {
        self.active_streams.fetch_sub(1, Ordering::AcqRel);
        self.idle.notify_waiters();
    }
}

#[tonic::async_trait]
impl MetricsIngest for CleanupProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        self.active_streams.fetch_add(1, Ordering::AcqRel);
        let guard = ActiveStreamGuard {
            active_streams: Arc::clone(&self.active_streams),
            idle: Arc::clone(&self.idle),
        };
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(2);
        tokio::spawn(async move {
            let _guard = guard;
            let Some(hello) = inbound.message().await.ok().flatten() else {
                return;
            };
            let header = frame_header(&hello).expect("hello header");
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::HelloAck(HelloAck {
                        header: Some(reply_header(header)),
                        server_protocol_version: 1,
                        require_full_snapshot: true,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            while inbound.message().await.ok().flatten().is_some() {}
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn grpc_client_shutdown_joins_transport_and_leaves_no_active_stream() {
    let active_streams = Arc::new(AtomicUsize::new(0));
    let idle = Arc::new(Notify::new());
    let service = CleanupProbe {
        active_streams: Arc::clone(&active_streams),
        idle: Arc::clone(&idle),
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let client = GrpcMetricsClient::connect(grpc_config(&endpoint, "cleanup"))
        .await
        .expect("client connects");
    assert_eq!(active_streams.load(Ordering::Acquire), 1);

    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("client shutdown joins every owned task");
    tokio::time::timeout(TEST_TIMEOUT, async {
        while active_streams.load(Ordering::Acquire) != 0 {
            idle.notified().await;
        }
    })
    .await
    .expect("server observes client stream cleanup");

    shutdown.cancel();
    server_handle
        .await
        .expect("probe server joins")
        .expect("probe server stops cleanly");
}

async fn start_probe_server<S>(
    service: S,
) -> (
    String,
    CancellationToken,
    tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
)
where
    S: MetricsIngest + Clone + Send + Sync + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind probe server");
    let addr = listener.local_addr().expect("probe address");
    let shutdown = CancellationToken::new();
    let server_shutdown = shutdown.clone();
    let handle = tokio::spawn(async move {
        Server::builder()
            .add_service(MetricsIngestServer::new(service))
            .serve_with_incoming_shutdown(
                TcpListenerStream::new(listener),
                server_shutdown.cancelled_owned(),
            )
            .await
    });
    (format!("http://{addr}"), shutdown, handle)
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

fn reply_header(header: &FrameHeader) -> FrameHeader {
    FrameHeader {
        protocol_version: 1,
        service_name: header.service_name.clone(),
        instance_id: header.instance_id.clone(),
        boot_id: header.boot_id.clone(),
        stream_epoch: header.stream_epoch,
        sequence: 0,
        emitted_at_unix_ms: header.emitted_at_unix_ms,
    }
}

struct RuntimeProbe {
    samples: AtomicUsize,
    sampled: Notify,
}

impl DataSourceMonitorable for RuntimeProbe {
    fn name(&self) -> &str {
        "runtime-orders"
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        DataSourceIdentity {
            id: 84,
            name: "runtime-orders".to_owned(),
            driver_name: Some("postgresql".to_owned()),
        }
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        self.samples.fetch_add(1, Ordering::AcqRel);
        self.sampled.notify_waiters();
        Ok(DruidTelemetrySnapshot {
            identity: self.identity(),
            pool_snapshot: PoolSnapshot {
                active_count: 4,
                idle_count: 8,
                max_active: 16,
                max_idle: 12,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 1_755_734_400_000,
        })
    }
}

#[tokio::test]
async fn runtime_shutdown_flushes_last_aggregated_batch_over_grpc() {
    let fixture = AdminFixture::start().await;
    let runtime = DruidMetricsRuntime::start_with_grpc(
        DruidMetricsConfig::builder()
            .sample_interval(Duration::from_millis(1))
            .coalesce_window(Duration::from_secs(30))
            .build()
            .expect("valid runtime config"),
        grpc_config(&fixture.grpc_endpoint, "runtime-shutdown"),
    )
    .await
    .expect("network runtime starts");
    let source = Arc::new(RuntimeProbe {
        samples: AtomicUsize::new(0),
        sampled: Notify::new(),
    });
    let _guard = runtime.register(source.clone());
    tokio::time::timeout(TEST_TIMEOUT, async {
        while source.samples.load(Ordering::Acquire) == 0 {
            source.sampled.notified().await;
        }
    })
    .await
    .expect("sampler produces one pending snapshot");

    runtime
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("runtime shutdown flushes, ACKs, and joins");
    let content = datasource_content(fixture.http_addr).await;
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["Identity"], 84);
    assert_eq!(content[0]["ActiveCount"], 4);

    fixture.shutdown().await;
}
