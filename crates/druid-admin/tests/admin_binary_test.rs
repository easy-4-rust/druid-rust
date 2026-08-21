//! RED tests for the standalone druid-admin binary.
//!
//! Task 1 Step 1: Tests for dual-port binding (HTTP 8080, gRPC 9090),
//! /health route, and graceful shutdown.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::Request;
use druid_metrics::aggregator::AggregatedBatch;
use druid_metrics::grpc_client::{GrpcAuthorization, GrpcClientConfig, GrpcMetricsClient};
use druid_metrics::protocol::{PoolSnapshotMsg, SnapshotEntry};
use tokio::io::AsyncWriteExt as _;
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint};
use tower::ServiceExt as _;

const SECURITY_ENV_VARS: &[&str] = &[
    "DRUID_ADMIN_HTTP_ADDR",
    "DRUID_ADMIN_GRPC_ADDR",
    "DRUID_ADMIN_TLS_CERT_PATH",
    "DRUID_ADMIN_TLS_KEY_PATH",
    "DRUID_ADMIN_AUTH_MODE",
    "DRUID_ADMIN_LOGIN_USERNAME",
    "DRUID_ADMIN_LOGIN_PASSWORD",
    "DRUID_ADMIN_SHUTDOWN_TIMEOUT_MS",
];

fn admin_command() -> tokio::process::Command {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_druid-admin"));
    for variable in SECURITY_ENV_VARS {
        command.env_remove(variable);
    }
    command
}

#[derive(Clone, Default)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

struct CapturedLogWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CapturedLogWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut *self.0.lock().expect("capture lock"), buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for CapturedLogs {
    type Writer = CapturedLogWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        CapturedLogWriter(Arc::clone(&self.0))
    }
}

impl CapturedLogs {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("capture lock").clone())
            .expect("tracing output is UTF-8")
    }
}

#[test]
fn login_request_debug_redacts_password() {
    let login = druid_admin::servlet::LoginRequest {
        login_username: "admin".to_owned(),
        login_password: "debug-login-password-must-stay-secret".to_owned(),
    };
    let debug = format!("{login:?}");
    assert!(!debug.contains("debug-login-password-must-stay-secret"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn basic_authorization_scheme_is_ascii_case_insensitive() {
    let auth = druid_admin::config::AdminAuthMode::basic("admin", "secret");
    assert!(auth.authorizes_header(Some("basic YWRtaW46c2VjcmV0")));
    assert!(auth.authorizes_header(Some("BASIC YWRtaW46c2VjcmV0")));
    assert!(auth.authorizes_credentials("admin", "secret"));
    for malformed in [
        None,
        Some("Bearer YWRtaW46c2VjcmV0"),
        Some("Basic"),
        Some("Basic !!!"),
        Some("Basic /w=="),
        Some("Basic YWRtaW4="),
        Some("Basic YWRtaW46"),
        Some("Basic  YWRtaW46c2VjcmV0"),
    ] {
        assert!(!auth.authorizes_header(malformed), "{malformed:?}");
    }
    for (username, password) in [
        ("admin", "x"),
        ("admin", "secret-with-a-different-length"),
        ("administrator", "secret"),
        ("admin:other", "secret"),
        ("", "secret"),
        ("admin", ""),
    ] {
        assert!(!auth.authorizes_credentials(username, password));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn production_trace_logs_path_without_query_credentials() {
    let logs = CapturedLogs::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .without_time()
        .with_writer(logs.clone())
        .finish();
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    let app = druid_admin::standalone_router(druid_admin::MetricsRepository::new());
    let response = app
        .oneshot(
            Request::builder()
                .uri(
                    "/druid/datasource.json?loginPassword=trace-password-secret&token=trace-token-secret",
                )
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let output = logs.text();
    assert!(output.contains("/druid/datasource.json"), "{output}");
    for secret in [
        "trace-password-secret",
        "trace-token-secret",
        "loginPassword",
        "token=",
    ] {
        assert!(
            !output.contains(secret),
            "tracing leaked {secret}: {output}"
        );
    }
}

#[tokio::test]
async fn secure_defaults() {
    let defaults = druid_admin::config::AdminServerConfig::default();
    assert_eq!(defaults.http_addr(), "127.0.0.1:8080".parse().unwrap());
    assert_eq!(defaults.grpc_addr(), "127.0.0.1:9090".parse().unwrap());
    assert!(defaults.http_addr().ip().is_loopback());
    assert!(defaults.grpc_addr().ip().is_loopback());

    let remote_http = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve remote HTTP port")
        .local_addr()
        .expect("remote HTTP port");
    let grpc_addr = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve gRPC port")
        .local_addr()
        .expect("gRPC port");
    let mut insecure_remote = admin_command()
        .env(
            "DRUID_ADMIN_HTTP_ADDR",
            format!("0.0.0.0:{}", remote_http.port()),
        )
        .env("DRUID_ADMIN_GRPC_ADDR", grpc_addr.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn insecure remote admin");
    let status = tokio::time::timeout(Duration::from_secs(2), insecure_remote.wait())
        .await
        .expect("insecure remote configuration must fail before serving")
        .expect("wait for insecure remote admin");
    assert!(
        !status.success(),
        "non-loopback HTTP without TLS and auth must fail closed"
    );
}

#[test]
fn monitor_properties_debug_redacts_password() {
    let properties = druid_admin::config::MonitorProperties {
        login_username: Some("admin".to_owned()),
        login_password: Some("debug-must-not-leak-this-password".to_owned()),
        ..Default::default()
    };
    let debug = format!("{properties:?}");
    assert!(!debug.contains("debug-must-not-leak-this-password"));
    assert!(debug.contains("[REDACTED]"));

    let config = druid_admin::config::AdminServerConfig::try_new(
        "127.0.0.1:8080".parse().unwrap(),
        "127.0.0.1:9090".parse().unwrap(),
        None,
        druid_admin::config::AdminAuthMode::basic("admin", "debug-must-not-leak-this-password"),
    )
    .expect("loopback config");
    let config_debug = format!("{config:?}");
    assert!(!config_debug.contains("debug-must-not-leak-this-password"));
    assert!(config_debug.contains("[REDACTED]"));
}

#[test]
fn every_non_loopback_bind_requires_tls_and_auth() {
    use druid_admin::config::{AdminAuthMode, AdminServerConfig, AdminTlsFiles};

    let loopback_http = "127.0.0.1:8080".parse().unwrap();
    let loopback_grpc = "[::1]:9090".parse().unwrap();
    let remote_addresses = [
        "0.0.0.0:8080".parse().unwrap(),
        "[::]:8080".parse().unwrap(),
        "[2001:db8::10]:8080".parse().unwrap(),
    ];
    for remote in remote_addresses {
        let no_security =
            AdminServerConfig::try_new(remote, loopback_grpc, None, AdminAuthMode::Disabled);
        assert!(no_security.is_err(), "remote address {remote}");

        let missing_auth = AdminServerConfig::try_new(
            remote,
            loopback_grpc,
            Some(AdminTlsFiles::new("certificate.pem", "private-key.pem")),
            AdminAuthMode::Disabled,
        );
        assert!(missing_auth.is_err(), "remote address {remote}");

        let missing_tls = AdminServerConfig::try_new(
            remote,
            loopback_grpc,
            None,
            AdminAuthMode::basic("admin", "secret"),
        );
        assert!(missing_tls.is_err(), "remote address {remote}");

        let secured = AdminServerConfig::try_new(
            remote,
            loopback_grpc,
            Some(AdminTlsFiles::new("certificate.pem", "private-key.pem")),
            AdminAuthMode::basic("admin", "secret"),
        );
        assert!(secured.is_ok(), "remote address {remote}");
    }

    assert!(AdminServerConfig::try_new(
        loopback_http,
        loopback_grpc,
        None,
        AdminAuthMode::Disabled,
    )
    .is_ok());
    assert!(AdminServerConfig::try_new(
        loopback_http,
        "[::]:9090".parse().unwrap(),
        None,
        AdminAuthMode::Disabled,
    )
    .is_err());
}

#[tokio::test]
async fn invalid_tls_material_fails_before_listening() {
    let http_port = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve HTTP port")
        .local_addr()
        .expect("HTTP port")
        .port();
    let grpc_addr = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve gRPC port")
        .local_addr()
        .expect("gRPC port");
    let invalid_pem = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let mut child = admin_command()
        .env("DRUID_ADMIN_HTTP_ADDR", format!("0.0.0.0:{http_port}"))
        .env("DRUID_ADMIN_GRPC_ADDR", grpc_addr.to_string())
        .env("DRUID_ADMIN_TLS_CERT_PATH", &invalid_pem)
        .env("DRUID_ADMIN_TLS_KEY_PATH", &invalid_pem)
        .env("DRUID_ADMIN_AUTH_MODE", "basic")
        .env("DRUID_ADMIN_LOGIN_USERNAME", "admin")
        .env("DRUID_ADMIN_LOGIN_PASSWORD", "secret")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn admin with invalid TLS material");
    let status =
        if let Ok(result) = tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
            result.expect("wait for invalid TLS child")
        } else {
            child.kill().await.expect("kill insecure TLS child");
            let _ = child.wait().await;
            panic!("invalid TLS material must fail before either listener starts");
        };
    assert!(!status.success());
}

fn secure_grpc_config(port: u16, authorization: Option<GrpcAuthorization>) -> GrpcClientConfig {
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(include_bytes!(
            "fixtures/admin-test-cert.pem"
        )))
        .domain_name("localhost");
    let endpoint = Endpoint::from_shared(format!("https://127.0.0.1:{port}"))
        .expect("valid secure gRPC endpoint")
        .tls_config(tls)
        .expect("valid client TLS config");
    GrpcClientConfig {
        endpoint,
        service_name: "secure-orders-runtime".to_owned(),
        instance_id: "secure-instance".to_owned(),
        boot_id: "secure-boot".to_owned(),
        ack_window_capacity: 8,
        shutdown_timeout: Duration::from_secs(3),
        authorization,
    }
}

#[tokio::test]
async fn remote_http_and_grpc_use_tls_and_require_authentication() {
    let http_port = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve HTTP port")
        .local_addr()
        .expect("HTTP port")
        .port();
    let grpc_port = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve gRPC port")
        .local_addr()
        .expect("gRPC port")
        .port();
    let fixture_dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut child = admin_command()
        .env("DRUID_ADMIN_HTTP_ADDR", format!("0.0.0.0:{http_port}"))
        .env("DRUID_ADMIN_GRPC_ADDR", format!("0.0.0.0:{grpc_port}"))
        .env(
            "DRUID_ADMIN_TLS_CERT_PATH",
            format!("{fixture_dir}/admin-test-cert.pem"),
        )
        .env(
            "DRUID_ADMIN_TLS_KEY_PATH",
            format!("{fixture_dir}/admin-test-key.pem"),
        )
        .env("DRUID_ADMIN_AUTH_MODE", "basic")
        .env("DRUID_ADMIN_LOGIN_USERNAME", "admin")
        .env("DRUID_ADMIN_LOGIN_PASSWORD", "secret")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn secure remote admin");
    let certificate =
        reqwest::Certificate::from_pem(include_bytes!("fixtures/admin-test-cert.pem"))
            .expect("valid test certificate");
    let client = reqwest::Client::builder()
        .add_root_certificate(certificate)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("secure HTTP client");
    let base_url = format!("https://127.0.0.1:{http_port}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().expect("poll secure admin child") {
            panic!("secure admin exited during startup with {status}");
        }
        let attempt_error = match client.get(format!("{base_url}/health")).send().await {
            Ok(response) if response.status().is_success() => break,
            Ok(response) => format!("HTTP {}", response.status()),
            Err(error) => format!("{error:?}"),
        };
        if tokio::time::Instant::now() >= deadline {
            child.kill().await.expect("kill non-TLS admin child");
            let _ = child.wait().await;
            panic!("remote HTTP listener did not complete a TLS handshake: {attempt_error}");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert!(
        reqwest::get(format!("http://127.0.0.1:{http_port}/health"))
            .await
            .is_err(),
        "remote HTTP listener must not accept plaintext"
    );
    assert_eq!(
        client
            .get(format!("{base_url}/druid/login.html"))
            .send()
            .await
            .expect("public login page")
            .status(),
        reqwest::StatusCode::OK
    );
    let login = client
        .post(format!("{base_url}/druid/submitLogin"))
        .form(&[("loginUsername", "admin"), ("loginPassword", "secret")])
        .send()
        .await
        .expect("TLS login request");
    assert!(
        login
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .any(|cookie| cookie.contains("; Secure")),
        "TLS login must issue a Secure session cookie"
    );
    for protected_path in ["/druid/datasource.json", "/metrics"] {
        assert_eq!(
            client
                .get(format!("{base_url}{protected_path}"))
                .send()
                .await
                .expect("protected request")
                .status(),
            reqwest::StatusCode::FOUND,
            "unauthenticated {protected_path} redirects to login"
        );
    }
    assert_eq!(
        client
            .get(format!("{base_url}/druid/datasource.json"))
            .header("authorization", "Basic aW52YWxpZA==")
            .send()
            .await
            .expect("invalid protected request")
            .status(),
        reqwest::StatusCode::FOUND,
        "invalid Basic credentials must not cross the HTTP auth boundary"
    );
    assert_eq!(
        client
            .get(format!("{base_url}/druid/datasource.json"))
            .header("authorization", "Basic YWRtaW46c2VjcmV0")
            .send()
            .await
            .expect("authenticated protected request")
            .status(),
        reqwest::StatusCode::OK
    );

    let missing = GrpcMetricsClient::connect(secure_grpc_config(grpc_port, None)).await;
    assert!(
        missing.is_err(),
        "missing production credential is rejected"
    );
    let invalid = GrpcMetricsClient::connect(secure_grpc_config(
        grpc_port,
        Some(GrpcAuthorization::basic("admin", "wrong-secret")),
    ))
    .await;
    assert!(
        invalid.is_err(),
        "invalid production credential is rejected"
    );

    let authenticated_config =
        secure_grpc_config(grpc_port, Some(GrpcAuthorization::basic("admin", "secret")));
    let config_debug = format!("{authenticated_config:?}");
    assert!(!config_debug.contains("secret"));
    assert!(!config_debug.contains("YWRtaW46c2VjcmV0"));
    assert!(config_debug.contains("[REDACTED]"));
    let metrics_client = GrpcMetricsClient::connect(authenticated_config)
        .await
        .expect("production metrics client authenticates over TLS");
    metrics_client
        .send_batch(AggregatedBatch {
            entries: vec![SnapshotEntry {
                datasource_id: 901,
                datasource_name: "secure-orders".to_owned(),
                driver_name: Some("postgresql".to_owned()),
                pool: Some(PoolSnapshotMsg {
                    active_count: 7,
                    idle_count: 3,
                    max_active: 16,
                    max_idle: 8,
                    waiting_count: 0,
                }),
                sql_stats: Vec::new(),
                wall_check_count: 0,
                wall_deny_count: 0,
                wall_violation_count: 0,
                sampling_time_millis: 1_755_734_400_000,
            }],
            is_full_snapshot: true,
        })
        .await
        .expect("authenticated batch queued");
    metrics_client
        .shutdown(Duration::from_secs(3))
        .await
        .expect("authenticated batch ACKed and client joined");
    let projected: serde_json::Value = client
        .get(format!("{base_url}/druid/datasource.json"))
        .header("authorization", "Basic YWRtaW46c2VjcmV0")
        .send()
        .await
        .expect("authenticated projection request")
        .json()
        .await
        .expect("projection JSON");
    assert_eq!(projected["Content"][0]["Identity"], 901);

    child.kill().await.expect("stop secure admin child");
    let _ = child.wait().await;
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_forces_held_connection_exit_before_shutdown_deadline() {
    let http_addr = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve HTTP address")
        .local_addr()
        .expect("HTTP address");
    let grpc_addr = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve gRPC address")
        .local_addr()
        .expect("gRPC address");
    let mut child = admin_command()
        .env("DRUID_ADMIN_HTTP_ADDR", http_addr.to_string())
        .env("DRUID_ADMIN_GRPC_ADDR", grpc_addr.to_string())
        .env("DRUID_ADMIN_SHUTDOWN_TIMEOUT_MS", "200")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn admin for shutdown deadline");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if tokio::net::TcpStream::connect(http_addr).await.is_ok() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "admin did not start before shutdown test deadline"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let mut held = tokio::net::TcpStream::connect(http_addr)
        .await
        .expect("open held HTTP connection");
    held.write_all(
        b"POST /druid/submitLogin HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 100000\r\n\r\nloginUsername=admin&",
    )
    .await
    .expect("write partial request body");
    assert!(
        tokio::time::timeout(Duration::from_millis(100), held.readable())
            .await
            .is_err(),
        "partial request remains in flight before SIGTERM"
    );

    let pid = child.id().expect("child process ID");
    let kill_status = tokio::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .await
        .expect("send SIGTERM");
    assert!(kill_status.success(), "SIGTERM command succeeds");
    let status =
        if let Ok(result) = tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
            result.expect("wait for shutdown-deadline child")
        } else {
            child.kill().await.expect("kill hung admin child");
            let _ = child.wait().await;
            panic!("SIGTERM must not wait indefinitely for a held HTTP connection");
        };
    assert!(
        !status.success(),
        "deadline abort must return an explicit process failure"
    );
    drop(held);
    tokio::net::TcpListener::bind(http_addr)
        .await
        .expect("HTTP listener is released after forced shutdown");
    tokio::net::TcpListener::bind(grpc_addr)
        .await
        .expect("gRPC listener is released after forced shutdown");
}

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
