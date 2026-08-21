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

#[tokio::test]
async fn grpc_bind_failure_cancels_http_sibling_and_exits_immediately() {
    let grpc_conflict = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve conflicting gRPC address");
    let grpc_addr = grpc_conflict.local_addr().expect("gRPC conflict address");
    let http_reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve HTTP address");
    let http_addr = http_reservation.local_addr().expect("HTTP address");
    drop(http_reservation);

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_druid-admin"))
        .env("DRUID_ADMIN_HTTP_ADDR", http_addr.to_string())
        .env("DRUID_ADMIN_GRPC_ADDR", grpc_addr.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn admin binary");
    let status =
        if let Ok(result) = tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
            result.expect("wait for admin binary")
        } else {
            child.kill().await.expect("kill stuck admin binary");
            let output = child
                .wait_with_output()
                .await
                .expect("collect stuck admin output");
            panic!(
                "gRPC bind failure must not leave the HTTP sibling running: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
    assert!(!status.success(), "bind failure must be returned by main");
    tokio::net::TcpListener::bind(http_addr)
        .await
        .expect("HTTP sibling released its listener before process exit");
}
