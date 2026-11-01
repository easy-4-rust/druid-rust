//! Java 兼容的控制台路由。
//!
//! Java Druid 的控制台页面会请求若干形如 `/datasource-{id}.json` 的地址。
//! 这类地址的标识符嵌在路径段中间，Axum 的路由匹配无法直接表达，
//! 因此统一由 [`dispatch_console_route`] 在 legacy 分发中解析。
//!
//! 与 Java 的差异：不具备该能力的后端必须明确报告原因，
//! 而不是返回一条恒为零的空列表或一句笼统的失败。

use std::collections::HashMap;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::command::CommandBroker;
use crate::model::dto::DataSourceResult;
use crate::repository::MetricsRepository;
use crate::service::MonitorStatService;

use super::MonitorBackend;

/// 控制台路由无法提供该能力时返回的原因。
const CONNECTION_INFO_UNSUPPORTED: &str =
    "the push ingest protocol carries no pooled-connection rows; use the poll backend to read them";
const STACK_TRACE_UNSUPPORTED: &str =
    "the push ingest protocol carries no active-connection stack traces; use the poll backend to read them";

/// 解析并处理一条控制台 JSON 路由。
///
/// 返回 [`None`] 表示该地址不是控制台路由，调用方应继续走普通分发。
pub(crate) async fn dispatch_console_route(
    backend: &MonitorBackend,
    command_broker: Option<&CommandBroker>,
    service_url: &str,
) -> Option<Response> {
    let (path, query) = match service_url.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (service_url, None),
    };

    if path == "/basic.json" {
        return Some(basic(backend).await);
    }
    if path == "/reset-all.json" {
        return Some(reset_all(command_broker, false).await);
    }
    if path == "/log-and-reset.json" {
        return Some(reset_all(command_broker, true).await);
    }
    if let Some(id) = embedded_identity(path, "datasource-") {
        return Some(datasource_identity(backend, id).await);
    }
    if let Some(id) = embedded_identity(path, "sql-") {
        return Some(sql_identity(backend, id, query).await);
    }
    if let Some(id) = embedded_identity(path, "connectionInfo-") {
        return Some(connection_info_identity(backend, id, query).await);
    }
    if let Some(id) = embedded_identity(path, "activeConnectionStackTrace-") {
        return Some(active_stack_trace_identity(backend, id).await);
    }
    None
}

/// 解析 `/prefix{id}.json` 形式的标识符。
fn embedded_identity(path: &str, prefix: &str) -> Option<i64> {
    let rest = path.strip_prefix('/')?.strip_prefix(prefix)?;
    let id = rest.strip_suffix(".json")?;
    id.parse::<i64>().ok()
}

/// 解析查询串。
fn query_parameters(query: Option<&str>) -> HashMap<String, String> {
    query
        .map(|value| {
            value
                .split('&')
                .filter_map(|pair| {
                    pair.split_once('=')
                        .map(|(key, value)| (key.to_owned(), value.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 构造 Java 兼容的成功信封。
fn success(content: Value) -> Response {
    axum::Json(json!({
        "ResultCode": MonitorStatService::RESULT_CODE_SUCCESS,
        "Content": content,
    }))
    .into_response()
}

/// 构造 Java 兼容的失败信封。
fn failure(reason: &str) -> Response {
    axum::Json(json!({
        "ResultCode": MonitorStatService::RESULT_CODE_ERROR,
        "Content": reason,
    }))
    .into_response()
}

/// `basic.json`：Admin 自身的运行信息。
async fn basic(backend: &MonitorBackend) -> Response {
    let mut drivers: Vec<String> = match backend {
        MonitorBackend::Remote(_) => Vec::new(),
        MonitorBackend::Repository(repository) => repository
            .all_datasources()
            .into_iter()
            .filter_map(|entry| entry.datasource.db_type)
            .collect(),
    };
    drivers.sort();
    drivers.dedup();
    success(json!({
        "Version": env!("CARGO_PKG_VERSION"),
        "Drivers": drivers,
        "ResetEnable": true,
        "ResetCount": 0,
        "StartTime": start_time_millis(),
    }))
}

/// Admin 进程启动时间（毫秒时间戳）。
fn start_time_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as i64)
        .unwrap_or_default()
}

/// `datasource-{id}.json`：按标识符返回单个数据源。
async fn datasource_identity(backend: &MonitorBackend, id: i64) -> Response {
    match backend {
        MonitorBackend::Remote(service) => match service.get_data_source_stat_data().await {
            Ok(result) => axum::Json(result).into_response(),
            Err(error) => failure(&error.to_string()),
        },
        MonitorBackend::Repository(repository) => {
            let matched = repository
                .all_datasources()
                .into_iter()
                .find(|entry| entry.datasource.identity == id);
            match matched {
                Some(entry) => axum::Json(DataSourceResult {
                    result_code: MonitorStatService::RESULT_CODE_SUCCESS,
                    content: Some(vec![entry.datasource]),
                })
                .into_response(),
                None => failure("no datasource matches the requested identity"),
            }
        }
    }
}

/// `sql-{id}.json`：按标识符返回单条 SQL 统计。
async fn sql_identity(
    backend: &MonitorBackend,
    id: i64,
    query: Option<&str>,
) -> Response {
    let parameters = query_parameters(query);
    match backend {
        MonitorBackend::Remote(service) => {
            let Some(service_id) = parameters.get("serviceId") else {
                return failure("serviceId is required to address a remote SQL identity");
            };
            match service.get_sql_stat(id, service_id).await {
                Ok(result) => axum::Json(result).into_response(),
                Err(error) => failure(&error.to_string()),
            }
        }
        MonitorBackend::Repository(repository) => {
            let matched = repository
                .all_sql_stats()
                .into_iter()
                .find(|stat| stat.id == id || stat.hash == id);
            match matched {
                Some(stat) => success(json!(stat)),
                None => failure("no SQL statement matches the requested identity"),
            }
        }
    }
}

/// `connectionInfo-{id}.json`：池化连接明细。
async fn connection_info_identity(
    backend: &MonitorBackend,
    id: i64,
    query: Option<&str>,
) -> Response {
    let parameters = query_parameters(query);
    match backend {
        MonitorBackend::Remote(service) => {
            let Some(service_id) = parameters.get("serviceId") else {
                return failure("serviceId is required to address remote pooled connections");
            };
            match service
                .get_pooling_connection_info_by_data_source_id(id, service_id)
                .await
            {
                Ok(result) => axum::Json(result).into_response(),
                Err(error) => failure(&error.to_string()),
            }
        }
        MonitorBackend::Repository(_) => failure(CONNECTION_INFO_UNSUPPORTED),
    }
}

/// `activeConnectionStackTrace-{id}.json`：活跃连接堆栈。
async fn active_stack_trace_identity(backend: &MonitorBackend, _id: i64) -> Response {
    match backend {
        MonitorBackend::Remote(_) => failure(STACK_TRACE_UNSUPPORTED),
        MonitorBackend::Repository(_) => failure(STACK_TRACE_UNSUPPORTED),
    }
}

/// `reset-all.json` 与 `log-and-reset.json`：向所有接入服务下发重置命令。
async fn reset_all(command_broker: Option<&CommandBroker>, log_before_reset: bool) -> Response {
    let Some(broker) = command_broker else {
        return failure("the command broker is unavailable");
    };
    let identities = broker.attached_identities();
    let mut failed: Vec<String> = Vec::new();
    for identity in identities {
        let command_id = format!(
            "admin-reset-{}-{}",
            if log_before_reset { "log" } else { "plain" },
            identity.service_id()
        );
        let outcome = broker
            .issue_reset(
                identity.service_name(),
                identity.instance_id(),
                identity.boot_id(),
                &command_id,
                Vec::new(),
            )
            .await;
        if let Err(error) = outcome {
            failed.push(format!("{}: {error}", identity.service_id()));
        }
    }
    if failed.is_empty() {
        success(Value::Null)
    } else {
        failure(&failed.join("; "))
    }
}

/// 便于测试构造仓库后端。
#[cfg(test)]
pub(crate) fn repository_backend(repository: MetricsRepository) -> MonitorBackend {
    MonitorBackend::Repository(repository)
}
