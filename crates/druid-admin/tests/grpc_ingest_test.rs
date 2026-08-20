//! gRPC ingest integration tests.
//!
//! Tests the actual gRPC streaming ingest by starting a tonic server
//! and pushing snapshots via a gRPC client.

use std::collections::HashMap;
use std::time::Duration;

use druid_admin::ingest::ingest_proto::metrics_ingest_client::MetricsIngestClient;
use druid_admin::ingest::ingest_proto::{DataSourceStats, MetricsSnapshot, SqlStats, WallStats};
use druid_admin::ingest::IngestService;
use druid_admin::repository::MetricsRepository;

/// Helper: start a gRPC server and return (addr, server_handle).
async fn start_grpc_server() -> (String, tokio::task::JoinHandle<()>) {
    let repo = MetricsRepository::new();
    let service = IngestService::new(repo);

    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    (format!("http://{local_addr}"), handle)
}

/// Pushing a single snapshot via gRPC must be accepted.
#[tokio::test]
async fn grpc_push_single_snapshot() {
    let (addr, handle) = start_grpc_server().await;
    let mut client = MetricsIngestClient::connect(addr).await.unwrap();

    let stream = tokio_stream::iter(vec![MetricsSnapshot {
        service_id: "svc-1".to_owned(),
        identity: 1,
        sequence: 1,
        timestamp_ms: 1000,
        datasource: Some(DataSourceStats {
            identity: 1,
            db_type: "mysql".to_owned(),
            url: "jdbc:mysql://localhost/test".to_owned(),
            user_name: "root".to_owned(),
            active_count: 5,
            pooling_count: 10,
            ..Default::default()
        }),
        sql_stats: HashMap::new(),
        wall: None,
    }]);

    let resp = client.push_snapshots(stream).await.unwrap();
    let body = resp.into_inner();
    assert_eq!(body.accepted, 1, "one snapshot must be accepted");
    assert_eq!(body.rejected, 0, "no rejections expected");

    handle.abort();
}

/// Pushing a duplicate sequence must be rejected.
#[tokio::test]
async fn grpc_rejects_duplicate_sequence() {
    let (addr, handle) = start_grpc_server().await;
    let mut client = MetricsIngestClient::connect(addr).await.unwrap();

    let stream = tokio_stream::iter(vec![
        MetricsSnapshot {
            service_id: "svc-1".to_owned(),
            identity: 1,
            sequence: 1,
            timestamp_ms: 1000,
            datasource: None,
            sql_stats: HashMap::new(),
            wall: None,
        },
        MetricsSnapshot {
            service_id: "svc-1".to_owned(),
            identity: 1,
            sequence: 1,
            timestamp_ms: 1000,
            datasource: None,
            sql_stats: HashMap::new(),
            wall: None,
        },
    ]);

    let resp = client.push_snapshots(stream).await.unwrap();
    let body = resp.into_inner();
    assert_eq!(body.accepted, 1);
    assert_eq!(body.rejected, 1);

    handle.abort();
}

/// Pushing multiple snapshots with increasing sequences must all be accepted.
#[tokio::test]
async fn grpc_accepts_increasing_sequences() {
    let (addr, handle) = start_grpc_server().await;
    let mut client = MetricsIngestClient::connect(addr).await.unwrap();

    let snapshots: Vec<MetricsSnapshot> = (1..=5)
        .map(|seq| MetricsSnapshot {
            service_id: "svc-1".to_owned(),
            identity: 1,
            sequence: seq,
            timestamp_ms: 1000 + seq as i64,
            datasource: Some(DataSourceStats {
                identity: 1,
                db_type: "postgres".to_owned(),
                ..Default::default()
            }),
            sql_stats: HashMap::new(),
            wall: None,
        })
        .collect();

    let stream = tokio_stream::iter(snapshots);
    let resp = client.push_snapshots(stream).await.unwrap();
    let body = resp.into_inner();
    assert_eq!(body.accepted, 5);
    assert_eq!(body.rejected, 0);

    handle.abort();
}

/// Pushing snapshots with SQL stats must populate the repository.
#[tokio::test]
async fn grpc_push_with_sql_stats() {
    let repo = MetricsRepository::new();
    let service = IngestService::new(repo.clone());

    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = MetricsIngestClient::connect(format!("http://{local_addr}"))
        .await
        .unwrap();

    let mut sql_stats = HashMap::new();
    sql_stats.insert(
        12345,
        SqlStats {
            id: 1,
            sql: "SELECT * FROM users".to_owned(),
            execute_count: 100,
            total_time: 5000,
            max_timespan: 200,
            error_count: 2,
            running_count: 1,
            concurrent_max: 3,
            fetch_row_count: 500,
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
        wall: Some(WallStats {
            check_count: 1000,
            violation_count: 5,
            ..Default::default()
        }),
    }]);

    let resp = client.push_snapshots(stream).await.unwrap();
    assert_eq!(resp.into_inner().accepted, 1);

    // Verify the repository has the data
    let sql_list = repo.all_sql_stats();
    assert_eq!(sql_list.len(), 1, "must have one SQL stat entry");
    assert_eq!(sql_list[0].sql.as_deref(), Some("SELECT * FROM users"));

    let wall = repo.aggregated_wall();
    assert_eq!(wall.content.check_count, 1000);

    handle.abort();
}
