//! Tests for MonitorViewServlet routes and auth middleware.
//!
//! Covers:
//! - Java-compatible JSON endpoints (/druid/datasource.json, /druid/sql.json, etc.)
//! - Auth middleware (login flow, session cookie, credential query params)
//! - Edge cases (404 for unknown paths, missing parameters, malformed requests)

mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use druid_admin::config::MonitorProperties;
use druid_admin::service::{EmptyAdminStatProvider, MonitorStatService, StaticDiscoveryClient};
use druid_admin::util::ReqwestHttpClient;

/// Helper: build a `MonitorViewServlet`-backed router with no auth configured.
async fn start_servlet_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let http_client = ReqwestHttpClient::new(Duration::from_secs(5), Duration::from_secs(10))
        .expect("HTTP client must build");
    let properties = Arc::new(MonitorProperties::default());
    let discovery_client = Arc::new(StaticDiscoveryClient::new(HashMap::new()));
    let service = Arc::new(
        MonitorStatService::new(discovery_client, properties, None, Arc::new(http_client))
            .with_admin_stat_provider(Arc::new(EmptyAdminStatProvider)),
    );
    let servlet = druid_admin::MonitorViewServlet::new(service);
    let app = servlet.router();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

/// Helper: build a `MonitorViewServlet`-backed router with auth enabled.
async fn start_servlet_server_with_auth(
    username: &str,
    password: &str,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let http_client = ReqwestHttpClient::new(Duration::from_secs(5), Duration::from_secs(10))
        .expect("HTTP client must build");
    let properties = Arc::new(MonitorProperties {
        login_username: Some(username.to_owned()),
        login_password: Some(password.to_owned()),
        ..Default::default()
    });
    let discovery_client = Arc::new(StaticDiscoveryClient::new(HashMap::new()));
    let service = Arc::new(
        MonitorStatService::new(
            discovery_client,
            properties.clone(),
            None,
            Arc::new(http_client),
        )
        .with_admin_stat_provider(Arc::new(EmptyAdminStatProvider)),
    );
    let servlet = druid_admin::MonitorViewServlet::new(service)
        .with_credentials(
            properties.login_username.clone(),
            properties.login_password.clone(),
        )
        .with_context_path(Some("/druid".to_owned()));
    let app = servlet.router();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

// ---- Route tests (no auth) ----

/// /druid/ root must redirect to /druid/index.html.
#[tokio::test]
async fn root_redirects_to_index() {
    let (addr, handle) = start_servlet_server().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://{addr}/druid/"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 302, "root must redirect");
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        location.contains("/druid/index.html"),
        "must redirect to index.html, got: {location}"
    );
    handle.abort();
}

/// /druid/datasource.json must return 200 with Java-compatible JSON.
#[tokio::test]
async fn datasource_json_returns_java_format() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/datasource.json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.get("ResultCode").is_some(), "must have ResultCode");
    assert!(body.get("Content").is_some(), "must have Content");
    handle.abort();
}

/// /druid/sql.json must return 200 with Java-compatible JSON.
#[tokio::test]
async fn sql_json_returns_java_format() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/sql.json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.get("ResultCode").is_some());
    handle.abort();
}

/// /druid/wall.json must return 200.
#[tokio::test]
async fn wall_json_returns_java_format() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/wall.json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.get("ResultCode").is_some());
    handle.abort();
}

/// /druid/serviceList.json must return the configured applications list.
#[tokio::test]
async fn service_list_returns_json_array() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/serviceList.json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.is_array(), "serviceList must be a JSON array");
    handle.abort();
}

/// /druid/api/datasources must return 200.
#[tokio::test]
async fn api_datasources_returns_200() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/api/datasources"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    handle.abort();
}

/// /metrics must return Prometheus text format.
#[tokio::test]
async fn metrics_returns_prometheus_format() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("text/plain"), "Content-Type must be text/plain");
    let body = resp.text().await.unwrap();
    assert!(body.contains("# TYPE"), "must contain TYPE lines");
    handle.abort();
}

/// Unknown .json route is handled by legacy_dispatch, which returns
/// an error ResultCode (not 404) -- matching Java behavior.
#[tokio::test]
async fn unknown_json_route_returns_error_result_code() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/nonexistent.json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "legacy dispatch returns 200");
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body.get("ResultCode").and_then(serde_json::Value::as_i64),
        Some(-1),
        "unknown route must return error ResultCode"
    );
    handle.abort();
}

/// Truly unknown route (no .json extension) must return 404.
#[tokio::test]
async fn unknown_non_json_route_returns_404() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/totally-unknown"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "non-JSON unknown route must be 404");
    handle.abort();
}

/// /druid/spring-detail.json without required params must return 422.
#[tokio::test]
async fn spring_detail_missing_params_returns_422() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/spring-detail.json"))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        422,
        "missing params must return 422 Unprocessable Entity"
    );
    handle.abort();
}

/// /druid/api/active without serviceId must return 422.
#[tokio::test]
async fn active_missing_service_id_returns_422() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/api/active"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 422, "missing serviceId must return 422");
    handle.abort();
}

/// /druid/api/active with serviceId but missing id must return 422.
#[tokio::test]
async fn active_missing_id_returns_422() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/api/active?serviceId=svc-1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 422, "missing id must return 422");
    handle.abort();
}

// ---- Auth middleware tests ----

/// With auth enabled, accessing a protected page without credentials must redirect to login.
#[tokio::test]
async fn auth_redirects_unauthenticated_to_login() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://{addr}/druid/datasource.json"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 302, "must redirect to login");
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        location.contains("login.html"),
        "must redirect to login.html, got: {location}"
    );
    handle.abort();
}

/// With auth enabled, login.html must be accessible without credentials.
#[tokio::test]
async fn auth_login_page_is_public() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let resp = reqwest::get(format!("http://{addr}/druid/login.html"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "login.html must be public");
    handle.abort();
}

/// With auth enabled, CSS assets must be accessible without credentials.
#[tokio::test]
async fn auth_css_assets_are_public() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let resp = reqwest::get(format!("http://{addr}/druid/css/style.css"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "CSS must be public");
    handle.abort();
}

/// submitLogin with correct credentials must return "success" and set a session cookie.
#[tokio::test]
async fn submit_login_success_sets_session_cookie() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .post(format!("http://{addr}/druid/submitLogin"))
        .form(&[("loginUsername", "admin"), ("loginPassword", "secret")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Read headers before consuming the body.
    let cookies: Vec<_> = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(String::from)
        .collect();
    let body = resp.text().await.unwrap();
    assert_eq!(body, "success", "login must return success");
    assert!(
        cookies.iter().any(|c| c.contains("druid-session")),
        "must set druid-session cookie"
    );
    handle.abort();
}

/// submitLogin with wrong credentials must return "error".
#[tokio::test]
async fn submit_login_wrong_password_returns_error() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/druid/submitLogin"))
        .form(&[("loginUsername", "admin"), ("loginPassword", "wrong")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "error", "wrong password must return error");
    handle.abort();
}

/// Accessing a protected page with valid query-param credentials must succeed.
#[tokio::test]
async fn auth_query_param_credentials_grant_access() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let resp = reqwest::get(format!(
        "http://{addr}/druid/datasource.json?loginUsername=admin&loginPassword=secret"
    ))
    .await
    .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "query-param credentials must grant access"
    );
    handle.abort();
}

/// Accessing a protected page with a valid session cookie must succeed.
#[tokio::test]
async fn auth_session_cookie_grants_access() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // Login first
    let resp = client
        .post(format!("http://{addr}/druid/submitLogin"))
        .form(&[("loginUsername", "admin"), ("loginPassword", "secret")])
        .send()
        .await
        .unwrap();
    let session_cookie = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .find_map(|v| {
            let s = v.to_str().ok()?;
            s.split(';').next().map(String::from)
        })
        .expect("must have session cookie");

    // Access protected page with session cookie
    let resp = client
        .get(format!("http://{addr}/druid/datasource.json"))
        .header("cookie", &session_cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "session cookie must grant access");
    handle.abort();
}

// ---- Static resource tests ----

/// Requesting a non-existent HTML page must return 404.
#[tokio::test]
async fn nonexistent_html_returns_404() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/nonexistent.html"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "nonexistent HTML must be 404");
    handle.abort();
}

/// Requesting a non-existent CSS asset must return 404.
#[tokio::test]
async fn nonexistent_css_returns_404() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/css/missing.css"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "missing CSS must be 404");
    handle.abort();
}

/// Requesting a non-existent JS asset must return 404.
#[tokio::test]
async fn nonexistent_js_returns_404() {
    let (addr, handle) = start_servlet_server().await;
    let resp = reqwest::get(format!("http://{addr}/druid/js/missing.js"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "missing JS must be 404");
    handle.abort();
}
