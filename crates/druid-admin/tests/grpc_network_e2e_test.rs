//! Real TCP boundary tests for the shared `druid_metrics.v1` bidi protocol.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
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
    client_frame, command, server_frame, BatchAck, ClientFrame, ClientGoodbye, ClientHeartbeat,
    ClientHello, Command, FrameHeader, HelloAck, PoolSnapshotMsg, RequestFullSnapshotCmd,
    ResyncRequired, ServerFrame, SnapshotBatch, SnapshotEntry, SqlStatMsg,
};
use druid_metrics::registry::{RegistryCommandExecutor, RegistryEntry};
use druid_metrics::{DruidMetricsConfig, DruidMetricsRuntime};
use parking_lot::RwLock;
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
async fn grpc_multi_entry_batch_is_atomically_visible_and_counted_once() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("atomic-batch", 25);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    let mut second = orders_entry(4, 5);
    second.datasource_id = 43;
    second.datasource_name = "inventory".to_owned();
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(identity.header(1)),
                entries: vec![orders_entry(7, 15), second],
                is_full_snapshot: true,
            })),
        })
        .await
        .expect("send multi-entry batch");
    let before_ack = datasource_content(fixture.http_addr).await;
    assert!(
        before_ack.is_empty() || before_ack.len() == 2,
        "HTTP must never expose a partially applied batch"
    );
    let ack = next_server_frame(&mut stream.inbound).await;
    assert!(matches!(
        ack.payload,
        Some(server_frame::Payload::BatchAck(BatchAck {
            accepted_sequence: 1,
            ..
        }))
    ));
    let after_ack = datasource_content(fixture.http_addr).await;
    assert_eq!(after_ack.len(), 2);
    let metrics = reqwest::get(format!("http://{}/metrics", fixture.http_addr))
        .await
        .expect("GET /metrics")
        .text()
        .await
        .expect("metrics text");
    assert!(metrics.contains("druid_admin_ingest_total 1"));
    assert!(metrics.contains("druid_admin_datasource_count 2"));
    drop(stream.tx);
    fixture.shutdown().await;
}

#[tokio::test]
async fn repository_rejection_does_not_ack_or_advance_and_same_sequence_can_retry() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("atomic-rejection", 27);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    let first = orders_entry(1, 2);
    let mut duplicate = orders_entry(99, 99);
    duplicate.datasource_name = "duplicate-orders".to_owned();
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(identity.header(1)),
                entries: vec![first, duplicate],
                is_full_snapshot: true,
            })),
        })
        .await
        .expect("send atomically rejected batch");
    let rejected = next_server_frame(&mut stream.inbound).await;
    assert!(matches!(
        rejected.payload,
        Some(server_frame::Payload::ResyncRequired(ResyncRequired {
            expected_sequence: 1,
            ..
        }))
    ));
    assert!(
        datasource_content(fixture.http_addr).await.is_empty(),
        "rejected batch must not become partially visible"
    );

    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 1, orders_entry(7, 15), true,).await,
        1,
        "corrected retry uses the unchanged expected sequence"
    );
    wait_for_active_count(fixture.http_addr, 7).await;
    let metrics = reqwest::get(format!("http://{}/metrics", fixture.http_addr))
        .await
        .expect("GET /metrics")
        .text()
        .await
        .expect("metrics text");
    assert!(metrics.contains("druid_admin_ingest_total 2"));
    assert!(metrics.contains("druid_admin_ingest_rejected_total 1"));
    drop(stream.tx);
    fixture.shutdown().await;
}

#[tokio::test]
async fn prometheus_escapes_client_controlled_label_values() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("prometheus-escape", 26);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    let mut entry = orders_entry(7, 15);
    entry.datasource_name = "orders\"\\\nmalicious_metric 1".to_owned();
    entry.driver_name = Some("post\"\\\ngresql".to_owned());
    assert_eq!(
        send_batch_and_expect_ack(&mut stream, &identity, 1, entry, true).await,
        1
    );
    let metrics = reqwest::get(format!("http://{}/metrics", fixture.http_addr))
        .await
        .expect("GET /metrics")
        .text()
        .await
        .expect("metrics text");
    assert!(metrics.contains("service=\"orders\\\"\\\\\\nmalicious_metric 1\""));
    assert!(metrics.contains("db_type=\"post\\\"\\\\\\ngresql\""));
    assert!(
        !metrics.lines().any(|line| line == "malicious_metric 1"),
        "a label newline must not inject a new Prometheus sample"
    );
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

#[tokio::test]
async fn grpc_rejects_zero_and_max_sequences_without_poisoning_other_streams() {
    let fixture = AdminFixture::start().await;
    for (suffix, sequence) in [("zero-sequence", 0), ("max-sequence", u64::MAX)] {
        let identity = StreamIdentity::new(suffix, sequence);
        let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
        stream
            .tx
            .send(ClientFrame {
                payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                    header: Some(identity.header(sequence)),
                    entries: vec![orders_entry(99, 99)],
                    is_full_snapshot: true,
                })),
            })
            .await
            .expect("send invalid sequence");
        let response = next_server_frame(&mut stream.inbound).await;
        assert!(matches!(
            response.payload,
            Some(server_frame::Payload::Error(ref error)) if error.close_stream
        ));
    }

    let healthy_identity = StreamIdentity::new("after-overflow", 44);
    let mut healthy_stream = open_generated_stream(&fixture.grpc_endpoint, &healthy_identity).await;
    assert_eq!(
        send_batch_and_expect_ack(
            &mut healthy_stream,
            &healthy_identity,
            1,
            orders_entry(7, 15),
            true,
        )
        .await,
        1
    );
    wait_for_active_count(fixture.http_addr, 7).await;
    drop(healthy_stream.tx);
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
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
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

#[derive(Debug, Clone, Copy)]
enum InvalidAckKind {
    MissingHeader,
    WrongIdentity,
    WrongProtocol,
}

#[derive(Clone)]
struct InvalidAckProbe {
    kind: InvalidAckKind,
    connections: Arc<AtomicUsize>,
    first_frame: Arc<Mutex<Option<Vec<u8>>>>,
    replay_results: mpsc::UnboundedSender<bool>,
}

#[tonic::async_trait]
impl MetricsIngest for InvalidAckProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
        let kind = self.kind;
        let first_frame = Arc::clone(&self.first_frame);
        let replay_results = self.replay_results.clone();
        tokio::spawn(async move {
            let Some(hello) = inbound.message().await.ok().flatten() else {
                return;
            };
            let hello_header = frame_header(&hello).expect("hello header").clone();
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::HelloAck(HelloAck {
                        header: Some(reply_header(&hello_header)),
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
            let sequence = frame_header(&frame).expect("batch header").sequence;
            if connection == 0 {
                *first_frame.lock().expect("first-frame lock") = Some(encoded);
                let header = match kind {
                    InvalidAckKind::MissingHeader => None,
                    InvalidAckKind::WrongIdentity => {
                        let mut header = reply_header(&hello_header);
                        header.boot_id.push_str("-wrong");
                        Some(header)
                    }
                    InvalidAckKind::WrongProtocol => {
                        let mut header = reply_header(&hello_header);
                        header.protocol_version = 2;
                        Some(header)
                    }
                };
                let _ = tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::BatchAck(BatchAck {
                            header,
                            accepted_sequence: sequence,
                        })),
                    }))
                    .await;
                return;
            }
            let same = first_frame
                .lock()
                .expect("first-frame lock")
                .as_ref()
                .is_some_and(|first| first == &encoded);
            let _ = replay_results.send(same);
            let _ = tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&hello_header)),
                        accepted_sequence: sequence,
                    })),
                }))
                .await;
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn invalid_ack_headers_close_transport_and_preserve_pending_frame() {
    for kind in [
        InvalidAckKind::MissingHeader,
        InvalidAckKind::WrongIdentity,
        InvalidAckKind::WrongProtocol,
    ] {
        let (replay_tx, mut replay_rx) = mpsc::unbounded_channel();
        let service = InvalidAckProbe {
            kind,
            connections: Arc::new(AtomicUsize::new(0)),
            first_frame: Arc::new(Mutex::new(None)),
            replay_results: replay_tx,
        };
        let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
        let client =
            GrpcMetricsClient::connect(grpc_config(&endpoint, &format!("invalid-ack-{kind:?}")))
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
            .expect("valid reconnect ACK completes shutdown");
        assert!(
            tokio::time::timeout(TEST_TIMEOUT, replay_rx.recv())
                .await
                .expect("replay observed")
                .expect("replay result"),
            "{kind:?} must preserve exact pending bytes across transport replacement"
        );
        shutdown.cancel();
        server_handle
            .await
            .expect("probe joins")
            .expect("probe stops");
    }
}

#[derive(Clone)]
struct ResyncThenAckProbe {
    connections: Arc<AtomicUsize>,
    replacement: mpsc::UnboundedSender<(u64, u32, bool)>,
}

#[tonic::async_trait]
impl MetricsIngest for ResyncThenAckProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let replacement = self.replacement.clone();
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
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
            let sequence = frame_header(&frame).expect("snapshot header").sequence;
            if connection == 0 {
                let _ = tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::ResyncRequired(ResyncRequired {
                            header: Some(reply_header(&header)),
                            reason: "test_gap".to_owned(),
                            expected_sequence: sequence,
                        })),
                    }))
                    .await;
                while inbound.message().await.ok().flatten().is_some() {}
                return;
            }
            let batch = match frame.payload.as_ref() {
                Some(client_frame::Payload::SnapshotBatch(batch)) => batch,
                other => panic!("expected authoritative SnapshotBatch, got {other:?}"),
            };
            let active = batch
                .entries
                .first()
                .and_then(|entry| entry.pool.as_ref())
                .map_or(0, |pool| pool.active_count);
            let _ = replacement.send((sequence, active, batch.is_full_snapshot));
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&header)),
                        accepted_sequence: sequence,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn resync_required_supersedes_pending_with_monotonic_registry_full_snapshot() {
    let source = Arc::new(MutableSource::new(9, 10));
    let (_registry, executor) = registry_executor(&source);
    let (replacement_tx, mut replacement_rx) = mpsc::unbounded_channel();
    let service = ResyncThenAckProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        replacement: replacement_tx,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let client = GrpcMetricsClient::connect_with_command_executor(
        grpc_config(&endpoint, "resync-required"),
        Some(executor),
    )
    .await
    .expect("client connects");
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(1, 2)],
            is_full_snapshot: false,
        })
        .await
        .expect("queue gap frame");
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("resync replacement is ACKed before shutdown");
    assert_eq!(
        tokio::time::timeout(TEST_TIMEOUT, replacement_rx.recv())
            .await
            .expect("replacement observed")
            .expect("replacement result"),
        (2, 9, true),
        "registry full snapshot supersedes sequence 1 and keeps monotonic sequence 2"
    );
    shutdown.cancel();
    server_handle
        .await
        .expect("probe joins")
        .expect("probe stops");
}

struct CutoffBarrierSource {
    active: AtomicU32,
    snapshot_sampled: Notify,
    release: (Mutex<bool>, Condvar),
}

impl CutoffBarrierSource {
    fn new(active: u32) -> Self {
        Self {
            active: AtomicU32::new(active),
            snapshot_sampled: Notify::new(),
            release: (Mutex::new(false), Condvar::new()),
        }
    }

    fn set_active(&self, active: u32) {
        self.active.store(active, Ordering::Release);
    }

    fn release_snapshot(&self) {
        let (lock, condition) = &self.release;
        *lock.lock().expect("cutoff gate lock") = true;
        condition.notify_all();
    }
}

impl DataSourceMonitorable for CutoffBarrierSource {
    fn name(&self) -> &str {
        "cutoff-orders"
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        DataSourceIdentity {
            id: 42,
            name: "orders".to_owned(),
            driver_name: Some("postgresql".to_owned()),
        }
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        let sampled_active = self.active.load(Ordering::Acquire);
        self.snapshot_sampled.notify_waiters();
        let (lock, condition) = &self.release;
        let mut released = lock.lock().expect("cutoff gate lock");
        while !*released {
            released = condition.wait(released).expect("cutoff gate wait");
        }
        Ok(DruidTelemetrySnapshot {
            identity: self.identity(),
            pool_snapshot: PoolSnapshot {
                active_count: sampled_active,
                idle_count: 0,
                max_active: 32,
                max_idle: 24,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 1_755_734_400_000,
        })
    }
}

#[derive(Clone)]
struct ResyncCutoffProbe {
    connections: Arc<AtomicUsize>,
    observed: mpsc::UnboundedSender<Vec<(u64, u32, bool)>>,
}

#[tonic::async_trait]
impl MetricsIngest for ResyncCutoffProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let observed = self.observed.clone();
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
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
            let Some(first) = inbound.message().await.ok().flatten() else {
                return;
            };
            let first_sequence = frame_header(&first).expect("first header").sequence;
            if connection == 0 {
                let _ = tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::ResyncRequired(ResyncRequired {
                            header: Some(reply_header(&header)),
                            reason: "cutoff_test".to_owned(),
                            expected_sequence: first_sequence,
                        })),
                    }))
                    .await;
                while inbound.message().await.ok().flatten().is_some() {}
                return;
            }

            let first_observation = snapshot_observation(&first);
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&header)),
                        accepted_sequence: first_observation.0,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            let Some(second) = inbound.message().await.ok().flatten() else {
                return;
            };
            let second_observation = snapshot_observation(&second);
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&header)),
                        accepted_sequence: second_observation.0,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            let _ = observed.send(vec![first_observation, second_observation]);
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

fn snapshot_observation(frame: &ClientFrame) -> (u64, u32, bool) {
    let batch = match frame.payload.as_ref() {
        Some(client_frame::Payload::SnapshotBatch(batch)) => batch,
        other => panic!("expected SnapshotBatch, got {other:?}"),
    };
    (
        batch.header.as_ref().expect("snapshot header").sequence,
        batch
            .entries
            .first()
            .and_then(|entry| entry.pool.as_ref())
            .map_or(0, |pool| pool.active_count),
        batch.is_full_snapshot,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resync_cutoff_preserves_live_update_queued_after_snapshot_sampling() {
    let source = Arc::new(CutoffBarrierSource::new(9));
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let executor = RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry {
            datasource_id: 42,
            weak_ref: Arc::downgrade(&monitorable),
        }])),
        8,
        32,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let (observed_tx, mut observed_rx) = mpsc::unbounded_channel();
    let service = ResyncCutoffProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        observed: observed_tx,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let client = GrpcMetricsClient::connect_with_command_executor(
        grpc_config(&endpoint, "resync-cutoff"),
        Some(executor),
    )
    .await
    .expect("client connects");
    let snapshot_sampled = source.snapshot_sampled.notified();
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(1, 2)],
            is_full_snapshot: false,
        })
        .await
        .expect("queue pre-cutoff frame");
    tokio::time::timeout(TEST_TIMEOUT, snapshot_sampled)
        .await
        .expect("authoritative snapshot sampled before queue drain");
    source.set_active(11);
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(11, 12)],
            is_full_snapshot: false,
        })
        .await
        .expect("queue post-cutoff live update");
    source.release_snapshot();
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("authoritative replacement and post-cutoff live update flush");
    assert_eq!(
        tokio::time::timeout(TEST_TIMEOUT, observed_rx.recv())
            .await
            .expect("server observed replacement and live update")
            .expect("observation result"),
        vec![(2, 9, true), (3, 11, false)]
    );
    shutdown.cancel();
    server_handle
        .await
        .expect("probe joins")
        .expect("probe stops");
}

#[derive(Clone)]
struct FullCommandDisconnectProbe {
    connections: Arc<AtomicUsize>,
    first_chunks: Arc<Mutex<Vec<(u64, u64)>>>,
    completed: mpsc::UnboundedSender<(Vec<(u64, u64)>, bool)>,
    disconnected_after_second: Arc<Notify>,
}

#[tonic::async_trait]
impl MetricsIngest for FullCommandDisconnectProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let first_chunks = Arc::clone(&self.first_chunks);
        let completed = self.completed.clone();
        let disconnected_after_second = Arc::clone(&self.disconnected_after_second);
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
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
                        require_full_snapshot: false,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            if connection == 0 {
                if tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::Command(Command {
                            header: Some(reply_header(&header)),
                            command_id: "durable-full-command".to_owned(),
                            payload: Some(command::Payload::RequestFullSnapshot(
                                RequestFullSnapshotCmd {},
                            )),
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
                let Some(first) = inbound.message().await.ok().flatten() else {
                    return;
                };
                let (first_sequence, first_id) = full_chunk_identity(&first);
                first_chunks
                    .lock()
                    .expect("chunk lock")
                    .push((first_sequence, first_id));
                if tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::BatchAck(BatchAck {
                            header: Some(reply_header(&header)),
                            accepted_sequence: first_sequence,
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
                let Some(second) = inbound.message().await.ok().flatten() else {
                    return;
                };
                first_chunks
                    .lock()
                    .expect("chunk lock")
                    .push(full_chunk_identity(&second));
                disconnected_after_second.notify_waiters();
                return;
            }

            let Some(replayed_second) = inbound.message().await.ok().flatten() else {
                return;
            };
            let replayed = full_chunk_identity(&replayed_second);
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&header)),
                        accepted_sequence: replayed.0,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            let Some(third) = inbound.message().await.ok().flatten() else {
                return;
            };
            let third = full_chunk_identity(&third);
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&header)),
                        accepted_sequence: third.0,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            let Some(command_ack) = inbound.message().await.ok().flatten() else {
                return;
            };
            let command_success = matches!(
                command_ack.payload,
                Some(client_frame::Payload::CommandAck(ref ack))
                    if ack.command_id == "durable-full-command" && ack.success
            );
            let mut chunks = first_chunks.lock().expect("chunk lock").clone();
            chunks.push(replayed);
            chunks.push(third);
            let _ = completed.send((chunks, command_success));
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

fn full_chunk_identity(frame: &ClientFrame) -> (u64, u64) {
    let batch = match frame.payload.as_ref() {
        Some(client_frame::Payload::SnapshotBatch(batch)) if batch.is_full_snapshot => batch,
        other => panic!("expected full SnapshotBatch, got {other:?}"),
    };
    (
        batch.header.as_ref().expect("full batch header").sequence,
        batch
            .entries
            .first()
            .expect("one entry per full chunk")
            .datasource_id,
    )
}

#[tokio::test]
async fn remote_full_snapshot_chunks_and_command_survive_mid_stream_disconnect() {
    let sources = [
        Arc::new(CommandSource::new(41)),
        Arc::new(CommandSource::new(42)),
        Arc::new(CommandSource::new(43)),
    ];
    let monitorables = sources
        .iter()
        .cloned()
        .map(|source| source as Arc<dyn DataSourceMonitorable>)
        .collect::<Vec<_>>();
    let registry = Arc::new(RwLock::new(
        monitorables
            .iter()
            .enumerate()
            .map(|(index, source)| RegistryEntry {
                datasource_id: 41 + index as u64,
                weak_ref: Arc::downgrade(source),
            })
            .collect(),
    ));
    let executor = RegistryCommandExecutor::new(
        registry,
        1,
        8,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let (completed_tx, mut completed_rx) = mpsc::unbounded_channel();
    let disconnected_after_second = Arc::new(Notify::new());
    let service = FullCommandDisconnectProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        first_chunks: Arc::new(Mutex::new(Vec::new())),
        completed: completed_tx,
        disconnected_after_second: Arc::clone(&disconnected_after_second),
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let mut config = grpc_config(&endpoint, "durable-full-command");
    config.ack_window_capacity = 1;
    let client = GrpcMetricsClient::connect_with_command_executor(config, Some(executor))
        .await
        .expect("client connects");
    tokio::time::timeout(TEST_TIMEOUT, disconnected_after_second.notified())
        .await
        .expect("first connection drops after receiving the second chunk");
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("all full chunks and command ACK survive reconnect");
    let (chunks, command_success) = tokio::time::timeout(TEST_TIMEOUT, completed_rx.recv())
        .await
        .expect("full command completion observed")
        .expect("full command result");
    assert_eq!(
        chunks
            .iter()
            .map(|(sequence, _)| *sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 2, 3],
        "sequence 2 is replayed exactly and the remaining chunk keeps sequence 3"
    );
    assert_eq!(
        chunks.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        vec![41, 42, 42, 43],
        "three registry entries arrive without omission; only the unacked chunk is replayed"
    );
    assert!(command_success, "success ACK follows every full chunk ACK");
    shutdown.cancel();
    server_handle
        .await
        .expect("probe joins")
        .expect("probe stops");
}

#[derive(Clone)]
struct CommandReplyReconnectProbe {
    connections: Arc<AtomicUsize>,
    final_chunk_acked: Arc<Notify>,
    observed: mpsc::UnboundedSender<(bool, bool)>,
    replayed_success: Arc<std::sync::atomic::AtomicBool>,
}

#[tonic::async_trait]
impl MetricsIngest for CommandReplyReconnectProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let final_chunk_acked = Arc::clone(&self.final_chunk_acked);
        let observed = self.observed.clone();
        let replayed_success = Arc::clone(&self.replayed_success);
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
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
                        require_full_snapshot: false,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            if connection == 0 {
                if tx
                    .send(Ok(full_snapshot_command(&header, "reply-reconnect")))
                    .await
                    .is_err()
                {
                    return;
                }
                let Some(chunk) = inbound.message().await.ok().flatten() else {
                    return;
                };
                let (sequence, _) = full_chunk_identity(&chunk);
                let _ = tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::BatchAck(BatchAck {
                            header: Some(reply_header(&header)),
                            accepted_sequence: sequence,
                        })),
                    }))
                    .await;
                final_chunk_acked.notify_waiters();
                // Deliberately close the response without reading the first CommandAck.
                return;
            }

            if connection == 1 {
                let Some(replayed_reply) = inbound.message().await.ok().flatten() else {
                    return;
                };
                replayed_success.store(
                    successful_command_ack(&replayed_reply, "reply-reconnect"),
                    Ordering::Release,
                );
                let Some(goodbye) = inbound.message().await.ok().flatten() else {
                    return;
                };
                assert!(matches!(
                    goodbye.payload,
                    Some(client_frame::Payload::Goodbye(_))
                ));
                return;
            }

            if tx
                .send(Ok(full_snapshot_command(&header, "reply-reconnect")))
                .await
                .is_err()
            {
                return;
            }
            let Some(idempotent_reply) = inbound.message().await.ok().flatten() else {
                return;
            };
            let idempotent_success = successful_command_ack(&idempotent_reply, "reply-reconnect");
            let _ = observed.send((replayed_success.load(Ordering::Acquire), idempotent_success));
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

fn full_snapshot_command(header: &FrameHeader, command_id: &str) -> ServerFrame {
    ServerFrame {
        payload: Some(server_frame::Payload::Command(Command {
            header: Some(reply_header(header)),
            command_id: command_id.to_owned(),
            payload: Some(command::Payload::RequestFullSnapshot(
                RequestFullSnapshotCmd {},
            )),
        })),
    }
}

fn successful_command_ack(frame: &ClientFrame, command_id: &str) -> bool {
    matches!(
        frame.payload.as_ref(),
        Some(client_frame::Payload::CommandAck(ack))
            if ack.command_id == command_id && ack.success
    )
}

#[tokio::test]
async fn final_command_reply_replays_after_half_close_and_duplicate_id_is_idempotent() {
    let source = Arc::new(CommandSource::new(52));
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let executor = RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry {
            datasource_id: 52,
            weak_ref: Arc::downgrade(&monitorable),
        }])),
        1,
        8,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let final_chunk_acked = Arc::new(Notify::new());
    let (observed_tx, mut observed_rx) = mpsc::unbounded_channel();
    let service = CommandReplyReconnectProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        final_chunk_acked: Arc::clone(&final_chunk_acked),
        observed: observed_tx,
        replayed_success: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let client = GrpcMetricsClient::connect_with_command_executor(
        grpc_config(&endpoint, "reply-reconnect"),
        Some(executor),
    )
    .await
    .expect("client connects");
    tokio::time::timeout(TEST_TIMEOUT, final_chunk_acked.notified())
        .await
        .expect("server ACKs final chunk before half-close");
    let observed = tokio::time::timeout(TEST_TIMEOUT, observed_rx.recv())
        .await
        .expect("server observes replay and idempotent duplicate reply")
        .expect("command reply result");
    assert_eq!(observed, (true, true));
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("durable command reply is confirmed after reconnect");
    shutdown.cancel();
    server_handle
        .await
        .expect("probe joins")
        .expect("probe stops");
}

#[derive(Clone)]
struct SequentialCommandsProbe {
    connections: Arc<AtomicUsize>,
    command_results: mpsc::UnboundedSender<(String, bool)>,
    live_observed: mpsc::UnboundedSender<u64>,
}

#[tonic::async_trait]
impl MetricsIngest for SequentialCommandsProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let command_results = self.command_results.clone();
        let live_observed = self.live_observed.clone();
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
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
                        require_full_snapshot: false,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            if connection < 2 {
                let command_id = format!("sequential-command-{}", connection + 1);
                if tx
                    .send(Ok(full_snapshot_command(&header, &command_id)))
                    .await
                    .is_err()
                {
                    return;
                }
                let Some(chunk) = inbound.message().await.ok().flatten() else {
                    return;
                };
                let (sequence, _) = full_chunk_identity(&chunk);
                if tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::BatchAck(BatchAck {
                            header: Some(reply_header(&header)),
                            accepted_sequence: sequence,
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
                let Some(reply) = inbound.message().await.ok().flatten() else {
                    return;
                };
                let success = successful_command_ack(&reply, &command_id);
                if connection == 0 {
                    let _ = command_results.send((command_id, success));
                    // The peer consumed CommandAck but half-closes before reading Goodbye.
                    return;
                }
                let Some(goodbye) = inbound.message().await.ok().flatten() else {
                    return;
                };
                assert!(matches!(
                    goodbye.payload,
                    Some(client_frame::Payload::Goodbye(_))
                ));
                let _ = command_results.send((command_id, success));
                return;
            }

            let Some(live) = inbound.message().await.ok().flatten() else {
                return;
            };
            let sequence = frame_header(&live).expect("live header").sequence;
            let _ = live_observed.send(sequence);
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&header)),
                        accepted_sequence: sequence,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn live_client_completes_two_different_commands_then_resumes_batches() {
    let source = Arc::new(CommandSource::new(61));
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let executor = RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry {
            datasource_id: 61,
            weak_ref: Arc::downgrade(&monitorable),
        }])),
        1,
        8,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let (command_tx, mut command_rx) = mpsc::unbounded_channel();
    let (live_tx, mut live_rx) = mpsc::unbounded_channel();
    let service = SequentialCommandsProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        command_results: command_tx,
        live_observed: live_tx,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let client = GrpcMetricsClient::connect_with_command_executor(
        grpc_config(&endpoint, "sequential-commands"),
        Some(executor),
    )
    .await
    .expect("client connects");
    let first = tokio::time::timeout(TEST_TIMEOUT, command_rx.recv())
        .await
        .expect("first command completes while producer stays open")
        .expect("first command result");
    let second = tokio::time::timeout(TEST_TIMEOUT, command_rx.recv())
        .await
        .expect("second command is not blocked by the first")
        .expect("second command result");
    assert_eq!(
        [first, second],
        [
            ("sequential-command-1".to_owned(), true),
            ("sequential-command-2".to_owned(), true),
        ]
    );
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(7, 15)],
            is_full_snapshot: false,
        })
        .await
        .expect("live producer remains open after both commands");
    client.flush(TEST_TIMEOUT).await.expect("live batch ACKed");
    assert_eq!(
        tokio::time::timeout(TEST_TIMEOUT, live_rx.recv())
            .await
            .expect("live traffic resumes after command barriers")
            .expect("live sequence"),
        3
    );
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("client shutdown");
    shutdown.cancel();
    server_handle
        .await
        .expect("probe joins")
        .expect("probe stops");
}

#[derive(Clone)]
struct InterleavedCommandProbe {
    connections: Arc<AtomicUsize>,
    command_b_sent: Arc<Notify>,
    command_b_result: mpsc::UnboundedSender<bool>,
    live_observed: mpsc::UnboundedSender<u64>,
}

#[tonic::async_trait]
impl MetricsIngest for InterleavedCommandProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let command_b_sent = Arc::clone(&self.command_b_sent);
        let command_b_result = self.command_b_result.clone();
        let live_observed = self.live_observed.clone();
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
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
                        require_full_snapshot: false,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            if connection == 0 {
                if tx
                    .send(Ok(full_snapshot_command(&header, "interleaved-a")))
                    .await
                    .is_err()
                {
                    return;
                }
                let Some(chunk_a) = inbound.message().await.ok().flatten() else {
                    return;
                };
                let (sequence_a, _) = full_chunk_identity(&chunk_a);
                if tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::BatchAck(BatchAck {
                            header: Some(reply_header(&header)),
                            accepted_sequence: sequence_a,
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
                let Some(reply_a) = inbound.message().await.ok().flatten() else {
                    return;
                };
                assert!(successful_command_ack(&reply_a, "interleaved-a"));
                if tx
                    .send(Ok(full_snapshot_command(&header, "interleaved-b")))
                    .await
                    .is_err()
                {
                    return;
                }
                command_b_sent.notify_waiters();
                let Some(goodbye) = inbound.message().await.ok().flatten() else {
                    return;
                };
                assert!(matches!(
                    goodbye.payload,
                    Some(client_frame::Payload::Goodbye(_))
                ));
                return;
            }
            if connection == 1 {
                let Some(chunk_b) = inbound.message().await.ok().flatten() else {
                    return;
                };
                let (sequence_b, _) = full_chunk_identity(&chunk_b);
                if tx
                    .send(Ok(ServerFrame {
                        payload: Some(server_frame::Payload::BatchAck(BatchAck {
                            header: Some(reply_header(&header)),
                            accepted_sequence: sequence_b,
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
                let Some(reply_b) = inbound.message().await.ok().flatten() else {
                    return;
                };
                let _ = command_b_result.send(successful_command_ack(&reply_b, "interleaved-b"));
                let Some(goodbye) = inbound.message().await.ok().flatten() else {
                    return;
                };
                assert!(matches!(
                    goodbye.payload,
                    Some(client_frame::Payload::Goodbye(_))
                ));
                return;
            }

            let Some(live) = inbound.message().await.ok().flatten() else {
                return;
            };
            let sequence = frame_header(&live).expect("live header").sequence;
            let _ = live_observed.send(sequence);
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&header)),
                        accepted_sequence: sequence,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

struct CountingCommandSource {
    snapshots: AtomicUsize,
}

impl DataSourceMonitorable for CountingCommandSource {
    fn name(&self) -> &str {
        "counting-command-source"
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        DataSourceIdentity {
            id: 71,
            name: "counting-command-source".to_owned(),
            driver_name: Some("postgresql".to_owned()),
        }
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        self.snapshots.fetch_add(1, Ordering::AcqRel);
        Ok(DruidTelemetrySnapshot {
            identity: self.identity(),
            pool_snapshot: PoolSnapshot {
                active_count: 7,
                idle_count: 1,
                max_active: 16,
                max_idle: 8,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 1_755_734_400_000,
        })
    }
}

#[tokio::test]
async fn command_received_before_rotation_goodbye_survives_into_successor_session() {
    let source = Arc::new(CountingCommandSource {
        snapshots: AtomicUsize::new(0),
    });
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let executor = RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry {
            datasource_id: 71,
            weak_ref: Arc::downgrade(&monitorable),
        }])),
        1,
        8,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let command_b_sent = Arc::new(Notify::new());
    let (command_b_tx, mut command_b_rx) = mpsc::unbounded_channel();
    let (live_tx, mut live_rx) = mpsc::unbounded_channel();
    let service = InterleavedCommandProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        command_b_sent: Arc::clone(&command_b_sent),
        command_b_result: command_b_tx,
        live_observed: live_tx,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let client = GrpcMetricsClient::connect_with_command_executor(
        grpc_config(&endpoint, "interleaved-command"),
        Some(executor),
    )
    .await
    .expect("client connects");
    tokio::time::timeout(TEST_TIMEOUT, command_b_sent.notified())
        .await
        .expect("command B is sent before old-stream Goodbye");
    assert!(tokio::time::timeout(TEST_TIMEOUT, command_b_rx.recv())
        .await
        .expect("successor executes preserved command B")
        .expect("command B result"));
    assert_eq!(
        source.snapshots.load(Ordering::Acquire),
        2,
        "commands A and B each sample the registry exactly once"
    );
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(7, 15)],
            is_full_snapshot: false,
        })
        .await
        .expect("live batch remains queued across both rotations");
    client.flush(TEST_TIMEOUT).await.expect("live batch ACKed");
    assert_eq!(
        tokio::time::timeout(TEST_TIMEOUT, live_rx.recv())
            .await
            .expect("live traffic resumes")
            .expect("live sequence"),
        3
    );
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("client shutdown");
    shutdown.cancel();
    server_handle
        .await
        .expect("probe joins")
        .expect("probe stops");
}

struct CommandSource {
    id: u64,
}

impl CommandSource {
    fn new(id: u64) -> Self {
        Self { id }
    }
}

impl DataSourceMonitorable for CommandSource {
    fn name(&self) -> &str {
        "command-source"
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        DataSourceIdentity {
            id: self.id,
            name: format!("command-source-{}", self.id),
            driver_name: Some("postgresql".to_owned()),
        }
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        Ok(DruidTelemetrySnapshot {
            identity: self.identity(),
            pool_snapshot: PoolSnapshot {
                active_count: self.id as u32,
                idle_count: 0,
                max_active: 64,
                max_idle: 0,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 1_755_734_400_000,
        })
    }
}

#[derive(Clone)]
struct WrongHelloVersionProbe;

#[tonic::async_trait]
impl MetricsIngest for WrongHelloVersionProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let Some(hello) = inbound.message().await.ok().flatten() else {
                return;
            };
            let header = frame_header(&hello).expect("hello header");
            let _ = tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::HelloAck(HelloAck {
                        header: Some(reply_header(header)),
                        server_protocol_version: 2,
                        require_full_snapshot: true,
                    })),
                }))
                .await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn hello_ack_server_protocol_version_must_match() {
    let (endpoint, shutdown, server_handle) = start_probe_server(WrongHelloVersionProbe).await;
    let result = GrpcMetricsClient::connect(grpc_config(&endpoint, "wrong-hello-version")).await;
    assert!(
        result.is_err(),
        "server protocol version 2 must be rejected"
    );
    shutdown.cancel();
    server_handle
        .await
        .expect("probe joins")
        .expect("probe stops");
}

#[derive(Clone)]
struct AckThenHoldProbe {
    ack_limit: usize,
}

#[tonic::async_trait]
impl MetricsIngest for AckThenHoldProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(8);
        let ack_limit = self.ack_limit;
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
                        require_full_snapshot: true,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            let mut received = 0;
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if !matches!(frame.payload, Some(client_frame::Payload::SnapshotBatch(_))) {
                    continue;
                }
                received += 1;
                if received <= ack_limit {
                    let sequence = frame_header(&frame).expect("batch header").sequence;
                    if tx
                        .send(Ok(ServerFrame {
                            payload: Some(server_frame::Payload::BatchAck(BatchAck {
                                header: Some(reply_header(&header)),
                                accepted_sequence: sequence,
                            })),
                        }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn shutdown_timeout_reports_only_actual_pending_and_queued_work() {
    let service = AckThenHoldProbe { ack_limit: 5 };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let client = GrpcMetricsClient::connect(grpc_config(&endpoint, "unflushed-count"))
        .await
        .expect("client connects");
    for active in 1..=5 {
        client
            .send_batch(AggregatedBatch {
                entries: vec![orders_entry(active, active + 1)],
                is_full_snapshot: active == 1,
            })
            .await
            .expect("queue historically ACKed batch");
        client.flush(TEST_TIMEOUT).await.expect("historical ACK");
    }
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(6, 7)],
            is_full_snapshot: false,
        })
        .await
        .expect("queue final unacked batch");
    let result = client.shutdown(Duration::from_millis(100)).await;
    assert!(matches!(
        result,
        Err(druid_metrics::MetricsError::ShutdownTimeout { unflushed: 1 })
    ));
    shutdown.cancel();
    server_handle
        .await
        .expect("probe joins")
        .expect("probe stops");
}

#[derive(Clone)]
struct CleanupProbe {
    active_streams: Arc<AtomicUsize>,
    idle: Arc<Notify>,
    goodbyes: Arc<AtomicUsize>,
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
        let goodbyes = Arc::clone(&self.goodbyes);
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
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    goodbyes.fetch_add(1, Ordering::AcqRel);
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn grpc_client_shutdown_joins_transport_and_leaves_no_active_stream() {
    let active_streams = Arc::new(AtomicUsize::new(0));
    let idle = Arc::new(Notify::new());
    let goodbyes = Arc::new(AtomicUsize::new(0));
    let service = CleanupProbe {
        active_streams: Arc::clone(&active_streams),
        idle: Arc::clone(&idle),
        goodbyes: Arc::clone(&goodbyes),
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
    assert_eq!(
        goodbyes.load(Ordering::Acquire),
        1,
        "normal shutdown is confirmed by a server-observed ClientGoodbye"
    );

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

async fn start_probe_server_at<S>(
    addr: SocketAddr,
    service: S,
) -> (
    CancellationToken,
    tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
)
where
    S: MetricsIngest + Clone + Send + Sync + 'static,
{
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind fixed probe server address");
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
    (shutdown, handle)
}

async fn start_production_server_at(
    addr: SocketAddr,
    service: IngestService,
) -> (
    CancellationToken,
    tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind fixed production server address");
    let shutdown = CancellationToken::new();
    let server_shutdown = shutdown.clone();
    let service = service.with_shutdown_token(shutdown.clone());
    let handle = tokio::spawn(async move {
        Server::builder()
            .add_service(service.into_server())
            .serve_with_incoming_shutdown(
                TcpListenerStream::new(listener),
                server_shutdown.cancelled_owned(),
            )
            .await
    });
    (shutdown, handle)
}

async fn reserve_loopback_addr() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve loopback address");
    listener.local_addr().expect("reserved address")
}

async fn start_http_with_repo(
    repository: MetricsRepository,
) -> (
    SocketAddr,
    CancellationToken,
    tokio::task::JoinHandle<Result<(), std::io::Error>>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind HTTP projection server");
    let addr = listener.local_addr().expect("HTTP projection address");
    let shutdown = CancellationToken::new();
    let server_shutdown = shutdown.clone();
    let app = druid_admin::standalone_router(repository);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(server_shutdown.cancelled_owned())
            .await
    });
    (addr, shutdown, handle)
}

async fn wait_for_active_count(http_addr: SocketAddr, expected: i64) {
    tokio::time::timeout(TEST_TIMEOUT, async {
        loop {
            if let Ok(response) =
                reqwest::get(format!("http://{http_addr}/druid/datasource.json")).await
            {
                if let Ok(body) = response.json::<Value>().await {
                    if body["Content"].as_array().is_some_and(|entries| {
                        entries.len() == 1 && entries[0]["ActiveCount"] == expected
                    }) {
                        return;
                    }
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("expected datasource state becomes visible");
}

struct MutableSource {
    active: AtomicU32,
    idle: AtomicU32,
}

impl MutableSource {
    fn new(active: u32, idle: u32) -> Self {
        Self {
            active: AtomicU32::new(active),
            idle: AtomicU32::new(idle),
        }
    }

    fn set_pool(&self, active: u32, idle: u32) {
        self.active.store(active, Ordering::Release);
        self.idle.store(idle, Ordering::Release);
    }
}

impl DataSourceMonitorable for MutableSource {
    fn name(&self) -> &str {
        "orders"
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {}

    fn identity(&self) -> DataSourceIdentity {
        DataSourceIdentity {
            id: 42,
            name: "orders".to_owned(),
            driver_name: Some("postgresql".to_owned()),
        }
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        Ok(DruidTelemetrySnapshot {
            identity: self.identity(),
            pool_snapshot: PoolSnapshot {
                active_count: self.active.load(Ordering::Acquire),
                idle_count: self.idle.load(Ordering::Acquire),
                max_active: 32,
                max_idle: 24,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 1_755_734_400_000,
        })
    }
}

fn registry_executor(
    source: &Arc<MutableSource>,
) -> (Arc<RwLock<Vec<RegistryEntry>>>, RegistryCommandExecutor) {
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let registry = Arc::new(RwLock::new(vec![RegistryEntry {
        datasource_id: 42,
        weak_ref: Arc::downgrade(&monitorable),
    }]));
    let executor = RegistryCommandExecutor::new(
        Arc::clone(&registry),
        8,
        32,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    (registry, executor)
}

#[tokio::test]
async fn grpc_reconnect_after_fully_acked_batch_keeps_next_sequence_and_updates_rest() {
    let repository = MetricsRepository::new();
    let service = IngestService::new(repository.clone());
    let grpc_addr = reserve_loopback_addr().await;
    let (http_addr, http_shutdown, http_handle) = start_http_with_repo(repository).await;
    let (grpc_shutdown, grpc_handle) = start_production_server_at(grpc_addr, service.clone()).await;
    let client = GrpcMetricsClient::connect(grpc_config(
        &format!("http://{grpc_addr}"),
        "sequence-continuity",
    ))
    .await
    .expect("client connects");
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(1, 2)],
            is_full_snapshot: true,
        })
        .await
        .expect("queue sequence one");
    client
        .flush(TEST_TIMEOUT)
        .await
        .expect("sequence one ACKed");
    wait_for_active_count(http_addr, 1).await;

    grpc_shutdown.cancel();
    grpc_handle
        .await
        .expect("first gRPC joins")
        .expect("first gRPC stops");
    let (grpc_shutdown, grpc_handle) = start_production_server_at(grpc_addr, service).await;
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(2, 3)],
            is_full_snapshot: false,
        })
        .await
        .expect("queue post-reconnect batch");
    client
        .flush(TEST_TIMEOUT)
        .await
        .expect("post-reconnect sequence is ACKed");
    wait_for_active_count(http_addr, 2).await;
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("client shutdown");

    grpc_shutdown.cancel();
    grpc_handle.await.expect("gRPC joins").expect("gRPC stops");
    http_shutdown.cancel();
    http_handle.await.expect("HTTP joins").expect("HTTP stops");
}

#[tokio::test]
async fn admin_restart_without_pending_triggers_registry_full_snapshot() {
    let source = Arc::new(MutableSource::new(3, 4));
    let (_registry, executor) = registry_executor(&source);
    let grpc_addr = reserve_loopback_addr().await;
    let first_repository = MetricsRepository::new();
    let first_service = IngestService::new(first_repository.clone());
    let (first_http_addr, first_http_shutdown, first_http_handle) =
        start_http_with_repo(first_repository).await;
    let (first_grpc_shutdown, first_grpc_handle) =
        start_production_server_at(grpc_addr, first_service).await;
    let client = GrpcMetricsClient::connect_with_command_executor(
        grpc_config(&format!("http://{grpc_addr}"), "restart-no-pending"),
        Some(executor),
    )
    .await
    .expect("client connects");
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(3, 4)],
            is_full_snapshot: true,
        })
        .await
        .expect("initial full snapshot");
    client.flush(TEST_TIMEOUT).await.expect("initial ACK");
    wait_for_active_count(first_http_addr, 3).await;

    first_grpc_shutdown.cancel();
    first_grpc_handle
        .await
        .expect("first gRPC joins")
        .expect("first gRPC stops");
    first_http_shutdown.cancel();
    first_http_handle
        .await
        .expect("first HTTP joins")
        .expect("first HTTP stops");
    source.set_pool(7, 8);
    let restarted_repository = MetricsRepository::new();
    let restarted_service = IngestService::new(restarted_repository.clone());
    let (restarted_http_addr, restarted_http_shutdown, restarted_http_handle) =
        start_http_with_repo(restarted_repository).await;
    let (restarted_grpc_shutdown, restarted_grpc_handle) =
        start_production_server_at(grpc_addr, restarted_service).await;

    wait_for_active_count(restarted_http_addr, 7).await;
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("client converges and shuts down");
    restarted_grpc_shutdown.cancel();
    restarted_grpc_handle
        .await
        .expect("restarted gRPC joins")
        .expect("restarted gRPC stops");
    restarted_http_shutdown.cancel();
    restarted_http_handle
        .await
        .expect("restarted HTTP joins")
        .expect("restarted HTTP stops");
}

#[derive(Clone)]
struct HoldSecondAckProbe {
    second_received: Arc<Notify>,
    disconnect: CancellationToken,
}

#[tonic::async_trait]
impl MetricsIngest for HoldSecondAckProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(4);
        let second_received = Arc::clone(&self.second_received);
        let disconnect = self.disconnect.clone();
        tokio::spawn(async move {
            let Some(hello) = inbound.message().await.ok().flatten() else {
                return;
            };
            let hello_header = frame_header(&hello).expect("hello header").clone();
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::HelloAck(HelloAck {
                        header: Some(reply_header(&hello_header)),
                        server_protocol_version: 1,
                        require_full_snapshot: true,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            let Some(first) = inbound.message().await.ok().flatten() else {
                return;
            };
            let first_sequence = frame_header(&first).expect("first batch header").sequence;
            if tx
                .send(Ok(ServerFrame {
                    payload: Some(server_frame::Payload::BatchAck(BatchAck {
                        header: Some(reply_header(&hello_header)),
                        accepted_sequence: first_sequence,
                    })),
                }))
                .await
                .is_err()
            {
                return;
            }
            let Some(_second) = inbound.message().await.ok().flatten() else {
                return;
            };
            second_received.notify_waiters();
            disconnect.cancelled().await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn admin_restart_with_pending_incremental_supersedes_it_with_monotonic_full_snapshot() {
    let source = Arc::new(MutableSource::new(2, 3));
    let (_registry, executor) = registry_executor(&source);
    let grpc_addr = reserve_loopback_addr().await;
    let second_received = Arc::new(Notify::new());
    let disconnect = CancellationToken::new();
    let probe = HoldSecondAckProbe {
        second_received: Arc::clone(&second_received),
        disconnect: disconnect.clone(),
    };
    let (probe_shutdown, probe_handle) = start_probe_server_at(grpc_addr, probe).await;
    let client = GrpcMetricsClient::connect_with_command_executor(
        grpc_config(&format!("http://{grpc_addr}"), "restart-with-pending"),
        Some(executor),
    )
    .await
    .expect("client connects");
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(2, 3)],
            is_full_snapshot: true,
        })
        .await
        .expect("initial full snapshot");
    client.flush(TEST_TIMEOUT).await.expect("first frame ACKed");
    source.set_pool(9, 10);
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(8, 8)],
            is_full_snapshot: false,
        })
        .await
        .expect("incremental enters pending window");
    tokio::time::timeout(TEST_TIMEOUT, second_received.notified())
        .await
        .expect("old server receives incremental before restart");
    disconnect.cancel();
    probe_shutdown.cancel();
    probe_handle
        .await
        .expect("probe joins")
        .expect("probe stops");

    let restarted_repository = MetricsRepository::new();
    let restarted_service = IngestService::new(restarted_repository.clone());
    let (http_addr, http_shutdown, http_handle) =
        start_http_with_repo(restarted_repository.clone()).await;
    let (grpc_shutdown, grpc_handle) =
        start_production_server_at(grpc_addr, restarted_service).await;
    wait_for_active_count(http_addr, 9).await;
    let entries = restarted_repository.all_datasources();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].sequence, 3,
        "authoritative full snapshot preserves monotonic sequence after superseding sequence 2"
    );
    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("resynced client shuts down without timeout");

    grpc_shutdown.cancel();
    grpc_handle.await.expect("gRPC joins").expect("gRPC stops");
    http_shutdown.cancel();
    http_handle.await.expect("HTTP joins").expect("HTTP stops");
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
