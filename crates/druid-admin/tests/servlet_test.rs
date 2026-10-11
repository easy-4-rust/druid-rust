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

use druid_admin::config::AdminAuthMode;
use druid_admin::config::MonitorProperties;
use druid_admin::repository::MetricsRepository;
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

#[tokio::test]
async fn malformed_legacy_query_does_not_echo_sensitive_input() {
    let (addr, handle) = start_servlet_server().await;
    let response = reqwest::get(format!(
        "http://{addr}/druid/unused.json?page=credential_marker_9381"
    ))
    .await
    .expect("malformed legacy request");
    assert_eq!(response.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    let body = response.text().await.expect("error response");
    assert!(body.contains("invalid parameter page"));
    assert!(!body.contains("credential_marker_9381"));
    handle.abort();
}

#[test]
fn admin_error_display_omits_untrusted_urls_and_parameter_values() {
    use druid_admin::service::MonitorStatServiceError;
    use druid_admin::util::HttpError;

    let url = "https://user:credential_marker_9381@backend.example/druid";
    let downstream = HttpError::Status {
        url: url.to_owned(),
        status: reqwest::StatusCode::BAD_GATEWAY,
    };
    let invalid = MonitorStatServiceError::InvalidParameter {
        name: "page",
        value: "credential_marker_9381".to_owned(),
    };
    let unknown = MonitorStatServiceError::UnknownServiceId("credential_marker_9381".into());
    for rendered in [
        downstream.to_string(),
        invalid.to_string(),
        unknown.to_string(),
    ] {
        assert!(!rendered.contains("credential_marker_9381"), "{rendered}");
    }
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
    assert!(
        cookies.iter().all(|cookie| !cookie.contains("; Secure")),
        "plain loopback HTTP must not issue an unusable Secure-only session cookie"
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

/// Credentials in a URL must not bypass authentication because URLs are commonly logged.
#[tokio::test]
async fn auth_query_param_credentials_do_not_bypass_login() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!(
            "http://{addr}/druid/datasource.json?loginUsername=admin&loginPassword=secret"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        302,
        "query-param credentials must not bypass login"
    );
    handle.abort();
}

/// Standard HTTP Basic credentials may reuse the configured servlet credentials.
#[tokio::test]
async fn auth_basic_header_grants_access() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/druid/datasource.json"))
        .header("authorization", "Basic YWRtaW46c2VjcmV0")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "valid Basic credentials grant access");
    handle.abort();
}

/// Prometheus output is management data and must share the protected route boundary.
#[tokio::test]
async fn auth_metrics_requires_login() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://{addr}/metrics"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 302, "metrics must redirect to login");
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

#[tokio::test]
async fn login_session_capacity_evicts_oldest_cookie_without_rejecting_new_login() {
    let (addr, handle) = start_servlet_server_with_auth("admin", "secret").await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let mut first_cookie = None;
    let mut newest_cookie = None;
    for index in 0..=1_024 {
        let response = client
            .post(format!("http://{addr}/druid/submitLogin"))
            .form(&[("loginUsername", "admin"), ("loginPassword", "secret")])
            .send()
            .await
            .expect("login request completes");
        assert_eq!(response.status(), 200, "login {index} succeeds");
        let cookie = response
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .expect("successful login issues a session cookie")
            .to_owned();
        if first_cookie.is_none() {
            first_cookie = Some(cookie.clone());
        }
        newest_cookie = Some(cookie);
    }

    let resource = format!("http://{addr}/druid/datasource.json");
    let old_response = client
        .get(&resource)
        .header("cookie", first_cookie.expect("first cookie"))
        .send()
        .await
        .expect("old session lookup completes");
    assert_eq!(old_response.status(), 302, "oldest session is evicted");
    let new_response = client
        .get(&resource)
        .header("cookie", newest_cookie.expect("newest cookie"))
        .send()
        .await
        .expect("new session lookup completes");
    assert_eq!(new_response.status(), 200, "newest session remains valid");
    handle.abort();
}

#[tokio::test]
async fn changing_admin_credentials_invalidates_existing_session_cookie() {
    let servlet = druid_admin::MonitorViewServlet::from_repository(MetricsRepository::new())
        .with_auth_mode(AdminAuthMode::basic("admin", "old-secret"));
    let first_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let first_addr = first_listener.local_addr().unwrap();
    let first_router = servlet.clone().router();
    let first_server = tokio::spawn(async move {
        axum::serve(first_listener, first_router).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let login = client
        .post(format!("http://{first_addr}/druid/submitLogin"))
        .form(&[("loginUsername", "admin"), ("loginPassword", "old-secret")])
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let cookie = login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let second_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second_addr = second_listener.local_addr().unwrap();
    let second_router = servlet
        .with_auth_mode(AdminAuthMode::basic("admin", "new-secret"))
        .router();
    let second_server = tokio::spawn(async move {
        axum::serve(second_listener, second_router).await.unwrap();
    });
    let protected = format!("http://{second_addr}/druid/datasource.json");
    let old_session = client
        .get(&protected)
        .header("cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(
        old_session.status(),
        302,
        "old auth session must be rejected"
    );
    let new_login = client
        .post(format!("http://{second_addr}/druid/submitLogin"))
        .form(&[("loginUsername", "admin"), ("loginPassword", "new-secret")])
        .send()
        .await
        .unwrap();
    assert_eq!(new_login.status(), 200);
    let new_cookie = new_login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    assert_eq!(new_login.text().await.unwrap(), "success");
    let current_session = client
        .get(&protected)
        .header("cookie", new_cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(current_session.status(), 200);
    first_server.abort();
    second_server.abort();
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
