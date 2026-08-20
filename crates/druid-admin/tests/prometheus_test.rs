//! RED tests for Prometheus /metrics endpoint.
//!
//! Task 4 Step 1: Tests for /metrics endpoint returning Prometheus text format.

use std::collections::HashMap;
use std::time::Duration;

use druid_admin::model::dto::{DataSourceContent, WallResult};
use druid_admin::repository::{DataSourceEntry, DataSourceId, MetricsRepository};

/// Helper: start the standalone HTTP server with data in the repository.
async fn start_server_with_data() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let repo = MetricsRepository::new();

    // Insert test data
    let id = DataSourceId {
        service_id: "svc-1".to_owned(),
        identity: 1,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity: 1,
            db_type: Some("mysql".to_owned()),
            active_count: 5,
            pooling_count: 10,
            execute_count: 1000,
            error_count: 3,
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall: WallResult::default(),
        last_updated_ms: 0,
        sequence: 1,
    };
    repo.upsert_datasource(id, entry);

    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

/// /metrics must return 200 OK.
#[tokio::test]
async fn metrics_returns_ok() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    assert_eq!(resp.status(), 200);

    handle.abort();
}

/// /metrics must return Content-Type: text/plain; version=0.0.4 (Prometheus text format).
#[tokio::test]
async fn metrics_content_type_is_prometheus() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.contains("text/plain"),
        "Content-Type must be text/plain, got: {content_type}"
    );

    handle.abort();
}

/// /metrics must contain druid_admin_datasource_active_count metric.
#[tokio::test]
async fn metrics_contains_datasource_metrics() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(
        body.contains("druid_admin_datasource_active_count"),
        "must contain datasource active count metric:\n{body}"
    );

    handle.abort();
}

/// /metrics must contain HELP and TYPE lines for metric families.
#[tokio::test]
async fn metrics_contains_help_and_type() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(body.contains("# HELP"), "must contain HELP lines:\n{body}");
    assert!(body.contains("# TYPE"), "must contain TYPE lines:\n{body}");

    handle.abort();
}

/// /metrics must contain ingest counters from the repository.
#[tokio::test]
async fn metrics_contains_ingest_counters() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(
        body.contains("druid_admin_ingest_total"),
        "must contain ingest total counter:\n{body}"
    );
    assert!(
        body.contains("druid_admin_ingest_rejected_total"),
        "must contain ingest rejected counter:\n{body}"
    );

    handle.abort();
}

/// /metrics must contain datasource count gauge.
#[tokio::test]
async fn metrics_contains_datasource_count() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(
        body.contains("druid_admin_datasource_count"),
        "must contain datasource count gauge:\n{body}"
    );

    handle.abort();
}
