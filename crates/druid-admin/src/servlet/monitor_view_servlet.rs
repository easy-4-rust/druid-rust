#![allow(clippy::case_sensitive_file_extension_comparisons)]
#![allow(clippy::unused_async)] // axum handlers require async signature even when no .await

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{to_bytes, Body};
use axum::extract::{Form, OriginalUri, Path, Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_valid::Valid;
use druid_metrics::ServiceIdentity;
use parking_lot::RwLock;
use serde_json::{json, Value};
use tokio_metrics::TaskMonitor;

use crate::config::AdminAuthMode;
use crate::model::dto::DataSourceResult;
use crate::repository::{DataSourceId, MetricsRepository, MetricsRepositoryConfig};
use crate::security::AdminRedactionPolicy;
use crate::service::{MonitorStatService, MonitorStatServiceError, StatQuery};
use crate::CommandBroker;

use super::LoginRequest;

mod console_routes;
mod monitor_view_a;
mod monitor_view_b;

/// Session expiry duration (2 hours, matching typical Java Servlet session).
const SESSION_TTL: Duration = Duration::from_secs(7200);
const MAX_SESSIONS: usize = 1_024;
const MAX_MANAGEMENT_JSON_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Druid 管理协议的 Axum 路由。
///
/// 对应 Java: `com.alibaba.druid.admin.servlet.MonitorViewServlet`。
///
/// Sessions are stored with creation timestamps, a two-hour TTL, and a hard
/// capacity so repeated successful logins cannot grow memory without bound.
#[derive(Clone)]
pub struct MonitorViewServlet {
    backend: MonitorBackend,
    task_monitor: TaskMonitor,
    auth: AdminAuthMode,
    redaction_policy: AdminRedactionPolicy,
    /// Session token -> creation instant. Expired entries are pruned on access.
    sessions: Arc<RwLock<HashMap<String, Instant>>>,
    context_path: Arc<String>,
    secure_cookies: bool,
    command_broker: Option<CommandBroker>,
    metrics_appendix: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    remote_metrics_output_rejected_total: Arc<AtomicU64>,
}

#[derive(Clone)]
enum MonitorBackend {
    Remote(Arc<MonitorStatService>),
    Repository(MetricsRepository),
}

/// 返回稳定端点清单；保留早期 `druid-admin` 门面 API。
#[must_use]
pub fn endpoint_list() -> &'static str {
    MonitorViewServlet::endpoint_list()
}

fn required_json_string<'a>(body: &'a Value, field: &str) -> Result<&'a str, String> {
    body.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{field} must be a non-empty string"))
}

fn command_error_status(error: &str) -> StatusCode {
    if error.contains("conflicting canonical payload") {
        StatusCode::CONFLICT
    } else if error.contains("queue full") || error.contains("broker full") {
        StatusCode::TOO_MANY_REQUESTS
    } else if error.contains("deadline exceeded") {
        StatusCode::GATEWAY_TIMEOUT
    } else if error.contains("no live stream") || error.contains("shutting down") {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::BAD_REQUEST
    }
}

fn command_error_response(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({"success": false, "error": error}))).into_response()
}

fn command_broker_error_response(error: &str) -> Response {
    // Broker 诊断可能包含不可信的传输或驱动详情；只保留公开错误类别，
    // 不把原始诊断回显给管理客户端。
    let status = command_error_status(error);
    let message = match status {
        StatusCode::CONFLICT => "command ID conflicts with an existing request",
        StatusCode::TOO_MANY_REQUESTS => "command queue is full",
        StatusCode::GATEWAY_TIMEOUT => "command result timed out",
        StatusCode::SERVICE_UNAVAILABLE => "target service is unavailable",
        _ => "command request failed",
    };
    command_error_response(status, message)
}

async fn connection_response(
    state: MonitorViewServlet,
    id: i64,
    service_identity: ServiceIdentity,
) -> Response {
    match &state.backend {
        MonitorBackend::Remote(service) => {
            let service = Arc::clone(service);
            state
                .task_monitor
                .instrument(async move {
                    match service
                        .get_pooling_connection_info_by_data_source_id(
                            id,
                            service_identity.service_id(),
                        )
                        .await
                    {
                        Ok(result) => Json(result).into_response(),
                        Err(error) => error_response(error),
                    }
                })
                .await
        }
        MonitorBackend::Repository(repository) => repository
            .get_datasource(&DataSourceId {
                service_identity,
                identity: id,
            })
            .map_or_else(
                || StatusCode::NOT_FOUND.into_response(),
                |entry| Json(entry.datasource).into_response(),
            ),
    }
}

fn empty_success() -> Value {
    json!({
        "ResultCode": MonitorStatService::RESULT_CODE_SUCCESS,
        "Content": [],
    })
}

fn write_repository_metrics(body: &mut String, repository: &MetricsRepository) {
    let (ingest_total, rejected_total) = repository.counters();
    let (capacity_rejected_total, ttl_evicted_total, output_rejected_total) =
        repository.limit_counters();
    let datasource_count = repository.len();
    let repository_state_bytes = repository.retained_state_bytes();
    let _ = writeln!(
        body,
        "# HELP druid_admin_ingest_total Total number of ingest batches received.\n# TYPE druid_admin_ingest_total counter\ndruid_admin_ingest_total {ingest_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_capacity_rejected_total Total repository transactions rejected by configured capacity.\n# TYPE druid_admin_capacity_rejected_total counter\ndruid_admin_capacity_rejected_total {capacity_rejected_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_ttl_evicted_total Total datasource entries evicted after TTL.\n# TYPE druid_admin_ttl_evicted_total counter\ndruid_admin_ttl_evicted_total {ttl_evicted_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_metrics_output_rejected_total Total metrics expositions rejected for exceeding the byte budget.\n# TYPE druid_admin_metrics_output_rejected_total counter\ndruid_admin_metrics_output_rejected_total {output_rejected_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_metrics_output_limit_bytes Configured maximum metrics exposition bytes.\n# TYPE druid_admin_metrics_output_limit_bytes gauge\ndruid_admin_metrics_output_limit_bytes {}",
        repository.max_metrics_output_bytes()
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_ingest_rejected_total Total number of rejected ingest batches.\n# TYPE druid_admin_ingest_rejected_total counter\ndruid_admin_ingest_rejected_total {rejected_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_datasource_count Number of tracked datasource instances.\n# TYPE druid_admin_datasource_count gauge\ndruid_admin_datasource_count {datasource_count}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_repository_state_bytes Serialized bytes of retained datasource, SQL and wall projections.\n# TYPE druid_admin_repository_state_bytes gauge\ndruid_admin_repository_state_bytes {repository_state_bytes}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_repository_state_limit_bytes Configured maximum serialized repository state bytes.\n# TYPE druid_admin_repository_state_limit_bytes gauge\ndruid_admin_repository_state_limit_bytes {}",
        repository.max_total_state_bytes()
    );
    body.push_str(concat!(
        "# HELP druid_admin_datasource_active_count Active connection count.\n",
        "# TYPE druid_admin_datasource_active_count gauge\n",
        "# HELP druid_admin_datasource_pooling_count Pooled connection count.\n",
        "# TYPE druid_admin_datasource_pooling_count gauge\n",
        "# HELP druid_admin_datasource_execute_count Total execute count.\n",
        "# TYPE druid_admin_datasource_execute_count counter\n",
        "# HELP druid_admin_datasource_error_count Total error count.\n",
        "# TYPE druid_admin_datasource_error_count counter\n",
        "# HELP druid_admin_datasource_commit_count Total commit count.\n",
        "# TYPE druid_admin_datasource_commit_count counter\n",
        "# HELP druid_admin_datasource_rollback_count Total rollback count.\n",
        "# TYPE druid_admin_datasource_rollback_count counter\n",
    ));
    for entry in repository.all_datasources() {
        let datasource = &entry.datasource;
        let service_id =
            prometheus_label_value(datasource.service_id.as_deref().unwrap_or("unknown"));
        let service_name =
            prometheus_label_value(datasource.service_name.as_deref().unwrap_or("unknown"));
        let instance_id =
            prometheus_label_value(datasource.instance_id.as_deref().unwrap_or("unknown"));
        let boot_id = prometheus_label_value(datasource.boot_id.as_deref().unwrap_or("unknown"));
        let datasource_name =
            prometheus_label_value(datasource.name.as_deref().unwrap_or("unknown"));
        let db_type = prometheus_label_value(datasource.db_type.as_deref().unwrap_or("unknown"));
        let identity = prometheus_label_value(&datasource.identity.to_string());
        let labels = format!(
            "service_id=\"{service_id}\",service_name=\"{service_name}\",instance_id=\"{instance_id}\",boot_id=\"{boot_id}\",datasource=\"{identity}\",datasource_name=\"{datasource_name}\",db_type=\"{db_type}\""
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_active_count{{{labels}}} {}",
            datasource.active_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_pooling_count{{{labels}}} {}",
            datasource.pooling_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_execute_count{{{labels}}} {}",
            datasource.execute_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_error_count{{{labels}}} {}",
            datasource.error_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_commit_count{{{labels}}} {}",
            datasource.commit_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_rollback_count{{{labels}}} {}",
            datasource.rollback_count
        );
    }
}

fn prometheus_label_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn missing_parameter(name: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"ResultCode": -1, "Content": format!("missing parameter: {name}")})),
    )
        .into_response()
}

fn invalid_service_id() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"ResultCode": -1, "Content": "invalid canonical serviceId"})),
    )
        .into_response()
}

fn error_response(error: MonitorStatServiceError) -> Response {
    // 只向客户端返回错误类别和静态参数名。上游 URL、参数值与发现信息
    // 可能含凭据，不得通过 JSON Content 或错误字符串回显。
    let (status, error_msg) = match error {
        MonitorStatServiceError::UnknownServiceId(_) => {
            (StatusCode::NOT_FOUND, "unknown serviceId".to_owned())
        }
        MonitorStatServiceError::InvalidParameter { name, .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("invalid parameter {name}"),
        ),
        _ => (
            StatusCode::BAD_GATEWAY,
            "upstream admin service failed".to_owned(),
        ),
    };
    (
        status,
        Json::<Value>(json!({"ResultCode": -1, "Content": error_msg})),
    )
        .into_response()
}

fn redirect_found(location: String) -> Response {
    (StatusCode::FOUND, [(header::LOCATION, location)]).into_response()
}

fn is_public_path(path: &str, context_path: &str) -> bool {
    let Some(relative_path) = path.strip_prefix(context_path) else {
        return false;
    };
    matches!(
        relative_path,
        "/login.html" | "/nopermit.html" | "/submitLogin"
    ) || relative_path.starts_with("/css/")
        || relative_path.starts_with("/js/")
}

fn is_command_path(path: &str, context_path: &str) -> bool {
    path.strip_prefix(context_path) == Some("/api/commands")
}

/// Check whether the request carries a valid (non-expired) session token.
///
/// While holding the write lock, expired sessions are pruned; admission also
/// enforces a hard cap independent of login rate.
fn request_has_session(request: &Request, sessions: &RwLock<HashMap<String, Instant>>) -> bool {
    let token = request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == "druid-session").then_some(value.to_owned())
            })
        });
    let Some(token) = token else {
        return false;
    };
    let now = Instant::now();
    let mut guard = sessions.write();
    // Prune expired sessions.
    guard.retain(|_, created| now.duration_since(*created) < SESSION_TTL);
    guard
        .get(&token)
        .is_some_and(|created| now.duration_since(*created) < SESSION_TTL)
}

fn static_resource_response(resource: &str) -> Response {
    let Some(body) = static_resource(resource) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = if resource.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if resource.ends_with(".css") {
        "text/css;charset=utf-8"
    } else {
        "text/javascript;charset=utf-8"
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "max-age=36000"),
        ],
        body,
    )
        .into_response()
}

fn static_resource(resource: &str) -> Option<&'static str> {
    Some(match resource {
        "activeConnectionStackTrace.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/activeConnectionStackTrace.html"
        )),
        "api.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/api.html"
        )),
        "connectionInfo.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/connectionInfo.html"
        )),
        "datasource.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/datasource.html"
        )),
        "header.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/header.html"
        )),
        "index.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/index.html"
        )),
        "login.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/login.html"
        )),
        "nopermit.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/nopermit.html"
        )),
        "spring-detail.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/spring-detail.html"
        )),
        "spring.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/spring.html"
        )),
        "sql-detail.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/sql-detail.html"
        )),
        "sql.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/sql.html"
        )),
        "wall.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/wall.html"
        )),
        "webapp.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/webapp.html"
        )),
        "websession-detail.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/websession-detail.html"
        )),
        "websession.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/websession.html"
        )),
        "weburi-detail.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/weburi-detail.html"
        )),
        "weburi.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/weburi.html"
        )),
        "css/bootstrap.min.css" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/css/bootstrap.min.css"
        )),
        "css/style.css" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/css/style.css"
        )),
        "js/bootstrap.min.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/bootstrap.min.js"
        )),
        "js/common.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/common.js"
        )),
        "js/doT.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/doT.js"
        )),
        "js/jquery.min.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/jquery.min.js"
        )),
        "js/lang.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/lang.js"
        )),
        _ => return None,
    })
}

#[cfg(test)]
mod command_error_security_tests {
    use axum::body::to_bytes;

    use super::{command_broker_error_response, StatusCode};

    #[tokio::test]
    async fn broker_diagnostics_are_not_reflected_to_command_clients() {
        let secret = "postgres://operator:very-secret@example.test/db";
        for (diagnostic, expected_status) in [
            (
                format!("command ID has a conflicting canonical payload: {secret}"),
                StatusCode::CONFLICT,
            ),
            (
                format!("command broker full: {secret}"),
                StatusCode::TOO_MANY_REQUESTS,
            ),
            (
                format!("command result deadline exceeded: {secret}"),
                StatusCode::GATEWAY_TIMEOUT,
            ),
            (
                format!("no live stream matches the requested identity: {secret}"),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                format!("admin command broker is shutting down: {secret}"),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                format!("unexpected broker failure: {secret}"),
                StatusCode::BAD_REQUEST,
            ),
        ] {
            let response = command_broker_error_response(&diagnostic);
            assert_eq!(response.status(), expected_status);
            let body = to_bytes(response.into_body(), 4096)
                .await
                .expect("response body");
            let body = std::str::from_utf8(&body).expect("JSON response");
            assert!(!body.contains(secret));
            assert!(!body.contains("operator"));
        }
    }
}
