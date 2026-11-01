//! Tests for the separated console's authentication surface.
//!
//! - A bearer token grants the operator role for a cross-origin client, while a
//!   wrong or absent token does not.
//! - The versioned API serves the operator view (URL and SQL intact) to an
//!   authenticated operator; the Java-compatible routes keep the conservative
//!   redaction.
//! - CORS answers a preflight for a configured origin and refuses the rest.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use serde_json::Value;

use druid_admin::config::AdminAuthMode;
use druid_admin::repository::MetricsRepository;

async fn start_admin(
    build: impl FnOnce(druid_admin::MonitorViewServlet) -> druid_admin::MonitorViewServlet,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let servlet = build(druid_admin::MonitorViewServlet::from_repository(
        MetricsRepository::new(),
    ));
    let app = servlet.router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

#[tokio::test]
async fn bearer_token_grants_operator_access() {
    let (addr, _handle) = start_admin(|s| {
        s.with_auth_mode(AdminAuthMode::basic("admin", "secret"))
            .with_bearer_token("console-token")
    })
    .await;

    let response = reqwest::Client::new()
        .get(format!("http://{addr}/druid/api/v1/basic"))
        .header("Authorization", "Bearer console-token")
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status().as_u16(),
        200,
        "a valid bearer token must grant access"
    );
}

#[tokio::test]
async fn wrong_or_absent_bearer_token_is_redirected() {
    let (addr, _handle) = start_admin(|s| {
        s.with_auth_mode(AdminAuthMode::basic("admin", "secret"))
            .with_bearer_token("console-token")
    })
    .await;

    let no_redirect = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    for token in [None, Some("Bearer wrong-token"), Some("Bearer")] {
        let mut request = no_redirect.get(format!("http://{addr}/druid/api/v1/basic"));
        if let Some(value) = token {
            request = request.header("Authorization", value);
        }
        let response = request.send().await.unwrap();
        assert_eq!(
            response.status().as_u16(),
            302,
            "token {token:?} must not grant access"
        );
    }
}

#[tokio::test]
async fn v1_api_serves_the_operator_view_while_java_routes_stay_redacted() {
    let repository = MetricsRepository::new();
    let (datasource_id, mut entry) =
        common::make_datasource_entry_with_sql(3, "sqlite", 1, 11, "select * from t", 5);
    entry.datasource.url = Some("jdbc:postgresql://10.0.0.7:5432/orders".to_owned());
    entry.datasource.user_name = Some("orders_app".to_owned());
    repository.upsert_datasource(datasource_id, entry);
    let servlet = druid_admin::MonitorViewServlet::from_repository(repository)
        .with_auth_mode(AdminAuthMode::basic("admin", "secret"))
        .with_bearer_token("console-token");
    let app = servlet.router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let _handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 版本化 API：operator 视图保留 URL 与 SQL。
    let v1: Value = reqwest::Client::new()
        .get(format!("http://{addr}/druid/api/v1/datasources"))
        .header("Authorization", "Bearer console-token")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        v1["datasources"][0]["url"], "jdbc:postgresql://10.0.0.7:5432/orders",
        "the versioned API must keep the operator view, got {v1}"
    );

    // Java 兼容路由：保守脱敏仍然生效。
    let java: Value = reqwest::Client::new()
        .get(format!("http://{addr}/druid/datasource.json"))
        .header("Authorization", "Bearer console-token")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        java["Content"][0]["URL"], "[REDACTED]",
        "the Java-compatible route must stay redacted, got {java}"
    );
}

#[tokio::test]
async fn cors_preflight_answers_the_configured_origin() {
    let (addr, _handle) =
        start_admin(|s| s.with_cors_origins(vec!["https://console.example.com".to_owned()])).await;

    let client = reqwest::Client::new();
    let allowed = client
        .request(
            reqwest::Method::OPTIONS,
            format!("http://{addr}/druid/api/v1/basic"),
        )
        .header("Origin", "https://console.example.com")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .unwrap();
    assert_eq!(allowed.status().as_u16(), 200);
    assert_eq!(
        allowed
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://console.example.com"),
        "the configured origin must be allowed"
    );

    let refused = client
        .request(
            reqwest::Method::OPTIONS,
            format!("http://{addr}/druid/api/v1/basic"),
        )
        .header("Origin", "https://evil.example.com")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .unwrap();
    assert!(
        refused
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "an unconfigured origin must not be allowed"
    );
}
