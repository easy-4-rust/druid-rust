//! Shared test helpers for druid-admin integration tests.
//!
//! Provides reusable server startup functions to reduce duplication
//! across test files.

#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use druid_admin::ingest::IngestService;
use druid_admin::model::dto::{DataSourceContent, SqlListContent, WallResult};
use druid_admin::repository::{DataSourceEntry, DataSourceId, MetricsRepository};

/// Time to wait for servers to become ready.
const READY_DELAY: Duration = Duration::from_millis(50);

/// Handle to a running standalone HTTP server.
pub struct HttpServerHandle {
    pub addr: SocketAddr,
    pub handle: tokio::task::JoinHandle<()>,
}

/// Handle to a running gRPC server.
pub struct GrpcServerHandle {
    pub addr: String,
    pub handle: tokio::task::JoinHandle<()>,
}

/// Start the standalone HTTP server with an empty repository.
/// Returns the bound address and a join handle.
pub async fn start_http_server() -> HttpServerHandle {
    let repo = MetricsRepository::new();
    start_http_server_with_repo(repo).await
}

/// Start the standalone HTTP server with the given repository.
pub async fn start_http_server_with_repo(repo: MetricsRepository) -> HttpServerHandle {
    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(READY_DELAY).await;
    HttpServerHandle { addr, handle }
}

/// Start a gRPC ingest server with an empty repository.
/// Returns the gRPC endpoint URL and a join handle.
pub async fn start_grpc_server() -> (GrpcServerHandle, MetricsRepository) {
    let repo = MetricsRepository::new();
    let service = IngestService::new(repo.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    tokio::time::sleep(READY_DELAY).await;
    (
        GrpcServerHandle {
            addr: format!("http://{local_addr}"),
            handle,
        },
        repo,
    )
}

/// Start both HTTP and gRPC servers sharing the same repository.
pub async fn start_admin() -> (HttpServerHandle, GrpcServerHandle, MetricsRepository) {
    let repo = MetricsRepository::new();
    let http = start_http_server_with_repo(repo.clone()).await;
    let grpc_service = IngestService::new(repo.clone());
    let grpc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr = grpc_listener.local_addr().unwrap();
    let grpc_handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(grpc_service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                grpc_listener,
            ))
            .await
            .unwrap();
    });
    tokio::time::sleep(READY_DELAY).await;
    (
        http,
        GrpcServerHandle {
            addr: format!("http://{grpc_addr}"),
            handle: grpc_handle,
        },
        repo,
    )
}

/// Build a `DataSourceEntry` with minimal required fields for testing.
pub fn make_datasource_entry(
    identity: i64,
    db_type: &str,
    sequence: u64,
) -> (DataSourceId, DataSourceEntry) {
    let id = DataSourceId {
        service_id: "svc-1".to_owned(),
        identity,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity,
            db_type: Some(db_type.to_owned()),
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall: WallResult::default(),
        last_updated_ms: 1000,
        sequence,
    };
    (id, entry)
}

/// Build a `DataSourceEntry` with SQL stats for testing.
pub fn make_datasource_entry_with_sql(
    identity: i64,
    db_type: &str,
    sequence: u64,
    sql_hash: i64,
    sql_text: &str,
    execute_count: i64,
) -> (DataSourceId, DataSourceEntry) {
    let mut sql_stats = HashMap::new();
    sql_stats.insert(
        sql_hash,
        SqlListContent {
            id: 1,
            sql: Some(sql_text.to_owned()),
            execute_count,
            ..Default::default()
        },
    );
    let id = DataSourceId {
        service_id: "svc-1".to_owned(),
        identity,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity,
            db_type: Some(db_type.to_owned()),
            ..Default::default()
        },
        sql_stats,
        wall: WallResult::default(),
        last_updated_ms: 1000,
        sequence,
    };
    (id, entry)
}

/// Build a `DataSourceEntry` with wall stats for testing.
pub fn make_datasource_entry_with_wall(
    identity: i64,
    db_type: &str,
    sequence: u64,
    check_count: i64,
) -> (DataSourceId, DataSourceEntry) {
    let mut wall = WallResult::default();
    wall.content.check_count = check_count;
    let id = DataSourceId {
        service_id: "svc-1".to_owned(),
        identity,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity,
            db_type: Some(db_type.to_owned()),
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall,
        last_updated_ms: 1000,
        sequence,
    };
    (id, entry)
}
