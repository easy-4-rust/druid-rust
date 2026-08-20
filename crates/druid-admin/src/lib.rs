//! Druid 管理端的 Axum 实现。
//!
//! 对应 Java 模块：`druid-admin`。本 crate 保留 Java 管理协议和 JSON
//! 字段，同时用可注入的发现与 SPI 替代 Spring Cloud/Kubernetes
//! 静态依赖。
//!
//! The standalone admin mode (binary `druid-admin`) uses an in-memory
//! [`repository::MetricsRepository`] instead of remote HTTP pull.

pub mod admin_state;
pub mod config;
pub mod druid_admin_application;
pub mod ingest;
pub mod model;
pub mod repository;
pub mod service;
pub mod servlet;
pub mod util;

pub use admin_state::AdminState;
pub use druid_admin_application::DruidAdminApplication;
pub use repository::MetricsRepository;
pub use servlet::monitor_view_servlet::{endpoint_list, MonitorViewServlet};
pub use servlet::StatViewServlet;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

/// Build the standalone Axum router with /health, REST API, /metrics, and static UI.
///
/// This router is used by the standalone `druid-admin` binary and
/// integration tests. It does NOT include the gRPC ingest service
/// (that runs on a separate port).
#[must_use]
pub fn standalone_router(repo: MetricsRepository) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/druid/datasource.json", get(datasource_handler))
        .route("/druid/sql.json", get(sql_handler))
        .route("/druid/wall.json", get(wall_handler))
        .route("/druid/api.json", get(api_handler))
        .route("/metrics", get(metrics_handler))
        .route("/druid/", get(ui_index))
        .route("/druid/index.html", get(ui_index))
        .route("/druid/datasource.html", get(ui_datasource))
        .route("/druid/sql.html", get(ui_sql))
        .route("/druid/wall.html", get(ui_wall))
        .route("/druid/api.html", get(ui_api))
        .route("/druid/login.html", get(ui_login))
        .route("/druid/spring.html", get(ui_spring))
        .route("/druid/weburi.html", get(ui_weburi))
        .route("/druid/websession.html", get(ui_websession))
        .route("/druid/webapp.html", get(ui_webapp))
        .route("/druid/css/{asset}", get(ui_css))
        .route("/druid/js/{asset}", get(ui_js))
        .with_state(repo)
}

/// Serve static HTML from the embedded resources.
fn static_resource(resource: &str) -> Option<(&'static str, &'static str)> {
    let (body, content_type) = match resource {
        "index.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/index.html"
            )),
            "text/html; charset=utf-8",
        ),
        "datasource.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/datasource.html"
            )),
            "text/html; charset=utf-8",
        ),
        "sql.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/sql.html"
            )),
            "text/html; charset=utf-8",
        ),
        "wall.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/wall.html"
            )),
            "text/html; charset=utf-8",
        ),
        "api.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/api.html"
            )),
            "text/html; charset=utf-8",
        ),
        "login.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/login.html"
            )),
            "text/html; charset=utf-8",
        ),
        "header.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/header.html"
            )),
            "text/html; charset=utf-8",
        ),
        "spring.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/spring.html"
            )),
            "text/html; charset=utf-8",
        ),
        "weburi.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/weburi.html"
            )),
            "text/html; charset=utf-8",
        ),
        "websession.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/websession.html"
            )),
            "text/html; charset=utf-8",
        ),
        "webapp.html" => (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/webapp.html"
            )),
            "text/html; charset=utf-8",
        ),
        _ => return None,
    };
    Some((body, content_type))
}

/// Serve static CSS from the embedded resources.
fn static_css(asset: &str) -> Option<(&'static str, &'static str)> {
    match asset {
        "bootstrap.min.css" => Some((
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/css/bootstrap.min.css"
            )),
            "text/css; charset=utf-8",
        )),
        "style.css" => Some((
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/css/style.css"
            )),
            "text/css; charset=utf-8",
        )),
        _ => None,
    }
}

/// Serve static JS from the embedded resources.
fn static_js(asset: &str) -> Option<(&'static str, &'static str)> {
    match asset {
        "bootstrap.min.js" => Some((
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/js/bootstrap.min.js"
            )),
            "application/javascript; charset=utf-8",
        )),
        "jquery.min.js" => Some((
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/js/jquery.min.js"
            )),
            "application/javascript; charset=utf-8",
        )),
        "common.js" => Some((
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/js/common.js"
            )),
            "application/javascript; charset=utf-8",
        )),
        "doT.js" => Some((
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/js/doT.js"
            )),
            "application/javascript; charset=utf-8",
        )),
        "lang.js" => Some((
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/support/http/resources/js/lang.js"
            )),
            "application/javascript; charset=utf-8",
        )),
        _ => None,
    }
}

fn serve_static(resource: &str) -> axum::response::Response {
    match static_resource(resource) {
        Some((body, ct)) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, ct)],
            body,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn ui_index() -> impl IntoResponse {
    serve_static("index.html")
}

async fn ui_datasource() -> impl IntoResponse {
    serve_static("datasource.html")
}

async fn ui_sql() -> impl IntoResponse {
    serve_static("sql.html")
}

async fn ui_wall() -> impl IntoResponse {
    serve_static("wall.html")
}

async fn ui_api() -> impl IntoResponse {
    serve_static("api.html")
}

async fn ui_login() -> impl IntoResponse {
    serve_static("login.html")
}

async fn ui_spring() -> impl IntoResponse {
    serve_static("spring.html")
}

async fn ui_weburi() -> impl IntoResponse {
    serve_static("weburi.html")
}

async fn ui_websession() -> impl IntoResponse {
    serve_static("websession.html")
}

async fn ui_webapp() -> impl IntoResponse {
    serve_static("webapp.html")
}

async fn ui_css(axum::extract::Path(asset): axum::extract::Path<String>) -> impl IntoResponse {
    match static_css(&asset) {
        Some((body, ct)) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, ct)],
            body,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn ui_js(axum::extract::Path(asset): axum::extract::Path<String>) -> impl IntoResponse {
    match static_js(&asset) {
        Some((body, ct)) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, ct)],
            body,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// /health handler: returns 200 OK with `{"status":"ok"}`.
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

/// /druid/datasource.json: returns Java-compatible datasource stats.
///
/// Response format: `{"ResultCode": 1, "Content": [...]}`
async fn datasource_handler(
    axum::extract::State(repo): axum::extract::State<MetricsRepository>,
) -> impl IntoResponse {
    let entries = repo.all_datasources();
    let content: Vec<Value> = entries
        .into_iter()
        .map(|e| serde_json::to_value(e.datasource).unwrap_or_default())
        .collect();
    (
        StatusCode::OK,
        Json(json!({"ResultCode": 1i32, "Content": content})),
    )
}

/// /druid/sql.json: returns Java-compatible SQL stats.
///
/// Response format: `{"ResultCode": 1, "Content": [...]}`
async fn sql_handler(
    axum::extract::State(repo): axum::extract::State<MetricsRepository>,
) -> impl IntoResponse {
    let stats = repo.all_sql_stats();
    let content: Vec<Value> = stats
        .into_iter()
        .map(|s| serde_json::to_value(s).unwrap_or_default())
        .collect();
    (
        StatusCode::OK,
        Json(json!({"ResultCode": 1i32, "Content": content})),
    )
}

/// /druid/wall.json: returns Java-compatible wall (firewall) stats.
///
/// Response format: `{"ResultCode": 1, "Content": {...}}`
async fn wall_handler(
    axum::extract::State(repo): axum::extract::State<MetricsRepository>,
) -> impl IntoResponse {
    let wall = repo.aggregated_wall();
    let content = serde_json::to_value(wall.content).unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({"ResultCode": 1i32, "Content": content})),
    )
}

/// /druid/api.json: returns API description with endpoint list.
///
/// Response format: `{"ResultCode": 1, "Content": {endpoints: [...]}}`
async fn api_handler() -> impl IntoResponse {
    let endpoints = serde_json::from_str::<Value>(endpoint_list()).unwrap_or_else(|_| json!([]));
    (
        StatusCode::OK,
        Json(json!({
            "ResultCode": 1i32,
            "Content": {
                "endpoints": endpoints,
                "version": "standalone-v1"
            }
        })),
    )
}

/// /metrics: returns Prometheus text format metrics from the repository.
///
/// Outputs metric families in Prometheus exposition format (text/plain; version=0.0.4).
/// Labels are restricted to allowed set (service, instance, datasource, db_type, driver).
async fn metrics_handler(
    axum::extract::State(repo): axum::extract::State<MetricsRepository>,
) -> impl IntoResponse {
    let mut output = String::with_capacity(4096);

    // Admin runtime metrics
    let (ingest_total, rejected_total) = repo.counters();
    let ds_count = repo.len();

    output.push_str("# HELP druid_admin_ingest_total Total number of ingest batches received.\n");
    output.push_str("# TYPE druid_admin_ingest_total counter\n");
    output.push_str(&format!("druid_admin_ingest_total {ingest_total}\n"));

    output.push_str("# HELP druid_admin_ingest_rejected_total Total number of rejected (stale/duplicate) ingest batches.\n");
    output.push_str("# TYPE druid_admin_ingest_rejected_total counter\n");
    output.push_str(&format!(
        "druid_admin_ingest_rejected_total {rejected_total}\n"
    ));

    output
        .push_str("# HELP druid_admin_datasource_count Number of tracked datasource instances.\n");
    output.push_str("# TYPE druid_admin_datasource_count gauge\n");
    output.push_str(&format!("druid_admin_datasource_count {ds_count}\n"));

    // Per-datasource metrics
    for entry in repo.all_datasources() {
        let ds = &entry.datasource;
        let service = entry.datasource.name.as_deref().unwrap_or("unknown");
        let db_type = entry.datasource.db_type.as_deref().unwrap_or("unknown");

        let labels = format!(
            "service=\"{service}\",datasource=\"{identity}\",db_type=\"{db_type}\"",
            identity = ds.identity
        );

        output.push_str("# HELP druid_admin_datasource_active_count Active connection count.\n");
        output.push_str("# TYPE druid_admin_datasource_active_count gauge\n");
        output.push_str(&format!(
            "druid_admin_datasource_active_count{{{labels}}} {val}\n",
            val = ds.active_count
        ));

        output.push_str("# HELP druid_admin_datasource_pooling_count Pooled connection count.\n");
        output.push_str("# TYPE druid_admin_datasource_pooling_count gauge\n");
        output.push_str(&format!(
            "druid_admin_datasource_pooling_count{{{labels}}} {val}\n",
            val = ds.pooling_count
        ));

        output.push_str("# HELP druid_admin_datasource_execute_count Total execute count.\n");
        output.push_str("# TYPE druid_admin_datasource_execute_count counter\n");
        output.push_str(&format!(
            "druid_admin_datasource_execute_count{{{labels}}} {val}\n",
            val = ds.execute_count
        ));

        output.push_str("# HELP druid_admin_datasource_error_count Total error count.\n");
        output.push_str("# TYPE druid_admin_datasource_error_count counter\n");
        output.push_str(&format!(
            "druid_admin_datasource_error_count{{{labels}}} {val}\n",
            val = ds.error_count
        ));

        output.push_str("# HELP druid_admin_datasource_commit_count Total commit count.\n");
        output.push_str("# TYPE druid_admin_datasource_commit_count counter\n");
        output.push_str(&format!(
            "druid_admin_datasource_commit_count{{{labels}}} {val}\n",
            val = ds.commit_count
        ));

        output.push_str("# HELP druid_admin_datasource_rollback_count Total rollback count.\n");
        output.push_str("# TYPE druid_admin_datasource_rollback_count counter\n");
        output.push_str(&format!(
            "druid_admin_datasource_rollback_count{{{labels}}} {val}\n",
            val = ds.rollback_count
        ));
    }

    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        output,
    )
}
