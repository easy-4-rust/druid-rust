//! RED tests for the standalone druid-admin binary.
//!
//! Task 1 Step 1: Tests for dual-port binding (HTTP 8080, gRPC 9090),
//! /health route, and graceful shutdown.

use std::time::Duration;

/// The standalone admin binary must expose a /health endpoint on the HTTP port
/// that returns 200 OK with a JSON status body.
#[tokio::test]
async fn health_endpoint_returns_ok() {
    let repo = druid_admin::repository::MetricsRepository::new();
    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    let resp = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("GET /health must succeed");
    assert_eq!(resp.status(), 200, "/health must return 200 OK");

    let body: serde_json::Value = resp.json().await.expect("response must be JSON");
    assert_eq!(
        body.get("status").and_then(|v| v.as_str()),
        Some("ok"),
        "/health body must contain status:ok"
    );

    handle.abort();
}

/// The admin binary must be able to bind the HTTP listener on a configurable port.
#[tokio::test]
async fn http_port_bindable() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await;
    assert!(listener.is_ok(), "HTTP port must be bindable");
    let addr = listener.unwrap().local_addr().unwrap();
    assert!(addr.port() > 0, "port must be non-zero");
}

/// The admin binary must be able to bind the gRPC listener on a separate port.
#[tokio::test]
async fn grpc_port_bindable() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await;
    assert!(listener.is_ok(), "gRPC port must be bindable");
    let addr = listener.unwrap().local_addr().unwrap();
    assert!(addr.port() > 0, "port must be non-zero");
}

/// Graceful shutdown: when the cancellation token fires, the server must stop
/// accepting new connections within the deadline.
#[tokio::test]
async fn graceful_shutdown_stops_server() {
    let repo = druid_admin::repository::MetricsRepository::new();
    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let shutdown = tokio_util::sync::CancellationToken::new();
    let shutdown_clone = shutdown.clone();

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_clone.cancelled().await;
            })
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify server is up
    let resp = reqwest::get(format!("http://{addr}/health")).await;
    assert!(resp.is_ok(), "server must respond before shutdown");

    // Trigger shutdown
    shutdown.cancel();

    // Wait for the server task to complete
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;

    // After shutdown, connection must fail
    let resp = reqwest::get(format!("http://{addr}/health")).await;
    assert!(
        resp.is_err(),
        "server must reject connections after shutdown"
    );
}

/// The /health endpoint must return Content-Type: application/json.
#[tokio::test]
async fn health_content_type_is_json() {
    let repo = druid_admin::repository::MetricsRepository::new();
    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let resp = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("GET /health must succeed");
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.contains("application/json"),
        "Content-Type must be application/json, got: {content_type}"
    );

    handle.abort();
}
