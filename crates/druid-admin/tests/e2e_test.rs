//! End-to-end integration tests.
//!
//! Task 5 Step 1: Tests for the full chain:
//! gRPC push → admin repository → REST query.

use std::collections::HashMap;
use std::time::Duration;

use druid_admin::ingest::ingest_proto::metrics_ingest_client::MetricsIngestClient;
use druid_admin::ingest::ingest_proto::{DataSourceStats, MetricsSnapshot, SqlStats, WallStats};
use druid_admin::ingest::IngestService;
use druid_admin::repository::MetricsRepository;
use serde_json::Value;

/// Start both HTTP and gRPC servers sharing the same repository.
/// Returns (`http_addr`, `grpc_addr`, repo, `http_handle`, `grpc_handle`).
async fn start_admin() -> (
    std::net::SocketAddr,
    String,
    MetricsRepository,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>,
) {
    let repo = MetricsRepository::new();

    // HTTP server
    let http_app = druid_admin::standalone_router(repo.clone());
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_addr = http_listener.local_addr().unwrap();
    let http_handle = tokio::spawn(async move {
        axum::serve(http_listener, http_app).await.unwrap();
    });

    // gRPC server
    let ingest_service = IngestService::new(repo.clone());
    let grpc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr = grpc_listener.local_addr().unwrap();
    let grpc_handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(ingest_service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                grpc_listener,
            ))
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    (
        http_addr,
        format!("http://{grpc_addr}"),
        repo,
        http_handle,
        grpc_handle,
    )
}

/// Full chain: gRPC push → repository → REST datasource.json query.
#[tokio::test]
async fn e2e_grpc_push_rest_query_datasource() {
    let (http_addr, grpc_addr, _repo, http_handle, grpc_handle) = start_admin().await;

    // Push via gRPC
    let mut client = MetricsIngestClient::connect(grpc_addr).await.unwrap();

    let stream = tokio_stream::iter(vec![MetricsSnapshot {
        service_id: "svc-1".to_owned(),
        identity: 42,
        sequence: 1,
        timestamp_ms: 1234567890,
        datasource: Some(DataSourceStats {
            identity: 42,
            db_type: "postgresql".to_owned(),
            url: "jdbc:postgresql://localhost/mydb".to_owned(),
            user_name: "app".to_owned(),
            active_count: 7,
            pooling_count: 15,
            execute_count: 500,
            error_count: 1,
            commit_count: 200,
            rollback_count: 3,
            ..Default::default()
        }),
        sql_stats: HashMap::new(),
        wall: None,
    }]);

    let resp = client.push_snapshots(stream).await.unwrap();
    assert_eq!(resp.into_inner().accepted, 1);

    // Query via REST
    let resp = reqwest::get(format!("http://{http_addr}/druid/datasource.json"))
        .await
        .expect("GET /druid/datasource.json must succeed");

    let body: Value = resp.json().await.expect("must be JSON");
    assert_eq!(body["ResultCode"], 1);

    let content = body["Content"].as_array().expect("Content must be array");
    assert_eq!(content.len(), 1, "must have one datasource");

    let ds = &content[0];
    assert_eq!(ds["DbType"], "postgresql");
    assert_eq!(ds["ActiveCount"], 7);
    assert_eq!(ds["PoolingCount"], 15);
    assert_eq!(ds["ExecuteCount"], 500);
    assert_eq!(ds["Identity"], 42);

    http_handle.abort();
    grpc_handle.abort();
}

/// Full chain: gRPC push with SQL stats → REST sql.json query.
#[tokio::test]
async fn e2e_grpc_push_rest_query_sql() {
    let (http_addr, grpc_addr, _repo, http_handle, grpc_handle) = start_admin().await;

    let mut client = MetricsIngestClient::connect(grpc_addr).await.unwrap();

    let mut sql_stats = HashMap::new();
    sql_stats.insert(
        99887,
        SqlStats {
            id: 1,
            sql: "SELECT * FROM orders WHERE id = ?".to_owned(),
            execute_count: 50,
            total_time: 2500,
            max_timespan: 150,
            error_count: 0,
            running_count: 1,
            concurrent_max: 2,
            fetch_row_count: 100,
            effected_row_count: 0,
            db_type: "mysql".to_owned(),
        },
    );

    let stream = tokio_stream::iter(vec![MetricsSnapshot {
        service_id: "svc-1".to_owned(),
        identity: 1,
        sequence: 1,
        timestamp_ms: 1000,
        datasource: None,
        sql_stats,
        wall: None,
    }]);

    let resp = client.push_snapshots(stream).await.unwrap();
    assert_eq!(resp.into_inner().accepted, 1);

    // Query via REST
    let resp = reqwest::get(format!("http://{http_addr}/druid/sql.json"))
        .await
        .expect("GET /druid/sql.json must succeed");

    let body: Value = resp.json().await.expect("must be JSON");
    let content = body["Content"].as_array().expect("Content must be array");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["SQL"], "SELECT * FROM orders WHERE id = ?");
    assert_eq!(content[0]["ExecuteCount"], 50);

    http_handle.abort();
    grpc_handle.abort();
}

/// Full chain: gRPC push with wall stats → REST wall.json query.
#[tokio::test]
async fn e2e_grpc_push_rest_query_wall() {
    let (http_addr, grpc_addr, _repo, http_handle, grpc_handle) = start_admin().await;

    let mut client = MetricsIngestClient::connect(grpc_addr).await.unwrap();

    let stream = tokio_stream::iter(vec![MetricsSnapshot {
        service_id: "svc-1".to_owned(),
        identity: 1,
        sequence: 1,
        timestamp_ms: 1000,
        datasource: None,
        sql_stats: HashMap::new(),
        wall: Some(WallStats {
            check_count: 5000,
            hard_check_count: 100,
            violation_count: 10,
            syntax_error_count: 2,
            black_list_hit_count: 5,
            white_list_hit_count: 4900,
            black_list_size: 50,
            white_list_size: 200,
        }),
    }]);

    let resp = client.push_snapshots(stream).await.unwrap();
    assert_eq!(resp.into_inner().accepted, 1);

    // Query via REST
    let resp = reqwest::get(format!("http://{http_addr}/druid/wall.json"))
        .await
        .expect("GET /druid/wall.json must succeed");

    let body: Value = resp.json().await.expect("must be JSON");
    let content = &body["Content"];
    assert_eq!(content["checkCount"], 5000);
    assert_eq!(content["violationCount"], 10);

    http_handle.abort();
    grpc_handle.abort();
}

/// Full chain: push → query → push newer → query (verify update).
#[tokio::test]
async fn e2e_push_update_push_query() {
    let (http_addr, grpc_addr, _repo, http_handle, grpc_handle) = start_admin().await;

    let mut client = MetricsIngestClient::connect(grpc_addr).await.unwrap();

    // First push
    let stream = tokio_stream::iter(vec![MetricsSnapshot {
        service_id: "svc-1".to_owned(),
        identity: 1,
        sequence: 1,
        timestamp_ms: 1000,
        datasource: Some(DataSourceStats {
            identity: 1,
            db_type: "mysql".to_owned(),
            active_count: 5,
            ..Default::default()
        }),
        sql_stats: HashMap::new(),
        wall: None,
    }]);
    let resp = client.push_snapshots(stream).await.unwrap();
    assert_eq!(resp.into_inner().accepted, 1);

    // Second push with newer sequence
    let stream = tokio_stream::iter(vec![MetricsSnapshot {
        service_id: "svc-1".to_owned(),
        identity: 1,
        sequence: 2,
        timestamp_ms: 2000,
        datasource: Some(DataSourceStats {
            identity: 1,
            db_type: "mysql".to_owned(),
            active_count: 10,
            ..Default::default()
        }),
        sql_stats: HashMap::new(),
        wall: None,
    }]);
    let resp = client.push_snapshots(stream).await.unwrap();
    assert_eq!(resp.into_inner().accepted, 1);

    // Query should show updated value
    let resp = reqwest::get(format!("http://{http_addr}/druid/datasource.json"))
        .await
        .expect("GET must succeed");

    let body: Value = resp.json().await.expect("must be JSON");
    let content = body["Content"].as_array().expect("Content must be array");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["ActiveCount"], 10, "must show updated value");

    http_handle.abort();
    grpc_handle.abort();
}

/// Health endpoint works alongside gRPC and REST.
#[tokio::test]
async fn e2e_health_always_works() {
    let (http_addr, grpc_addr, _repo, http_handle, grpc_handle) = start_admin().await;

    // Push some data first
    let mut client = MetricsIngestClient::connect(grpc_addr).await.unwrap();
    let stream = tokio_stream::iter(vec![MetricsSnapshot {
        service_id: "svc-1".to_owned(),
        identity: 1,
        sequence: 1,
        timestamp_ms: 1000,
        datasource: None,
        sql_stats: HashMap::new(),
        wall: None,
    }]);
    client.push_snapshots(stream).await.unwrap();

    // Health must still work
    let resp = reqwest::get(format!("http://{http_addr}/health"))
        .await
        .expect("GET /health must succeed");

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("must be JSON");
    assert_eq!(body["status"], "ok");

    http_handle.abort();
    grpc_handle.abort();
}

/// Prometheus /metrics works after data is pushed.
#[tokio::test]
async fn e2e_metrics_after_push() {
    let (http_addr, grpc_addr, _repo, http_handle, grpc_handle) = start_admin().await;

    let mut client = MetricsIngestClient::connect(grpc_addr).await.unwrap();
    let stream = tokio_stream::iter(vec![MetricsSnapshot {
        service_id: "svc-1".to_owned(),
        identity: 1,
        sequence: 1,
        timestamp_ms: 1000,
        datasource: Some(DataSourceStats {
            identity: 1,
            db_type: "mysql".to_owned(),
            active_count: 5,
            execute_count: 100,
            ..Default::default()
        }),
        sql_stats: HashMap::new(),
        wall: None,
    }]);
    client.push_snapshots(stream).await.unwrap();

    let resp = reqwest::get(format!("http://{http_addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(body.contains("druid_admin_datasource_count 1"));
    assert!(body.contains("druid_admin_datasource_active_count"));
    assert!(body.contains("druid_admin_ingest_total 1"));

    http_handle.abort();
    grpc_handle.abort();
}
