//! Java 兼容的控制台路由。
//!
//! Java Druid 的控制台页面会请求若干形如 `/datasource-{id}.json` 的地址。
//! 这类地址的标识符嵌在路径段中间，Axum 的路由匹配无法直接表达，
//! 因此统一由 [`dispatch_console_route`] 在 legacy 分发中解析。
//!
//! 与 Java 的差异：不具备该能力的后端必须明确报告原因，
//! 而不是返回一条恒为零的空列表或一句笼统的失败。

use std::collections::HashMap;

use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use crate::command::CommandBroker;
use crate::model::dto::DataSourceResult;
use crate::service::{ManagementSource, MonitorStatService};

/// 控制台路由无法提供该能力时返回的原因。
const CONNECTION_INFO_UNSUPPORTED: &str =
    "the push ingest protocol carries no pooled-connection rows; use the poll backend to read them";
const STACK_TRACE_UNSUPPORTED: &str =
    "the push ingest protocol carries no active-connection stack traces; use the poll backend to read them";

/// 解析并处理一条控制台 JSON 路由。
///
/// 返回 [`None`] 表示该地址不是控制台路由，调用方应继续走普通分发。
pub(crate) async fn dispatch_console_route(
    backend: &ManagementSource,
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
        return Some(active_stack_trace_identity(backend, id, query).await);
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
async fn basic(backend: &ManagementSource) -> Response {
    let mut drivers: Vec<String> = backend
        .data_sources()
        .await
        .ok()
        .and_then(|result| result.content)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|content| content.db_type)
        .collect();
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
async fn datasource_identity(backend: &ManagementSource, id: i64) -> Response {
    match backend.data_source(id, None).await {
        Ok(Some(content)) => axum::Json(DataSourceResult {
            result_code: MonitorStatService::RESULT_CODE_SUCCESS,
            content: Some(vec![content]),
        })
        .into_response(),
        Ok(None) => failure("no datasource matches the requested identity"),
        Err(error) => failure(&error.to_string()),
    }
}

/// `sql-{id}.json`：按标识符返回单条 SQL 统计。
async fn sql_identity(backend: &ManagementSource, id: i64, query: Option<&str>) -> Response {
    let parameters = query_parameters(query);
    let service_id = parameters.get("serviceId").map(String::as_str);
    match backend.sql_stat(id, service_id, None).await {
        Ok(Some(stat)) => success(json!(stat)),
        Ok(None) => failure("no SQL statement matches the requested identity"),
        Err(error) => failure(&error.to_string()),
    }
}

/// `connectionInfo-{id}.json`：池化连接明细。
async fn connection_info_identity(
    backend: &ManagementSource,
    id: i64,
    query: Option<&str>,
) -> Response {
    let parameters = query_parameters(query);
    let service_id = parameters.get("serviceId").map(String::as_str);
    if service_id.is_none() {
        return failure("serviceId is required to address remote pooled connections");
    }
    match backend.connection_info(id, service_id).await {
        Ok(Some(result)) => axum::Json(result).into_response(),
        Ok(None) => failure(CONNECTION_INFO_UNSUPPORTED),
        Err(error) => failure(&error.to_string()),
    }
}

/// `activeConnectionStackTrace-{id}.json`：活跃连接堆栈。
///
/// 数据来自被监控节点的借出点堆栈捕获（需节点 `removeAbandoned=true`），
/// 由轮询后端转发；推送后端不带连接级堆栈，明确报告能力缺失。
async fn active_stack_trace_identity(
    backend: &ManagementSource,
    id: i64,
    query: Option<&str>,
) -> Response {
    let parameters = query_parameters(query);
    let service_id = parameters.get("serviceId").map(String::as_str);
    if service_id.is_none() {
        return failure("serviceId is required to address remote active-connection stack traces");
    }
    match backend.active_connection_stack_trace(id, service_id).await {
        Ok(Some(result)) if result.result_code == MonitorStatService::RESULT_CODE_SUCCESS => {
            success(result.content)
        }
        // 节点回答了但报告自身失败（典型：未开启 removeAbandoned），原样转述原因。
        Ok(Some(result)) => failure(&result.content.to_string()),
        Ok(None) => failure(STACK_TRACE_UNSUPPORTED),
        Err(error) => failure(&error.to_string()),
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
