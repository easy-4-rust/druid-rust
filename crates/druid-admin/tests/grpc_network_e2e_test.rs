//! Real TCP boundary tests for the shared `druid_metrics.v1` bidi protocol.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use base64::Engine as _;
use druid::stats::{
    DataSourceIdentity, DataSourceMonitorable, DruidTelemetrySnapshot, PoolSnapshot,
    SnapshotUnavailable,
};
use druid_admin::config::{AdminAuthMode, AdminServerConfig};
use druid_admin::ingest::IngestService;
use druid_admin::{production_router_with_command_broker, CommandBroker, MetricsRepository};
use druid_metrics::aggregator::AggregatedBatch;
use druid_metrics::grpc_client::DEFAULT_ACK_WINDOW_CAPACITY;
use druid_metrics::grpc_client::{GrpcAuthorization, GrpcClientConfig, GrpcMetricsClient};
use druid_metrics::protocol::metrics_ingest_client::MetricsIngestClient;
use druid_metrics::protocol::metrics_ingest_server::{MetricsIngest, MetricsIngestServer};
use druid_metrics::protocol::{
    client_frame, command, server_frame, BatchAck, ClientFrame, ClientGoodbye, ClientHeartbeat,
    ClientHello, Command, FrameHeader, HelloAck, PoolSnapshotMsg, RequestFullSnapshotCmd,
    ResetStatsCmd, ResyncRequired, ServerFrame, SnapshotBatch, SnapshotEntry, SnapshotTransaction,
    SqlStatMsg,
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

#[test]
fn grpc_client_builder_defaults_ack_window_to_256_and_rejects_zero() {
    let endpoint = Endpoint::from_static("http://127.0.0.1:9090");
    let config = GrpcClientConfig::builder(
        endpoint.clone(),
        "orders-runtime",
        "instance-default-window",
        "boot-default-window",
    )
    .build()
    .expect("default loopback gRPC config is valid");
    assert_eq!(DEFAULT_ACK_WINDOW_CAPACITY, 256);
    assert_eq!(config.ack_window_capacity, DEFAULT_ACK_WINDOW_CAPACITY);
    let invalid = GrpcClientConfig::builder(
        endpoint,
        "orders-runtime",
        "instance-zero-window",
        "boot-zero-window",
    )
    .ack_window_capacity(0)
    .build();
    assert!(invalid.is_err());
}

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
                snapshot_transaction: is_full_snapshot.then(|| SnapshotTransaction {
                    transaction_id: format!("single-{sequence}"),
                    chunk_index: 0,
                    chunk_count: 1,
                    end: true,
                }),
            })),
        })
        .await
        .expect("send SnapshotBatch");
    match next_server_frame(&mut stream.inbound).await.payload {
        Some(server_frame::Payload::BatchAck(ack)) => ack.accepted_sequence,
        other => panic!("expected BatchAck, got {other:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
async fn send_snapshot_transaction_chunk(
    stream: &mut NetworkStream,
    identity: &StreamIdentity,
    sequence: u64,
    transaction_id: &str,
    chunk_index: u32,
    chunk_count: u32,
    end: bool,
    entries: Vec<SnapshotEntry>,
) -> ServerFrame {
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(identity.header(sequence)),
                entries,
                is_full_snapshot: true,
                snapshot_transaction: Some(SnapshotTransaction {
                    transaction_id: transaction_id.to_owned(),
                    chunk_index,
                    chunk_count,
                    end,
                }),
            })),
        })
        .await
        .expect("send authoritative snapshot transaction chunk");
    next_server_frame(&mut stream.inbound).await
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
        authorization: None,
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
async fn authoritative_snapshot_replaces_only_after_end_and_empty_snapshot_clears_stream() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("authoritative-replace", 12_001);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    let mut second = orders_entry(8, 14);
    second.datasource_id = 43;
    second.datasource_name = "legacy".to_owned();
    let initial = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        1,
        "initial-transaction",
        0,
        1,
        true,
        vec![orders_entry(7, 15), second],
    )
    .await;
    assert!(matches!(
        initial.payload,
        Some(server_frame::Payload::BatchAck(BatchAck {
            accepted_sequence: 1,
            ..
        }))
    ));
    assert_eq!(datasource_content(fixture.http_addr).await.len(), 2);

    let first_chunk = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        2,
        "replacement-transaction",
        0,
        2,
        false,
        vec![orders_entry(9, 13)],
    )
    .await;
    assert!(matches!(
        first_chunk.payload,
        Some(server_frame::Payload::BatchAck(BatchAck {
            accepted_sequence: 2,
            ..
        }))
    ));
    let visible_before_end = datasource_content(fixture.http_addr).await;
    assert_eq!(visible_before_end.len(), 2);
    assert!(visible_before_end.iter().any(|item| item["Identity"] == 43));
    assert!(visible_before_end
        .iter()
        .any(|item| item["Identity"] == 42 && item["ActiveCount"] == 7));

    let mut replacement_second = orders_entry(4, 20);
    replacement_second.datasource_id = 44;
    replacement_second.datasource_name = "replacement".to_owned();
    let end = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        3,
        "replacement-transaction",
        1,
        2,
        true,
        vec![replacement_second],
    )
    .await;
    assert!(matches!(
        end.payload,
        Some(server_frame::Payload::BatchAck(BatchAck {
            accepted_sequence: 3,
            ..
        }))
    ));
    let replaced = datasource_content(fixture.http_addr).await;
    assert_eq!(replaced.len(), 2);
    assert!(replaced
        .iter()
        .any(|item| item["Identity"] == 42 && item["ActiveCount"] == 9));
    assert!(replaced.iter().any(|item| item["Identity"] == 44));
    assert!(!replaced.iter().any(|item| item["Identity"] == 43));

    let empty = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        4,
        "empty-transaction",
        0,
        1,
        true,
        Vec::new(),
    )
    .await;
    assert!(matches!(
        empty.payload,
        Some(server_frame::Payload::BatchAck(BatchAck {
            accepted_sequence: 4,
            ..
        }))
    ));
    assert!(datasource_content(fixture.http_addr).await.is_empty());

    drop(stream.tx);
    fixture.shutdown().await;
}

#[tokio::test]
async fn incomplete_snapshot_survives_disconnect_without_partial_visibility() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("authoritative-resume", 12_002);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    let initial = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        1,
        "resume-initial",
        0,
        1,
        true,
        vec![orders_entry(7, 15)],
    )
    .await;
    assert!(matches!(
        initial.payload,
        Some(server_frame::Payload::BatchAck(_))
    ));

    let first = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        2,
        "resume-replacement",
        0,
        2,
        false,
        vec![orders_entry(11, 10)],
    )
    .await;
    assert!(matches!(
        first.payload,
        Some(server_frame::Payload::BatchAck(_))
    ));
    assert!(datasource_content(fixture.http_addr)
        .await
        .iter()
        .any(|item| item["ActiveCount"] == 7));
    drop(stream.tx);
    drop(stream.inbound);

    let mut resumed = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    let mut second = orders_entry(3, 21);
    second.datasource_id = 44;
    let end = send_snapshot_transaction_chunk(
        &mut resumed,
        &identity,
        3,
        "resume-replacement",
        1,
        2,
        true,
        vec![second],
    )
    .await;
    assert!(matches!(
        end.payload,
        Some(server_frame::Payload::BatchAck(_))
    ));
    let visible = datasource_content(fixture.http_addr).await;
    assert_eq!(visible.len(), 2);
    assert!(visible.iter().any(|item| item["ActiveCount"] == 11));
    assert!(visible.iter().any(|item| item["Identity"] == 44));

    drop(resumed.tx);
    fixture.shutdown().await;
}

#[tokio::test]
async fn out_of_order_snapshot_chunk_fails_closed_without_replacing_visible_state() {
    let fixture = AdminFixture::start().await;
    let identity = StreamIdentity::new("authoritative-order", 12_003);
    let mut stream = open_generated_stream(&fixture.grpc_endpoint, &identity).await;
    let initial = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        1,
        "order-initial",
        0,
        1,
        true,
        vec![orders_entry(7, 15)],
    )
    .await;
    assert!(matches!(
        initial.payload,
        Some(server_frame::Payload::BatchAck(_))
    ));

    let invalid = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        2,
        "order-invalid",
        1,
        2,
        true,
        vec![orders_entry(99, 0)],
    )
    .await;
    assert!(matches!(
        invalid.payload,
        Some(server_frame::Payload::Error(ref error)) if error.close_stream
    ));
    let visible = datasource_content(fixture.http_addr).await;
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0]["ActiveCount"], 7);

    fixture.shutdown().await;
}

#[tokio::test]
async fn snapshot_transaction_capacity_is_enforced_before_staging() {
    let repository = MetricsRepository::with_config(
        druid_admin::repository::MetricsRepositoryConfig::try_new(
            4,
            2,
            1,
            Duration::from_secs(60),
            4 * 1024,
        )
        .expect("valid tiny ingest limits"),
    );
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve bounded ingest address");
    let address = reservation.local_addr().expect("bounded ingest address");
    drop(reservation);
    let (shutdown, server_handle) =
        start_production_server_at(address, IngestService::new(repository.clone())).await;
    let identity = StreamIdentity::new("bounded-transaction", 12_004);
    let mut stream = open_generated_stream(&format!("http://{address}"), &identity).await;
    let response = send_snapshot_transaction_chunk(
        &mut stream,
        &identity,
        1,
        "too-many-chunks",
        0,
        3,
        false,
        vec![orders_entry(1, 1)],
    )
    .await;
    assert!(matches!(
        response.payload,
        Some(server_frame::Payload::Error(ref error)) if error.close_stream
    ));
    assert!(repository.is_empty());
    shutdown.cancel();
    drop(stream.tx);
    server_handle
        .await
        .expect("bounded ingest server joins")
        .expect("bounded ingest server stops");
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
async fn successor_stream_fences_old_epoch_before_it_can_overwrite_repository() {
    let repository = MetricsRepository::new();
    let service = IngestService::new(repository.clone());
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve fenced ingest address");
    let address = reservation.local_addr().expect("fenced ingest address");
    drop(reservation);
    let (shutdown, server_handle) = start_production_server_at(address, service).await;
    let endpoint = format!("http://{address}");
    let first_identity = StreamIdentity::new("epoch-fence", 51_001);
    let mut first = open_generated_stream(&endpoint, &first_identity).await;
    assert_eq!(
        send_batch_and_expect_ack(&mut first, &first_identity, 1, orders_entry(1, 1), true).await,
        1
    );

    let mut successor_identity = first_identity.clone();
    successor_identity.stream_epoch += 1;
    let mut successor = open_generated_stream(&endpoint, &successor_identity).await;
    assert_eq!(
        send_batch_and_expect_ack(
            &mut successor,
            &successor_identity,
            1,
            orders_entry(7, 7),
            true,
        )
        .await,
        1
    );

    let _ = first
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::SnapshotBatch(SnapshotBatch {
                header: Some(first_identity.header(2)),
                entries: vec![orders_entry(99, 99)],
                is_full_snapshot: false,
                snapshot_transaction: None,
            })),
        })
        .await;
    let old_end = tokio::time::timeout(TEST_TIMEOUT, first.inbound.message())
        .await
        .expect("old stream ends after successor attaches")
        .expect("old stream closes without transport error");
    assert!(
        old_end.is_none(),
        "old epoch must no longer receive snapshot ACKs"
    );
    let visible = repository.all_datasources();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].datasource.active_count, 7);

    drop(first.tx);
    drop(successor.tx);
    shutdown.cancel();
    server_handle
        .await
        .expect("fenced ingest server joins")
        .expect("fenced ingest server stops");
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
    let service_id =
        druid_metrics::ServiceIdentity::try_new("orders-runtime", "instance-health", "boot-health")
            .unwrap()
            .service_id()
            .to_owned();
    assert!(metrics.contains(&format!(
        "druid_admin_datasource_active_count{{service_id=\"{service_id}\",service_name=\"orders-runtime\",instance_id=\"instance-health\",boot_id=\"boot-health\",datasource=\"42\",datasource_name=\"orders\",db_type=\"postgresql\"}} 7"
    )));

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
                snapshot_transaction: Some(SnapshotTransaction {
                    transaction_id: "atomic-batch".to_owned(),
                    chunk_index: 0,
                    chunk_count: 1,
                    end: true,
                }),
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
                snapshot_transaction: Some(SnapshotTransaction {
                    transaction_id: "atomic-rejection".to_owned(),
                    chunk_index: 0,
                    chunk_count: 1,
                    end: true,
                }),
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
    assert!(metrics.contains("datasource_name=\"orders\\\"\\\\\\nmalicious_metric 1\""));
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
                snapshot_transaction: None,
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
                snapshot_transaction: None,
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
                snapshot_transaction: None,
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
                    snapshot_transaction: Some(SnapshotTransaction {
                        transaction_id: format!("invalid-sequence-{sequence}"),
                        chunk_index: 0,
                        chunk_count: 1,
                        end: true,
                    }),
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
    authorization_results: mpsc::UnboundedSender<(usize, bool)>,
}

#[tonic::async_trait]
impl MetricsIngest for ReconnectProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let authenticated = request
            .metadata()
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            == Some("Basic YWRtaW46c2VjcmV0");
        let _ = self.authorization_results.send((connection, authenticated));
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
    let (authorization_tx, mut authorization_rx) = mpsc::unbounded_channel();
    let service = ReconnectProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        first_frame: Arc::new(Mutex::new(None)),
        replay_results: replay_tx,
        authorization_results: authorization_tx,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let mut config = grpc_config(&endpoint, "reconnect");
    config.authorization = Some(GrpcAuthorization::basic("admin", "secret"));
    let client = GrpcMetricsClient::connect(config)
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
    assert_eq!(
        [
            tokio::time::timeout(TEST_TIMEOUT, authorization_rx.recv())
                .await
                .expect("initial authorization observed")
                .expect("initial authorization result"),
            tokio::time::timeout(TEST_TIMEOUT, authorization_rx.recv())
                .await
                .expect("reconnect authorization observed")
                .expect("reconnect authorization result"),
        ],
        [(0, true), (1, true)],
        "initial and reconnect requests carry the same production credential"
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
        Arc::new(RwLock::new(vec![RegistryEntry::new(
            42,
            Arc::downgrade(&monitorable),
        )])),
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
            .map(|(index, source)| RegistryEntry::new(41 + index as u64, Arc::downgrade(source)))
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
        Arc::new(RwLock::new(vec![RegistryEntry::new(
            52,
            Arc::downgrade(&monitorable),
        )])),
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
        Arc::new(RwLock::new(vec![RegistryEntry::new(
            61,
            Arc::downgrade(&monitorable),
        )])),
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

const SUCCESSOR_COMMAND_WINDOW: usize = 2;
const SUCCESSOR_COMMAND_BURST: usize = SUCCESSOR_COMMAND_WINDOW + 1;

#[derive(Clone)]
struct SuccessorBackpressureProbe {
    connections: Arc<AtomicUsize>,
    successors_sent: Arc<Notify>,
    command_results: mpsc::UnboundedSender<(String, bool)>,
    live_observed: mpsc::UnboundedSender<u64>,
}

#[tonic::async_trait]
impl MetricsIngest for SuccessorBackpressureProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let successors_sent = Arc::clone(&self.successors_sent);
        let command_results = self.command_results.clone();
        let live_observed = self.live_observed.clone();
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(SUCCESSOR_COMMAND_BURST + 2);
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

            if connection == 0
                && tx
                    .send(Ok(full_snapshot_command(&header, "backpressure-command-0")))
                    .await
                    .is_err()
            {
                return;
            }

            if connection <= SUCCESSOR_COMMAND_BURST {
                let command_id = format!("backpressure-command-{connection}");
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
                let _ = command_results.send((command_id, success));

                if connection == 0 {
                    for successor in 1..=SUCCESSOR_COMMAND_BURST {
                        if tx
                            .send(Ok(full_snapshot_command(
                                &header,
                                &format!("backpressure-command-{successor}"),
                            )))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    successors_sent.notify_waiters();
                }

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

#[tokio::test]
async fn validated_w_plus_one_commands_survive_teardown_backpressure_and_live_traffic_resumes() {
    let source = Arc::new(CountingCommandSource {
        snapshots: AtomicUsize::new(0),
    });
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let executor = RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry::new(
            71,
            Arc::downgrade(&monitorable),
        )])),
        1,
        8,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let successors_sent = Arc::new(Notify::new());
    let (command_tx, mut command_rx) = mpsc::unbounded_channel();
    let (live_tx, mut live_rx) = mpsc::unbounded_channel();
    let service = SuccessorBackpressureProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        successors_sent: Arc::clone(&successors_sent),
        command_results: command_tx,
        live_observed: live_tx,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let mut config = grpc_config(&endpoint, "successor-backpressure");
    config.ack_window_capacity = SUCCESSOR_COMMAND_WINDOW;
    let client = GrpcMetricsClient::connect_with_command_executor(config, Some(executor))
        .await
        .expect("client connects");

    tokio::time::timeout(TEST_TIMEOUT, successors_sent.notified())
        .await
        .expect("server queues W+1 commands before reading old-stream Goodbye");
    let mut results = Vec::new();
    for _ in 0..=SUCCESSOR_COMMAND_BURST {
        results.push(
            tokio::time::timeout(TEST_TIMEOUT, command_rx.recv())
                .await
                .expect("every validated command reaches a successor session")
                .expect("command result"),
        );
    }
    assert_eq!(
        results,
        vec![
            ("backpressure-command-0".to_owned(), true),
            ("backpressure-command-1".to_owned(), true),
            ("backpressure-command-2".to_owned(), true),
            ("backpressure-command-3".to_owned(), true),
        ],
        "the W-capacity receiver plus its reader-held frame migrate in order"
    );
    assert_eq!(
        source.snapshots.load(Ordering::Acquire),
        SUCCESSOR_COMMAND_BURST + 1,
        "each accepted command executes exactly once"
    );

    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(7, 15)],
            is_full_snapshot: false,
        })
        .await
        .expect("live producer remains usable after W+1 command rotations");
    client.flush(TEST_TIMEOUT).await.expect("live batch ACKed");
    assert_eq!(
        tokio::time::timeout(TEST_TIMEOUT, live_rx.recv())
            .await
            .expect("live traffic resumes")
            .expect("live sequence"),
        5
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

#[derive(Clone, Copy)]
enum MigrationViolation {
    Overflow,
    ConflictingPayload,
}

#[derive(Clone)]
struct MigrationViolationProbe {
    connections: Arc<AtomicUsize>,
    violation: MigrationViolation,
    violation_sent: mpsc::UnboundedSender<()>,
    successor_executed: mpsc::UnboundedSender<()>,
}

#[tonic::async_trait]
impl MetricsIngest for MigrationViolationProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        let violation = self.violation;
        let violation_sent = self.violation_sent.clone();
        let successor_executed = self.successor_executed.clone();
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(8);
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
                    .send(Ok(full_snapshot_command(&header, "migration-trigger")))
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
                let Some(command_ack) = inbound.message().await.ok().flatten() else {
                    return;
                };
                assert!(successful_command_ack(&command_ack, "migration-trigger"));
                match violation {
                    MigrationViolation::Overflow => {
                        for index in 0..=SUCCESSOR_COMMAND_WINDOW.saturating_mul(2) {
                            if tx
                                .send(Ok(full_snapshot_command(
                                    &header,
                                    &format!("overflow-command-{index}"),
                                )))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    MigrationViolation::ConflictingPayload => {
                        if tx
                            .send(Ok(full_snapshot_command(&header, "conflicting-command")))
                            .await
                            .is_err()
                            || tx
                                .send(Ok(reset_stats_command(&header, "conflicting-command")))
                                .await
                                .is_err()
                        {
                            return;
                        }
                    }
                }
                let _ = violation_sent.send(());
                while let Some(frame) = inbound.message().await.ok().flatten() {
                    if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                        break;
                    }
                }
                return;
            }

            let Some(successor) = inbound.message().await.ok().flatten() else {
                return;
            };
            if matches!(
                successor.payload,
                Some(client_frame::Payload::SnapshotBatch(_))
            ) {
                let _ = successor_executed.send(());
                let sequence = frame_header(&successor).expect("successor header").sequence;
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
            while let Some(frame) = inbound.message().await.ok().flatten() {
                if matches!(frame.payload, Some(client_frame::Payload::Goodbye(_))) {
                    break;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

fn reset_stats_command(header: &FrameHeader, command_id: &str) -> ServerFrame {
    ServerFrame {
        payload: Some(server_frame::Payload::Command(Command {
            header: Some(reply_header(header)),
            command_id: command_id.to_owned(),
            payload: Some(command::Payload::ResetStats(ResetStatsCmd {
                target_datasource_ids: Vec::new(),
            })),
        })),
    }
}

async fn assert_migration_violation_fails_closed(
    violation: MigrationViolation,
    expected_error: &str,
) {
    let source = Arc::new(CountingCommandSource {
        snapshots: AtomicUsize::new(0),
    });
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let executor = RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry::new(
            71,
            Arc::downgrade(&monitorable),
        )])),
        1,
        8,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let (sent_tx, mut sent_rx) = mpsc::unbounded_channel();
    let (executed_tx, mut executed_rx) = mpsc::unbounded_channel();
    let service = MigrationViolationProbe {
        connections: Arc::new(AtomicUsize::new(0)),
        violation,
        violation_sent: sent_tx,
        successor_executed: executed_tx,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let mut config = grpc_config(&endpoint, "migration-violation");
    config.ack_window_capacity = SUCCESSOR_COMMAND_WINDOW;
    let client = GrpcMetricsClient::connect_with_command_executor(config, Some(executor))
        .await
        .expect("client connects");
    tokio::time::timeout(TEST_TIMEOUT, sent_rx.recv())
        .await
        .expect("server sends migration violation")
        .expect("migration violation signal");
    let error = client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect_err("migration violation must fail closed");
    assert!(error.to_string().contains(expected_error), "{error}");
    assert_eq!(
        source.snapshots.load(Ordering::Acquire),
        1,
        "only the trigger command executes before fail-closed teardown"
    );
    assert!(
        executed_rx.try_recv().is_err(),
        "no successor command executes after migration failure"
    );
    shutdown.cancel();
    tokio::time::timeout(TEST_TIMEOUT, server_handle)
        .await
        .expect("server has no residual task")
        .expect("server joins")
        .expect("server stops");
}

#[tokio::test]
async fn two_w_plus_one_unique_commands_fail_closed_without_successor_execution() {
    assert_migration_violation_fails_closed(MigrationViolation::Overflow, "queue full").await;
}

#[tokio::test]
async fn same_command_id_with_different_payload_fails_closed() {
    assert_migration_violation_fails_closed(
        MigrationViolation::ConflictingPayload,
        "conflicting payload",
    )
    .await;
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
                let duplicate_b = full_snapshot_command(&header, "interleaved-b");
                for frame in [duplicate_b.clone(), duplicate_b] {
                    if tx.send(Ok(frame)).await.is_err() {
                        return;
                    }
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
        Arc::new(RwLock::new(vec![RegistryEntry::new(
            71,
            Arc::downgrade(&monitorable),
        )])),
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
    let registry = Arc::new(RwLock::new(vec![RegistryEntry::new(
        42,
        Arc::downgrade(&monitorable),
    )]));
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
    id: u64,
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
            id: self.id,
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
        id: 84,
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

#[tokio::test]
async fn unavailable_admin_shutdown_reports_exporter_and_client_work_as_unflushed() {
    let (endpoint, shutdown, server_handle) =
        start_probe_server(AckThenHoldProbe { ack_limit: 0 }).await;
    let runtime = DruidMetricsRuntime::start_with_grpc(
        DruidMetricsConfig::builder()
            .sample_interval(Duration::from_secs(3_600))
            .coalesce_window(Duration::from_millis(1))
            .shutdown_timeout(Duration::from_secs(5))
            .build()
            .expect("valid unavailable Admin runtime config"),
        grpc_config(&endpoint, "runtime-unavailable"),
    )
    .await
    .expect("runtime connects before Admin withholds ACKs");
    let source = Arc::new(RuntimeProbe {
        id: 84,
        samples: AtomicUsize::new(0),
        sampled: Notify::new(),
    });
    let _guard = runtime.register(source.clone());
    tokio::time::timeout(TEST_TIMEOUT, async {
        while source.samples.load(Ordering::Acquire) == 0 {
            source.sampled.notified().await;
        }
        while runtime.self_metrics().pending_snapshots() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("snapshot leaves the aggregator and becomes exporter/client-owned");

    let result = runtime.shutdown(Duration::from_millis(100)).await;
    assert!(
        matches!(
            result,
            Err(druid_metrics::MetricsError::ShutdownTimeout { unflushed: 1 })
        ),
        "one batch must have exactly one owner during exporter-to-client handoff: {result:?}"
    );
    shutdown.cancel();
    server_handle
        .await
        .expect("unavailable Admin probe joins")
        .expect("unavailable Admin probe stops");
}

#[tokio::test]
async fn blocked_exporter_to_client_handoff_counts_each_batch_exactly_once() {
    let (endpoint, shutdown, server_handle) =
        start_probe_server(AckThenHoldProbe { ack_limit: 0 }).await;
    let mut client_config = grpc_config(&endpoint, "blocked-handoff-exact");
    client_config.ack_window_capacity = 1;
    let runtime = DruidMetricsRuntime::start_with_grpc(
        DruidMetricsConfig::builder()
            .queue_capacity(4)
            .batch_size(1)
            .sample_interval(Duration::from_millis(50))
            .coalesce_window(Duration::from_millis(1))
            .shutdown_timeout(Duration::from_secs(5))
            .build()
            .expect("valid blocked handoff runtime config"),
        client_config,
    )
    .await
    .expect("blocked handoff runtime connects");
    let sources = (84..87)
        .map(|id| {
            Arc::new(RuntimeProbe {
                id,
                samples: AtomicUsize::new(0),
                sampled: Notify::new(),
            })
        })
        .collect::<Vec<_>>();
    let guards = sources
        .iter()
        .map(|source| runtime.register(source.clone()))
        .collect::<Vec<_>>();
    tokio::time::timeout(TEST_TIMEOUT, async {
        while sources
            .iter()
            .any(|source| source.samples.load(Ordering::Acquire) == 0)
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("all three datasources produce one batch");
    drop(guards);
    tokio::time::timeout(TEST_TIMEOUT, async {
        while runtime.self_metrics().pending_snapshots() != 0
            || runtime.self_metrics().unflushed_work() != 3
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("third batch blocks in exporter-to-client handoff");

    let result = runtime.shutdown(Duration::from_millis(100)).await;
    assert!(matches!(
        result,
        Err(druid_metrics::MetricsError::ShutdownTimeout { unflushed: 3 })
    ), "three batches across window, client queue and blocked handoff need three disjoint owners: {result:?}");
    shutdown.cancel();
    server_handle
        .await
        .expect("blocked handoff probe joins")
        .expect("blocked handoff probe stops");
}

#[tokio::test]
async fn admin_shutdown_cancels_and_joins_every_owned_ingest_stream() {
    let service = IngestService::new(MetricsRepository::new());
    let observer = service.clone();
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve owned stream address");
    let address = reservation.local_addr().expect("owned stream address");
    drop(reservation);
    let (shutdown, server_handle) = start_production_server_at(address, service).await;
    let identity = StreamIdentity::new("owned-shutdown", 91_001);
    let mut stream = open_generated_stream(&format!("http://{address}"), &identity).await;
    assert_eq!(observer.active_streams(), 1);

    shutdown.cancel();
    tokio::time::timeout(TEST_TIMEOUT, async {
        while observer.active_streams() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("shutdown cancellation releases the owned processing task");
    let response_end = tokio::time::timeout(TEST_TIMEOUT, stream.inbound.message())
        .await
        .expect("response stream ends on shutdown")
        .expect("response shutdown has no transport status error");
    assert!(response_end.is_none());
    drop(stream.tx);
    server_handle
        .await
        .expect("owned stream server joins")
        .expect("owned stream server stops");
}

#[tokio::test]
async fn admin_shutdown_releases_command_waiter_without_an_ack() {
    let broker = CommandBroker::new(8, Duration::from_secs(10));
    let service = IngestService::new(MetricsRepository::new()).with_command_broker(broker.clone());
    let observer = service.clone();
    let address = reserve_loopback_addr().await;
    let (shutdown, server_handle) = start_production_server_at(address, service).await;
    let identity = StreamIdentity::new("pending-ack-shutdown", 91_011);
    let mut stream = open_generated_stream(&format!("http://{address}"), &identity).await;
    assert_eq!(broker.live_stream_count(), 1);

    let issue_broker = broker.clone();
    let service_name = identity.service_name.clone();
    let instance_id = identity.instance_id.clone();
    let boot_id = identity.boot_id.clone();
    let mut pending = tokio::spawn(async move {
        issue_broker
            .issue_full_snapshot(&service_name, &instance_id, &boot_id, "pending-shutdown")
            .await
    });
    let delivered = next_server_frame(&mut stream.inbound).await;
    let Some(server_frame::Payload::Command(command)) = delivered.payload else {
        panic!("expected unacknowledged command, got {delivered:?}");
    };
    assert_eq!(command.command_id, "pending-shutdown");
    assert!(!pending.is_finished(), "the client has not sent an ACK");

    shutdown.cancel();
    let result =
        if let Ok(result) = tokio::time::timeout(Duration::from_secs(1), &mut pending).await {
            result.expect("command waiter task joins")
        } else {
            pending.abort();
            panic!("shutdown must release an unacknowledged command waiter within one second");
        };
    let pending_error = result.expect_err("shutdown cannot report a successful command ACK");
    assert!(pending_error.contains("shutting down"), "{pending_error}");
    let after_shutdown = broker
        .issue_full_snapshot(
            &identity.service_name,
            &identity.instance_id,
            &identity.boot_id,
            "after-shutdown",
        )
        .await
        .expect_err("shutdown rejects new command admission");
    assert!(after_shutdown.contains("shutting down"), "{after_shutdown}");
    tokio::time::timeout(TEST_TIMEOUT, server_handle)
        .await
        .expect("server joins after pending ACK shutdown")
        .expect("server task joins")
        .expect("server stops cleanly");
    assert_eq!(observer.active_streams(), 0);
    assert_eq!(broker.live_stream_count(), 0);
    drop(stream);
}

#[tokio::test]
async fn production_builder_order_binds_shared_command_broker_shutdown() {
    let shutdown = CancellationToken::new();
    let broker = CommandBroker::new(1, Duration::from_secs(10));
    let _service = IngestService::new(MetricsRepository::new())
        .with_shutdown_token(shutdown.clone())
        .with_command_broker(broker.clone());
    shutdown.cancel();

    let error = broker
        .issue_full_snapshot("orders-runtime", "instance-one", "boot-one", "after-stop")
        .await
        .expect_err("production broker must reject commands after server shutdown");
    assert!(error.contains("shutting down"), "{error}");
}

#[tokio::test]
async fn dropping_tonic_response_detaches_exact_broker_sender_and_reuses_capacity() {
    let broker = CommandBroker::new(1, Duration::from_millis(100));
    let service = IngestService::new(MetricsRepository::new()).with_command_broker(broker.clone());
    let observer = service.clone();
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve response-drop address");
    let address = reservation.local_addr().expect("response-drop address");
    drop(reservation);
    let (shutdown, server_handle) = start_production_server_at(address, service).await;
    let first_identity = StreamIdentity::new("response-drop-first", 91_101);
    let first_stream = open_generated_stream(&format!("http://{address}"), &first_identity).await;
    assert_eq!(broker.live_stream_count(), 1);
    assert_eq!(observer.active_streams(), 1);

    drop(first_stream.inbound);
    drop(first_stream.tx);
    tokio::time::timeout(TEST_TIMEOUT, async {
        while broker.live_stream_count() != 0 || observer.active_streams() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("response Drop detaches broker and aborts processing task");
    assert!(
        broker
            .issue_full_snapshot(
                &first_identity.service_name,
                &first_identity.instance_id,
                &first_identity.boot_id,
                "stale-response-drop",
            )
            .await
            .is_err(),
        "the detached identity cannot receive another command"
    );

    let second_identity = StreamIdentity::new("response-drop-second", 91_102);
    let second_stream = open_generated_stream(&format!("http://{address}"), &second_identity).await;
    assert_eq!(
        broker.live_stream_count(),
        1,
        "capacity one is reusable by a successor identity"
    );
    drop(second_stream.inbound);
    drop(second_stream.tx);
    shutdown.cancel();
    server_handle
        .await
        .expect("response-drop server joins")
        .expect("response-drop server stops");
}

#[tokio::test]
async fn full_capacity_command_journal_reconnects_after_hello_ack_without_partial_replay() {
    const CAPACITY: usize = 64;

    let broker = CommandBroker::new(CAPACITY, Duration::from_secs(30));
    let service = IngestService::new(MetricsRepository::new()).with_command_broker(broker.clone());
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve saturated replay address");
    let address = reservation.local_addr().expect("saturated replay address");
    drop(reservation);
    let (shutdown, server_handle) = start_production_server_at(address, service).await;
    let endpoint = format!("http://{address}");
    let first_identity = StreamIdentity::new("saturated-replay", 91_201);
    let mut first = open_generated_stream(&endpoint, &first_identity).await;
    let mut waiters = Vec::with_capacity(CAPACITY);

    for index in 0..CAPACITY {
        let issue_broker = broker.clone();
        let command_id = format!("replay-{index:02}");
        let service_name = first_identity.service_name.clone();
        let instance_id = first_identity.instance_id.clone();
        let boot_id = first_identity.boot_id.clone();
        waiters.push(tokio::spawn(async move {
            issue_broker
                .issue_full_snapshot(&service_name, &instance_id, &boot_id, &command_id)
                .await
        }));
        let delivered = next_server_frame(&mut first.inbound).await;
        let Some(server_frame::Payload::Command(command)) = delivered.payload else {
            panic!("expected first-session command {index}, got {delivered:?}");
        };
        assert_eq!(command.command_id, format!("replay-{index:02}"));
    }
    let overflow = broker
        .issue_full_snapshot(
            &first_identity.service_name,
            &first_identity.instance_id,
            &first_identity.boot_id,
            "replay-overflow",
        )
        .await
        .expect_err("N+1 journal admission must fail before replay");
    assert!(overflow.contains("broker full"), "{overflow}");

    drop(first.tx);
    tokio::time::timeout(TEST_TIMEOUT, async {
        while broker.live_stream_count() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first saturated stream half-closes and detaches");
    drop(first.inbound);

    let mut second_identity = first_identity.clone();
    second_identity.stream_epoch += 1;
    let mut second = open_generated_stream(&endpoint, &second_identity).await;
    for index in 0..CAPACITY {
        let delivered = next_server_frame(&mut second.inbound).await;
        let Some(server_frame::Payload::Command(command)) = delivered.payload else {
            panic!("expected replayed command {index}, got {delivered:?}");
        };
        assert_eq!(
            command.command_id,
            format!("replay-{index:02}"),
            "full-boundary replay order must be deterministic"
        );
    }

    for waiter in waiters {
        waiter.abort();
    }
    drop(second.tx);
    drop(second.inbound);
    shutdown.cancel();
    server_handle
        .await
        .expect("saturated replay server joins")
        .expect("saturated replay server stops");
}

#[tokio::test]
async fn old_slash_collision_pair_stays_distinct_across_broker_repository_rest_and_prometheus() {
    let repository = MetricsRepository::new();
    let broker = CommandBroker::new(8, TEST_TIMEOUT);
    let grpc_addr = reserve_loopback_addr().await;
    let service = IngestService::new(repository.clone()).with_command_broker(broker.clone());
    let (grpc_shutdown, grpc_handle) = start_production_server_at(grpc_addr, service).await;
    let (http_addr, http_shutdown, http_handle) = start_http_with_repo(repository.clone()).await;

    let exact_component = format!("{}x", "界".repeat(85));
    assert_eq!(exact_component.len(), 256);
    let exact_identity = StreamIdentity {
        service_name: exact_component,
        instance_id: "bounded-instance".to_owned(),
        boot_id: "bounded-boot".to_owned(),
        stream_epoch: 91_400,
    };
    let exact_stream = open_generated_stream(&format!("http://{grpc_addr}"), &exact_identity).await;
    drop(exact_stream.tx);
    drop(exact_stream.inbound);
    tokio::time::timeout(TEST_TIMEOUT, async {
        while broker.live_stream_count() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("exact-budget stream detaches");
    let endpoint = format!("http://{grpc_addr}");
    let first_identity = StreamIdentity {
        service_name: "a/b".to_owned(),
        instance_id: "c".to_owned(),
        boot_id: "boot-碰撞".to_owned(),
        stream_epoch: 91_301,
    };
    let second_identity = StreamIdentity {
        service_name: "a".to_owned(),
        instance_id: "b/c".to_owned(),
        boot_id: "boot-碰撞".to_owned(),
        stream_epoch: 91_301,
    };
    let first_service_id = druid_metrics::ServiceIdentity::try_new(
        &first_identity.service_name,
        &first_identity.instance_id,
        &first_identity.boot_id,
    )
    .unwrap()
    .service_id()
    .to_owned();
    let second_service_id = druid_metrics::ServiceIdentity::try_new(
        &second_identity.service_name,
        &second_identity.instance_id,
        &second_identity.boot_id,
    )
    .unwrap()
    .service_id()
    .to_owned();
    assert_ne!(first_service_id, second_service_id);

    let mut first = open_generated_stream(&endpoint, &first_identity).await;
    let mut second = open_generated_stream(&endpoint, &second_identity).await;
    assert_eq!(broker.live_stream_count(), 2);
    assert_eq!(
        send_batch_and_expect_ack(&mut first, &first_identity, 1, orders_entry(7, 15), true,).await,
        1
    );
    let mut second_entry = orders_entry(9, 13);
    second_entry.sql_stats[0].fingerprint = "fp:collision-second".to_owned();
    assert_eq!(
        send_batch_and_expect_ack(&mut second, &second_identity, 1, second_entry, true,).await,
        1
    );

    let entries = repository.all_datasources();
    assert_eq!(
        entries.len(),
        2,
        "authoritative replacement must key the typed identity, not old slash text"
    );
    let exposed_ids = entries
        .iter()
        .filter_map(|entry| entry.datasource.service_id.clone())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        exposed_ids,
        std::collections::HashSet::from([
            std::sync::Arc::<str>::from(first_service_id.clone()),
            std::sync::Arc::<str>::from(second_service_id.clone()),
        ])
    );

    for (stream, identity, command_id) in [
        (&mut first, &first_identity, "route-first"),
        (&mut second, &second_identity, "route-second"),
    ] {
        let issue_broker = broker.clone();
        let service_name = identity.service_name.clone();
        let instance_id = identity.instance_id.clone();
        let boot_id = identity.boot_id.clone();
        let command_id_owned = command_id.to_owned();
        let issue = tokio::spawn(async move {
            issue_broker
                .issue_full_snapshot(&service_name, &instance_id, &boot_id, &command_id_owned)
                .await
        });
        let delivered = next_server_frame(&mut stream.inbound).await;
        let Some(server_frame::Payload::Command(command)) = delivered.payload else {
            panic!("expected typed-identity routed command, got {delivered:?}");
        };
        assert_eq!(command.command_id, command_id);
        stream
            .tx
            .send(ClientFrame {
                payload: Some(client_frame::Payload::CommandAck(
                    druid_metrics::protocol::CommandAck {
                        header: Some(identity.header(0)),
                        command_id: command_id.to_owned(),
                        success: true,
                        error_message: String::new(),
                    },
                )),
            })
            .await
            .expect("send typed-identity command acknowledgement");
        assert!(issue.await.expect("typed command issue joins").is_ok());
    }

    let first_datasource: Value = reqwest::get(format!(
        "http://{http_addr}/druid/api/datasources/42?serviceId={first_service_id}"
    ))
    .await
    .expect("first collision datasource detail")
    .error_for_status()
    .expect("first collision datasource found")
    .json()
    .await
    .expect("first collision datasource JSON");
    let second_datasource: Value = reqwest::get(format!(
        "http://{http_addr}/druid/api/datasources/42?serviceId={second_service_id}"
    ))
    .await
    .expect("second collision datasource detail")
    .error_for_status()
    .expect("second collision datasource found")
    .json()
    .await
    .expect("second collision datasource JSON");
    assert_eq!(first_datasource["Content"][0]["ActiveCount"], 7);
    assert_eq!(first_datasource["Content"][0]["serviceName"], "a/b");
    assert_eq!(second_datasource["Content"][0]["ActiveCount"], 9);
    assert_eq!(second_datasource["Content"][0]["instanceId"], "b/c");

    let first_sql: Value = reqwest::get(format!(
        "http://{http_addr}/druid/api/sql/1?serviceId={first_service_id}&dataSourceId=42"
    ))
    .await
    .expect("first collision SQL detail")
    .error_for_status()
    .expect("first collision SQL found")
    .json()
    .await
    .expect("first collision SQL JSON");
    let second_sql: Value = reqwest::get(format!(
        "http://{http_addr}/druid/api/sql/1?serviceId={second_service_id}&dataSourceId=42"
    ))
    .await
    .expect("second collision SQL detail")
    .error_for_status()
    .expect("second collision SQL found")
    .json()
    .await
    .expect("second collision SQL JSON");
    assert_eq!(first_sql["SQL"], "fp:orders-by-id");
    assert_eq!(second_sql["SQL"], "fp:collision-second");

    let metrics = reqwest::get(format!("http://{http_addr}/metrics"))
        .await
        .expect("collision Prometheus response")
        .error_for_status()
        .expect("collision Prometheus is available")
        .text()
        .await
        .expect("collision Prometheus text");
    assert!(metrics.contains(&format!("service_id=\"{first_service_id}\"")));
    assert!(metrics.contains(&format!("service_id=\"{second_service_id}\"")));
    assert!(metrics.contains("service_name=\"a/b\""));
    assert!(metrics.contains("instance_id=\"b/c\""));

    drop(first.tx);
    drop(first.inbound);
    drop(second.tx);
    drop(second.inbound);
    grpc_shutdown.cancel();
    grpc_handle
        .await
        .expect("collision gRPC joins")
        .expect("collision gRPC stops");
    http_shutdown.cancel();
    http_handle
        .await
        .expect("collision HTTP joins")
        .expect("collision HTTP stops");
}

#[tokio::test]
async fn overlong_identity_is_rejected_before_hello_attachment_repository_or_rest_lookup() {
    let repository = MetricsRepository::new();
    let broker = CommandBroker::new(4, TEST_TIMEOUT);
    let grpc_addr = reserve_loopback_addr().await;
    let service = IngestService::new(repository.clone()).with_command_broker(broker.clone());
    let (grpc_shutdown, grpc_handle) = start_production_server_at(grpc_addr, service).await;
    let (http_addr, http_shutdown, http_handle) = start_http_with_repo(repository.clone()).await;

    let channel = Endpoint::from_shared(format!("http://{grpc_addr}"))
        .expect("valid overlong identity endpoint")
        .connect()
        .await
        .expect("connect overlong identity client");
    let mut client = MetricsIngestClient::new(channel);
    let (tx, rx) = mpsc::channel(2);
    let mut inbound = client
        .connect(ReceiverStream::new(rx))
        .await
        .expect("open overlong identity stream")
        .into_inner();
    let overlong_component = "x".repeat(257);
    tx.send(ClientFrame {
        payload: Some(client_frame::Payload::Hello(ClientHello {
            header: Some(FrameHeader {
                protocol_version: 1,
                service_name: overlong_component.clone(),
                instance_id: "bounded-instance".to_owned(),
                boot_id: "bounded-boot".to_owned(),
                stream_epoch: 91_401,
                sequence: 0,
                emitted_at_unix_ms: 1,
            }),
            supported_protocol_version: 1,
            full_snapshot_requested: false,
        })),
    })
    .await
    .expect("send overlong ClientHello");
    let rejected = next_server_frame(&mut inbound).await;
    assert!(matches!(
        rejected.payload,
        Some(server_frame::Payload::Error(
            druid_metrics::protocol::ServerError {
                code: 413,
                close_stream: true,
                ..
            }
        ))
    ));
    assert_eq!(broker.live_stream_count(), 0);
    assert!(repository.is_empty());
    assert!(broker
        .issue_reset(
            &overlong_component,
            "bounded-instance",
            "bounded-boot",
            "overlong-command",
            vec![42],
        )
        .await
        .is_err());

    let mut oversized_payload = b"druid-service-identity-v1\0".to_vec();
    for component in [
        overlong_component.as_str(),
        "bounded-instance",
        "bounded-boot",
    ] {
        oversized_payload.extend_from_slice(&(component.len() as u64).to_be_bytes());
        oversized_payload.extend_from_slice(component.as_bytes());
    }
    let oversized_service_id = format!(
        "sid1_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(oversized_payload)
    );
    for path in [
        format!("/druid/api/datasources/42?serviceId={oversized_service_id}"),
        format!("/druid/api/sql/1?serviceId={oversized_service_id}&dataSourceId=42"),
        format!("/druid/api/connections/42?serviceId={oversized_service_id}"),
    ] {
        let response = reqwest::get(format!("http://{http_addr}{path}"))
            .await
            .expect("oversized serviceId detail response");
        assert_eq!(response.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    }

    drop(tx);
    drop(inbound);
    grpc_shutdown.cancel();
    grpc_handle
        .await
        .expect("overlong identity gRPC joins")
        .expect("overlong identity gRPC stops");
    http_shutdown.cancel();
    http_handle
        .await
        .expect("overlong identity HTTP joins")
        .expect("overlong identity HTTP stops");
}

#[tokio::test]
async fn remote_plaintext_authorization_fails_before_the_probe_observes_a_connection() {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0")
        .await
        .expect("bind remote plaintext probe");
    let port = listener
        .local_addr()
        .expect("plaintext probe address")
        .port();
    let accept_task = tokio::spawn(async move { listener.accept().await });
    let mut config = grpc_config(&format!("http://0.0.0.0:{port}"), "plaintext-secret");
    config.shutdown_timeout = Duration::from_millis(100);
    config.authorization = Some(GrpcAuthorization::basic("admin", "never-send-over-h2c"));

    let result = tokio::time::timeout(
        Duration::from_millis(250),
        GrpcMetricsClient::connect(config),
    )
    .await;
    let probe_observed_connection = tokio::time::timeout(Duration::from_millis(50), accept_task)
        .await
        .is_ok();

    assert!(
        matches!(result, Ok(Err(_))),
        "configuration validation must reject remote h2c before attempting transport setup"
    );
    assert!(
        !probe_observed_connection,
        "the plaintext probe must not observe a connection carrying or preceding Authorization"
    );
}

#[derive(Clone)]
struct DelayedNeverHelloProbe {
    entered: Arc<Notify>,
    release_rpc: Arc<Notify>,
    response_senders: HeldResponseSenders,
}

type HeldResponseSenders = Arc<Mutex<Vec<mpsc::Sender<Result<ServerFrame, Status>>>>>;

#[tonic::async_trait]
impl MetricsIngest for DelayedNeverHelloProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        _request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        self.entered.notify_waiters();
        self.release_rpc.notified().await;
        let (tx, rx) = mpsc::channel(1);
        self.response_senders
            .lock()
            .expect("response sender lock")
            .push(tx);
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn initial_connect_rpc_and_hello_ack_share_one_configured_deadline() {
    let entered = Arc::new(Notify::new());
    let release_rpc = Arc::new(Notify::new());
    let response_senders = Arc::new(Mutex::new(Vec::new()));
    let service = DelayedNeverHelloProbe {
        entered: Arc::clone(&entered),
        release_rpc: Arc::clone(&release_rpc),
        response_senders,
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let mut config = grpc_config(&endpoint, "one-initial-deadline");
    config.shutdown_timeout = Duration::from_millis(200);
    let mut client_task = tokio::spawn(GrpcMetricsClient::connect(config));

    tokio::time::timeout(TEST_TIMEOUT, entered.notified())
        .await
        .expect("Connect RPC reaches the deterministic probe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    release_rpc.notify_waiters();
    let result = tokio::time::timeout(Duration::from_millis(125), &mut client_task).await;

    if result.is_err() {
        client_task.abort();
        let _ = client_task.await;
    }
    shutdown.cancel();
    server_handle
        .await
        .expect("deadline probe server joins")
        .expect("deadline probe server stops");

    assert!(
        matches!(result, Ok(Ok(Err(_)))),
        "endpoint connect, Connect RPC setup, and HelloAck must consume one 200ms budget"
    );
}

#[derive(Clone)]
struct ReconnectDeadlineProbe {
    connections: Arc<AtomicUsize>,
    changed: Arc<Notify>,
}

#[tonic::async_trait]
impl MetricsIngest for ReconnectDeadlineProbe {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let connection = self.connections.fetch_add(1, Ordering::AcqRel);
        self.changed.notify_waiters();
        if connection == 1 {
            std::future::pending::<()>().await;
            unreachable!("the stalled reconnect RPC is cancelled by its deadline");
        }

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
            let Some(batch) = inbound.message().await.ok().flatten() else {
                return;
            };
            if connection == 0 {
                return;
            }
            let sequence = frame_header(&batch).expect("batch header").sequence;
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
async fn reconnect_rpc_setup_deadline_expires_and_the_supervisor_retries() {
    let connections = Arc::new(AtomicUsize::new(0));
    let changed = Arc::new(Notify::new());
    let service = ReconnectDeadlineProbe {
        connections: Arc::clone(&connections),
        changed: Arc::clone(&changed),
    };
    let (endpoint, shutdown, server_handle) = start_probe_server(service).await;
    let mut config = grpc_config(&endpoint, "reconnect-deadline");
    config.shutdown_timeout = Duration::from_millis(100);
    let client = GrpcMetricsClient::connect(config)
        .await
        .expect("initial transport connects");
    client
        .send_batch(AggregatedBatch {
            entries: vec![orders_entry(7, 15)],
            is_full_snapshot: true,
        })
        .await
        .expect("queue one replayable batch");

    let retried_after_stall = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            // 先注册通知再检查计数，避免重连恰好发生在检查与等待之间时丢失唤醒。
            let notified = changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if connections.load(Ordering::Acquire) >= 3 {
                break;
            }
            notified.await;
        }
    })
    .await
    .is_ok();
    let shutdown_result = client.shutdown(TEST_TIMEOUT).await;
    shutdown.cancel();
    server_handle
        .await
        .expect("reconnect probe joins")
        .expect("reconnect probe stops");

    assert!(
        retried_after_stall,
        "a stalled reconnect Connect RPC must expire so a successor attempt can start"
    );
    assert!(
        shutdown_result.is_ok(),
        "the successor transport must ACK replayed work and shut down cleanly: {shutdown_result:?}"
    );
}

#[tokio::test]
async fn production_command_entry_is_authenticated_and_fails_closed_without_a_live_stream() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind command HTTP listener");
    let address = listener.local_addr().expect("command HTTP address");
    let config = AdminServerConfig::try_new(
        address,
        SocketAddr::from(([127, 0, 0, 1], 9090)),
        None,
        AdminAuthMode::basic("admin", "secret"),
    )
    .expect("valid loopback command config");
    let broker = CommandBroker::new(8, Duration::from_millis(200));
    let app = production_router_with_command_broker(MetricsRepository::new(), &config, broker);
    let shutdown = CancellationToken::new();
    let server_shutdown = shutdown.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(server_shutdown.cancelled_owned())
            .await
    });
    let body = json!({
        "serviceName": "orders-runtime",
        "instanceId": "instance-command",
        "bootId": "boot-command",
        "commandId": "reset-auth",
        "type": "resetStats",
        "targetDatasourceIds": [84]
    });
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("command HTTP client");

    let denied = client
        .post(format!("http://{address}/druid/api/commands"))
        .json(&body)
        .send()
        .await
        .expect("unauthenticated command request returns");
    assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unavailable = client
        .post(format!("http://{address}/druid/api/commands"))
        .basic_auth("admin", Some("secret"))
        .json(&body)
        .send()
        .await
        .expect("authenticated command request returns");
    assert_eq!(
        unavailable.status(),
        reqwest::StatusCode::SERVICE_UNAVAILABLE
    );

    shutdown.cancel();
    server
        .await
        .expect("command HTTP server joins")
        .expect("command HTTP server stops");
}

#[tokio::test]
async fn command_route_requires_explicit_basic_even_when_admin_auth_is_disabled_or_session_exists()
{
    async fn start_command_http(
        auth: AdminAuthMode,
    ) -> (
        SocketAddr,
        CancellationToken,
        tokio::task::JoinHandle<Result<(), std::io::Error>>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind command auth matrix listener");
        let address = listener.local_addr().expect("command auth matrix address");
        let config = AdminServerConfig::try_new(
            address,
            SocketAddr::from(([127, 0, 0, 1], 9090)),
            None,
            auth,
        )
        .expect("valid loopback command auth matrix config");
        let app = production_router_with_command_broker(
            MetricsRepository::new(),
            &config,
            CommandBroker::new(8, Duration::from_millis(200)),
        );
        let shutdown = CancellationToken::new();
        let server_shutdown = shutdown.clone();
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(server_shutdown.cancelled_owned())
                .await
        });
        (address, shutdown, server)
    }

    let body = json!({
        "serviceName": "orders-runtime",
        "instanceId": "instance-command-auth-matrix",
        "bootId": "boot-command-auth-matrix",
        "commandId": "reset-auth-matrix",
        "type": "resetStats",
        "targetDatasourceIds": [84]
    });
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("command auth matrix HTTP client");

    let (disabled_address, disabled_shutdown, disabled_server) =
        start_command_http(AdminAuthMode::Disabled).await;
    for authorization in [None, Some("Basic YWRtaW46YXJiaXRyYXJ5")] {
        let mut request = client
            .post(format!("http://{disabled_address}/druid/api/commands"))
            .json(&body);
        if let Some(authorization) = authorization {
            request = request.header(reqwest::header::AUTHORIZATION, authorization);
        }
        let response = request
            .send()
            .await
            .expect("disabled command request returns");
        assert_eq!(
            response.status(),
            reqwest::StatusCode::FORBIDDEN,
            "Disabled has no credential truth and must reject before broker side effects"
        );
    }
    disabled_shutdown.cancel();
    disabled_server
        .await
        .expect("disabled command server joins")
        .expect("disabled command server stops");

    let (basic_address, basic_shutdown, basic_server) =
        start_command_http(AdminAuthMode::basic("admin", "secret")).await;
    let login = client
        .post(format!("http://{basic_address}/druid/submitLogin"))
        .form(&[("loginUsername", "admin"), ("loginPassword", "secret")])
        .send()
        .await
        .expect("command auth matrix login returns");
    assert_eq!(login.status(), reqwest::StatusCode::OK);
    let session_cookie = login
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookie| cookie.split(';').next())
        .expect("login returns druid session cookie")
        .to_owned();

    let missing = client
        .post(format!("http://{basic_address}/druid/api/commands"))
        .json(&body)
        .send()
        .await
        .expect("missing Basic command request returns");
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);
    let session_only = client
        .post(format!("http://{basic_address}/druid/api/commands"))
        .header(reqwest::header::COOKIE, session_cookie)
        .json(&body)
        .send()
        .await
        .expect("session-only command request returns");
    assert_eq!(session_only.status(), reqwest::StatusCode::UNAUTHORIZED);
    let invalid = client
        .post(format!("http://{basic_address}/druid/api/commands"))
        .basic_auth("admin", Some("wrong"))
        .json(&body)
        .send()
        .await
        .expect("invalid Basic command request returns");
    assert_eq!(invalid.status(), reqwest::StatusCode::UNAUTHORIZED);
    let valid = client
        .post(format!("http://{basic_address}/druid/api/commands"))
        .basic_auth("admin", Some("secret"))
        .json(&body)
        .send()
        .await
        .expect("valid Basic command request returns");
    assert_eq!(
        valid.status(),
        reqwest::StatusCode::SERVICE_UNAVAILABLE,
        "valid explicit Basic must reach the empty live-stream broker"
    );

    basic_shutdown.cancel();
    basic_server
        .await
        .expect("Basic command server joins")
        .expect("Basic command server stops");
}

struct BrokerCommandProbe {
    reset_count: AtomicUsize,
    snapshot_count: AtomicUsize,
}

impl DataSourceMonitorable for BrokerCommandProbe {
    fn name(&self) -> &str {
        "broker-command-orders"
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {
        self.reset_count.fetch_add(1, Ordering::AcqRel);
    }

    fn identity(&self) -> DataSourceIdentity {
        DataSourceIdentity {
            id: 84,
            name: "broker-command-orders".to_owned(),
            driver_name: Some("postgresql".to_owned()),
        }
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        self.snapshot_count.fetch_add(1, Ordering::AcqRel);
        Ok(DruidTelemetrySnapshot {
            identity: self.identity(),
            pool_snapshot: PoolSnapshot {
                active_count: 9,
                idle_count: 7,
                max_active: 32,
                max_idle: 16,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 1_755_734_400_000,
        })
    }
}

#[tokio::test]
async fn authenticated_broker_executes_reset_and_full_snapshot_through_real_ingest_service() {
    let repository = MetricsRepository::new();
    let broker = CommandBroker::new(16, TEST_TIMEOUT);
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind command HTTP");
    let grpc_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind command gRPC");
    let http_addr = http_listener.local_addr().expect("command HTTP address");
    let grpc_addr = grpc_listener.local_addr().expect("command gRPC address");
    let config = AdminServerConfig::try_new(
        http_addr,
        grpc_addr,
        None,
        AdminAuthMode::basic("admin", "secret"),
    )
    .expect("valid command server config");
    let app = production_router_with_command_broker(repository.clone(), &config, broker.clone());
    let shutdown = CancellationToken::new();
    let http_shutdown = shutdown.clone();
    let grpc_shutdown = shutdown.clone();
    let http_handle = tokio::spawn(async move {
        axum::serve(http_listener, app)
            .with_graceful_shutdown(http_shutdown.cancelled_owned())
            .await
    });
    let ingest = IngestService::new(repository.clone())
        .with_command_broker(broker)
        .with_shutdown_token(grpc_shutdown.clone());
    let grpc_handle = tokio::spawn(async move {
        Server::builder()
            .add_service(ingest.into_server())
            .serve_with_incoming_shutdown(
                TcpListenerStream::new(grpc_listener),
                grpc_shutdown.cancelled_owned(),
            )
            .await
    });

    let source = Arc::new(BrokerCommandProbe {
        reset_count: AtomicUsize::new(0),
        snapshot_count: AtomicUsize::new(0),
    });
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let executor = RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry::new(
            84,
            Arc::downgrade(&monitorable),
        )])),
        1,
        8,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let client = GrpcMetricsClient::connect_with_command_executor(
        grpc_config(&format!("http://{grpc_addr}"), "command"),
        Some(executor),
    )
    .await
    .expect("production client connects to real IngestService");
    let http = reqwest::Client::new();
    let reset_body = json!({
        "serviceName": "orders-runtime",
        "instanceId": "instance-command",
        "bootId": "boot-command",
        "commandId": "reset-once",
        "type": "resetStats",
        "targetDatasourceIds": [84]
    });
    let reset = post_authenticated_command_until_live(&http, http_addr, &reset_body).await;
    assert_eq!(reset.status(), reqwest::StatusCode::OK);
    assert_eq!(source.reset_count.load(Ordering::Acquire), 1);

    let duplicate = post_authenticated_command_until_live(&http, http_addr, &reset_body).await;
    assert_eq!(duplicate.status(), reqwest::StatusCode::OK);
    assert_eq!(
        source.reset_count.load(Ordering::Acquire),
        1,
        "same ID and canonical payload must replay the stable result"
    );
    let conflict_body = json!({
        "serviceName": "orders-runtime",
        "instanceId": "instance-command",
        "bootId": "boot-command",
        "commandId": "reset-once",
        "type": "requestFullSnapshot"
    });
    let conflict = post_authenticated_command_until_live(&http, http_addr, &conflict_body).await;
    assert_eq!(conflict.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(source.reset_count.load(Ordering::Acquire), 1);

    let full_body = json!({
        "serviceName": "orders-runtime",
        "instanceId": "instance-command",
        "bootId": "boot-command",
        "commandId": "full-once",
        "type": "requestFullSnapshot"
    });
    let full = post_authenticated_command_until_live(&http, http_addr, &full_body).await;
    assert_eq!(full.status(), reqwest::StatusCode::OK);
    assert_eq!(source.snapshot_count.load(Ordering::Acquire), 1);
    let entries = repository.all_datasources();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].datasource.identity, 84);
    assert_eq!(entries[0].datasource.active_count, 9);

    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("command client shuts down cleanly");
    shutdown.cancel();
    http_handle
        .await
        .expect("command HTTP joins")
        .expect("command HTTP stops");
    grpc_handle
        .await
        .expect("command gRPC joins")
        .expect("command gRPC stops");
}

#[tokio::test]
async fn command_ack_error_does_not_expose_client_supplied_credentials_over_http() {
    let repository = MetricsRepository::new();
    let broker = CommandBroker::new(16, TEST_TIMEOUT);
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind command HTTP");
    let grpc_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind command gRPC");
    let http_addr = http_listener.local_addr().expect("command HTTP address");
    let grpc_addr = grpc_listener.local_addr().expect("command gRPC address");
    let config = AdminServerConfig::try_new(
        http_addr,
        grpc_addr,
        None,
        AdminAuthMode::basic("admin", "secret"),
    )
    .expect("valid command server config");
    let app = production_router_with_command_broker(repository.clone(), &config, broker.clone());
    let shutdown = CancellationToken::new();
    let http_shutdown = shutdown.clone();
    let grpc_shutdown = shutdown.clone();
    let http_handle = tokio::spawn(async move {
        axum::serve(http_listener, app)
            .with_graceful_shutdown(http_shutdown.cancelled_owned())
            .await
    });
    let ingest = IngestService::new(repository)
        .with_command_broker(broker)
        .with_shutdown_token(grpc_shutdown.clone());
    let grpc_handle = tokio::spawn(async move {
        Server::builder()
            .add_service(ingest.into_server())
            .serve_with_incoming_shutdown(
                TcpListenerStream::new(grpc_listener),
                grpc_shutdown.cancelled_owned(),
            )
            .await
    });

    let identity = StreamIdentity::new("command-error-redaction", 93_001);
    let mut stream = open_generated_stream(&format!("http://{grpc_addr}"), &identity).await;
    let request = tokio::spawn(async move {
        reqwest::Client::new()
            .post(format!("http://{http_addr}/druid/api/commands"))
            .basic_auth("admin", Some("secret"))
            .json(&json!({
                "serviceName": identity.service_name,
                "instanceId": identity.instance_id,
                "bootId": identity.boot_id,
                "commandId": "redacted-failure",
                "type": "requestFullSnapshot"
            }))
            .send()
            .await
            .expect("authenticated command request")
    });
    let delivered = next_server_frame(&mut stream.inbound).await;
    let Some(server_frame::Payload::Command(command)) = delivered.payload else {
        panic!("expected broker command, got {delivered:?}");
    };
    assert_eq!(command.command_id, "redacted-failure");
    stream
        .tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::CommandAck(
                druid_metrics::protocol::CommandAck {
                    header: Some(StreamIdentity::new("command-error-redaction", 93_001).header(0)),
                    command_id: "redacted-failure".to_owned(),
                    success: false,
                    error_message: "postgres://alice:top-secret@database/orders?token=private"
                        .to_owned(),
                },
            )),
        })
        .await
        .expect("send failed command acknowledgement");
    let response = request.await.expect("command request joins");
    assert_eq!(response.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    let body = response.text().await.expect("read command response");
    assert!(body.contains("redacted-failure"));
    assert!(body.contains("command failed"));
    assert!(!body.contains("top-secret"));
    assert!(!body.contains("private"));
    assert!(!body.contains("alice"));

    drop(stream.tx);
    shutdown.cancel();
    http_handle
        .await
        .expect("command HTTP joins")
        .expect("command HTTP stops");
    grpc_handle
        .await
        .expect("command gRPC joins")
        .expect("command gRPC stops");
}

async fn post_authenticated_command_until_live(
    client: &reqwest::Client,
    http_addr: SocketAddr,
    body: &Value,
) -> reqwest::Response {
    tokio::time::timeout(TEST_TIMEOUT, async {
        loop {
            let response = client
                .post(format!("http://{http_addr}/druid/api/commands"))
                .basic_auth("admin", Some("secret"))
                .json(body)
                .send()
                .await
                .expect("authenticated command request");
            if response.status() != reqwest::StatusCode::SERVICE_UNAVAILABLE {
                return response;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("live stream appears before command deadline")
}

struct BlockingResetCommandProbe {
    reset_count: AtomicUsize,
    reset_started: AtomicUsize,
    started: Notify,
    blocked: Mutex<bool>,
    release: Condvar,
}

impl BlockingResetCommandProbe {
    fn release_reset(&self) {
        *self.blocked.lock().expect("reset block lock") = false;
        self.release.notify_all();
    }
}

impl DataSourceMonitorable for BlockingResetCommandProbe {
    fn name(&self) -> &str {
        "blocking-reset-command"
    }

    fn data_source_stat_data(&self) -> Value {
        Value::Null
    }

    fn reset_stat(&self) {
        self.reset_count.fetch_add(1, Ordering::AcqRel);
        self.reset_started.store(1, Ordering::Release);
        self.started.notify_waiters();
        let mut blocked = self.blocked.lock().expect("reset block lock");
        while *blocked {
            blocked = self.release.wait(blocked).expect("reset block wait");
        }
    }

    fn identity(&self) -> DataSourceIdentity {
        DataSourceIdentity {
            id: 85,
            name: "blocking-reset-command".to_owned(),
            driver_name: Some("postgresql".to_owned()),
        }
    }

    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable> {
        Ok(DruidTelemetrySnapshot {
            identity: self.identity(),
            pool_snapshot: PoolSnapshot {
                active_count: 1,
                idle_count: 1,
                max_active: 4,
                max_idle: 2,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: None,
            sampling_time_millis: 1_755_734_400_000,
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_result_survives_real_tonic_half_close_and_reconnect_without_reexecution() {
    let repository = MetricsRepository::new();
    let broker = CommandBroker::new(16, TEST_TIMEOUT);
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind durable command HTTP");
    let http_addr = http_listener.local_addr().expect("durable HTTP address");
    let grpc_reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve durable gRPC address");
    let grpc_addr = grpc_reservation.local_addr().expect("durable gRPC address");
    drop(grpc_reservation);
    let config = AdminServerConfig::try_new(
        http_addr,
        grpc_addr,
        None,
        AdminAuthMode::basic("admin", "secret"),
    )
    .expect("valid durable command config");
    let app = production_router_with_command_broker(repository.clone(), &config, broker.clone());
    let http_shutdown = CancellationToken::new();
    let http_server_shutdown = http_shutdown.clone();
    let http_handle = tokio::spawn(async move {
        axum::serve(http_listener, app)
            .with_graceful_shutdown(http_server_shutdown.cancelled_owned())
            .await
    });
    let service = IngestService::new(repository.clone()).with_command_broker(broker.clone());
    let (grpc_shutdown, grpc_handle) = start_production_server_at(grpc_addr, service).await;

    let source = Arc::new(BlockingResetCommandProbe {
        reset_count: AtomicUsize::new(0),
        reset_started: AtomicUsize::new(0),
        started: Notify::new(),
        blocked: Mutex::new(true),
        release: Condvar::new(),
    });
    let monitorable: Arc<dyn DataSourceMonitorable> = source.clone();
    let executor = RegistryCommandExecutor::new(
        Arc::new(RwLock::new(vec![RegistryEntry::new(
            85,
            Arc::downgrade(&monitorable),
        )])),
        1,
        8,
        druid_metrics::SqlTextPolicy::FingerprintOnly,
    );
    let mut client_config = grpc_config(&format!("http://{grpc_addr}"), "durable-reset");
    client_config.shutdown_timeout = Duration::from_millis(200);
    let client = GrpcMetricsClient::connect_with_command_executor(client_config, Some(executor))
        .await
        .expect("durable reset client connects");
    let command_http = reqwest::Client::new();
    let body = json!({
        "serviceName": "orders-runtime",
        "instanceId": "instance-durable-reset",
        "bootId": "boot-durable-reset",
        "commandId": "durable-reset-once",
        "type": "resetStats",
        "targetDatasourceIds": [85]
    });
    let command_task = tokio::spawn(async move {
        post_authenticated_command_until_live(&command_http, http_addr, &body).await
    });
    tokio::time::timeout(TEST_TIMEOUT, async {
        while source.reset_started.load(Ordering::Acquire) == 0 {
            source.started.notified().await;
        }
    })
    .await
    .expect("reset side effect starts before the response stream closes");

    assert!(
        broker.cancel_stream(
            "orders-runtime",
            "instance-durable-reset",
            "boot-durable-reset"
        ),
        "the broker owns the live stream selected by the authenticated command"
    );
    source.release_reset();

    let response = tokio::time::timeout(TEST_TIMEOUT, command_task)
        .await
        .expect("command completes after reconnect")
        .expect("command HTTP task joins");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        source.reset_count.load(Ordering::Acquire),
        1,
        "the durable client result must replay without repeating ResetStats"
    );
    let progress = client.progress();
    assert_eq!(progress.transport_queued_commands(), 0);
    assert_eq!(progress.pending_commands(), 0);

    client
        .shutdown(TEST_TIMEOUT)
        .await
        .expect("durable reset client shuts down");
    grpc_shutdown.cancel();
    grpc_handle
        .await
        .expect("durable gRPC joins")
        .expect("durable gRPC stops");
    http_shutdown.cancel();
    http_handle
        .await
        .expect("durable HTTP joins")
        .expect("durable HTTP stops");
}
