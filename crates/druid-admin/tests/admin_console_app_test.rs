//! Tests for the separated console application's delivery.
//!
//! The console is an independently built application whose resources are
//! embedded by the build script. These tests are adaptive: when the console
//! has been built they assert it is served; when it has not (for example in
//! a Rust-only CI job) they assert the Admin still compiles, still serves its
//! API, and reports the absence instead of failing to start.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

async fn start_admin() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let servlet = druid_admin::MonitorViewServlet::from_repository(
        druid_admin::repository::MetricsRepository::new(),
    );
    let app = servlet.router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

async fn get(addr: SocketAddr, path: &str) -> (u16, String, String) {
    let response = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body = response.text().await.unwrap();
    (status, content_type, body)
}

/// 报告控制台是否已内嵌；未内嵌时这些断言改为验证降级行为。
async fn console_built(addr: SocketAddr) -> bool {
    let (status, _, _) = get(addr, "/druid/console").await;
    status == 200
}

/// 前端未构建时的降级契约：Admin 仍编译并正常提供 API。
async fn assert_degraded_console(addr: SocketAddr) {
    let (status, _, body) = get(addr, "/druid/console").await;
    assert_eq!(
        status, 404,
        "without a built console the entry must report its absence, got {body}"
    );
    assert!(
        body.contains("not been built"),
        "the absence must be explained, got {body}"
    );
    // API 不受前端缺失影响。
    let (api_status, _, api_body) = get(addr, "/druid/api/v1/basic").await;
    assert_eq!(
        api_status, 200,
        "the API must stay available without the console, got {api_body}"
    );
    let api_json: serde_json::Value =
        serde_json::from_str(&api_body).expect("the API must answer JSON");
    assert_eq!(api_json["apiVersion"], "v1");
}

#[tokio::test]
async fn console_entry_serves_the_built_application() {
    let (addr, _handle) = start_admin().await;
    if !console_built(addr).await {
        assert_degraded_console(addr).await;
        return;
    }

    let (status, content_type, body) = get(addr, "/druid/console").await;

    assert_eq!(status, 200, "the console entry must be served");
    assert!(
        content_type.starts_with("text/html"),
        "the console entry must be HTML, got {content_type}"
    );
    assert!(
        body.contains("<div id=\"app\">"),
        "the console entry must be the built application shell, got {body}"
    );
    assert!(
        body.contains("assets/"),
        "the console entry must reference the built assets, got {body}"
    );
}

#[tokio::test]
async fn console_unknown_path_falls_back_to_the_application_entry() {
    let (addr, _handle) = start_admin().await;
    if !console_built(addr).await {
        assert_degraded_console(addr).await;
        return;
    }

    let (status, _content_type, body) = get(addr, "/druid/console/datasources/7").await;

    assert_eq!(
        status, 200,
        "an unknown console path must fall back to the entry, got {body}"
    );
    assert!(
        body.contains("<div id=\"app\">"),
        "the fallback must be the application entry, got {body}"
    );
}

#[tokio::test]
async fn console_asset_is_served_with_its_own_content_type() {
    let (addr, _handle) = start_admin().await;
    if !console_built(addr).await {
        assert_degraded_console(addr).await;
        return;
    }

    let (status, entry_content_type, entry_body) = get(addr, "/druid/console").await;
    assert_eq!(status, 200);

    // 从入口 HTML 里取出一个真实构建产物，验证它可以被单独取到。
    let Some(marker) = entry_body.split_once("assets/").map(|(_, rest)| {
        let tail = rest.split('"').next().unwrap_or_default();
        format!("assets/{tail}")
    }) else {
        panic!("the console entry must reference a built asset, got {entry_body}");
    };

    let (asset_status, asset_content_type, asset_body) =
        get(addr, &format!("/druid/console/{marker}")).await;

    assert_eq!(asset_status, 200, "the built asset must be served");
    assert!(
        asset_content_type.contains("javascript") || asset_content_type.contains("css"),
        "the asset must carry its own content type, got {asset_content_type}"
    );
    assert!(!asset_body.is_empty(), "the asset must not be empty");
    assert_ne!(asset_content_type, entry_content_type);
}

#[tokio::test]
async fn the_api_stays_available_regardless_of_the_console() {
    // 无论前端是否构建，版本化 API 都可用；这是「前后端解耦」的硬约束。
    let (addr, _handle) = start_admin().await;
    let (status, content_type, body) = get(addr, "/druid/api/v1/basic").await;
    assert_eq!(status, 200);
    assert!(content_type.starts_with("application/json"));
    let json: serde_json::Value = serde_json::from_str(&body).expect("the API must answer JSON");
    assert_eq!(json["apiVersion"], "v1");
}
