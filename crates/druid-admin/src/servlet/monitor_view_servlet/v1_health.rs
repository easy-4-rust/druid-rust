//! 控制台采集健康与已有连接级读取能力的版本化入口。
use super::{v1_snapshot as snapshot, MonitorViewServlet};
use crate::service::ManagementSource;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

/// 从 repository 单调接收时间与真实计数生成健康快照。
pub(crate) async fn health(State(state): State<MonitorViewServlet>) -> Response {
    let source = state.backend.source();
    let repo = source.repository_view();
    let data = snapshot::load(source, false).await;
    let mut observations = repo.console_observations();
    observations.extend(data.rows.iter().filter(|r| r["source"] == "poll").map(|r| {
        json!({
            "serviceId": r["serviceId"], "serviceName": r["serviceName"], "identity": r["identity"],
            "name": r["name"], "source": "poll", "state": "reachable", "sequence": null,
            "samplingTimeMillis": null, "receivedAgeMillis": null
        })
    }));
    let retained = repo.retained_state_bytes();
    let (ingest, rejected) = repo.counters();
    let (capacity, ttl, output) = repo.limit_counters();
    // RSS 来自进程采样器；采样失败时保留空值，不能用 repository 投影字节替代。
    let process_rss_bytes =
        memory_stats::memory_stats().and_then(|stats| u64::try_from(stats.physical_mem).ok());
    let mut unavailable_fields = vec!["pendingAckCount", "queueDepth", "unflushedCount"];
    if process_rss_bytes.is_none() {
        unavailable_fields.push("processRssBytes");
    }
    data.response(json!({"apiVersion": "v1", "sources": observations, "processRssBytes": process_rss_bytes,
        "repository": {"ingestCount": ingest, "rejectedCount": rejected, "capacityRejectedCount": capacity,
        "ttlEvictedCount": ttl, "metricsOutputRejectedCount": output,
        "retainedStateBytes": retained, "maxTotalStateBytes": repo.max_total_state_bytes(),
        "entryTtlMillis": repo.entry_ttl().as_millis(), "maxServices": repo.max_services(),
        "maxSqlEntriesPerDatasource": repo.max_sql_entries_per_datasource(),
        "sampleHistory": repo.sample_history_status()},
        "unavailableFields": unavailable_fields}))
}

/// 查询最近成功接收的数值采样；来源身份和条数在服务端限制。
pub(crate) async fn history(
    State(state): State<MonitorViewServlet>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let datasource_id = match params.get("datasourceId") {
        Some(value) => match value.parse::<i64>() {
            Ok(id) => Some(id),
            Err(_) => return snapshot::error(StatusCode::UNPROCESSABLE_ENTITY, "数据源身份无效"),
        },
        None => None,
    };
    let limit = params
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100)
        .clamp(1, 500);
    let repo = state.backend.source().repository_view();
    Json(json!({"apiVersion": "v1", "source": "push", "storage": repo.sample_history_status(),
        "samples": repo.sample_history(params.get("serviceId").map(String::as_str), datasource_id, limit)})).into_response()
}

pub(super) async fn wall_response(
    source: &ManagementSource,
    params: &HashMap<String, String>,
) -> Response {
    let mut data = snapshot::Snapshot {
        rows: Vec::new(),
        failures: Vec::new(),
        successful: usize::from(!source.has_remote()),
    };
    if let Some(remote) = source.remote_service() {
        let response = remote.console_read("/druid/wall.json").await;
        data.failures = response["failures"].as_array().cloned().unwrap_or_default();
        for row in response["observations"].as_array().into_iter().flatten() {
            if !snapshot::matches(row, params) {
                continue;
            }
            data.successful += 1;
            if row["content"].is_object() {
                data.rows.push(row["content"].clone());
            }
        }
    }
    for (id, entry) in source.repository_view().console_entries() {
        let identity = json!({"serviceId": id.service_identity.shared_service_id(),
            "serviceName": id.service_identity.shared_service_name(), "identity": id.identity});
        if !snapshot::matches(&identity, params) {
            continue;
        }
        data.successful += 1;
        data.rows.push(json!({"checkCount": entry.wall.content.check_count,
            "violationCount": entry.wall.content.violation_count, "blackListHitCount": entry.wall.content.black_list_hit_count}));
    }
    let mut wall = json!({});
    let mut missing = Vec::new();
    let mut coverage = json!({});
    for key in [
        "checkCount",
        "hardCheckCount",
        "violationCount",
        "violationEffectRowCount",
        "syntaxErrorCount",
        "whiteListHitCount",
        "whiteListSize",
        "blackListHitCount",
        "blackListSize",
        "tables",
        "functions",
        "whiteList",
        "blackList",
    ] {
        let reported: Vec<&Value> = data
            .rows
            .iter()
            .filter_map(|r| r.get(key).filter(|v| !v.is_null()))
            .collect();
        coverage[key] = json!({"reported": reported.len(), "total": data.rows.len()});
        if reported.is_empty() {
            wall[key] = Value::Null;
            missing.push(key);
            continue;
        }
        if ["tables", "functions", "whiteList", "blackList"].contains(&key) {
            wall[key] = json!(reported
                .into_iter()
                .filter_map(Value::as_array)
                .flatten()
                .cloned()
                .collect::<Vec<_>>());
        } else {
            wall[key] = json!(reported
                .iter()
                .filter_map(|v| v.as_i64())
                .fold(0_i64, i64::saturating_add));
        }
    }
    wall["unavailableFields"] = json!(missing);
    data.response(json!({"apiVersion": "v1", "wall": wall, "coverage": coverage}))
}

pub(crate) async fn connections(
    State(state): State<MonitorViewServlet>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    connection_response(&state, id, &params, false).await
}
pub(crate) async fn stack(
    State(state): State<MonitorViewServlet>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    connection_response(&state, id, &params, true).await
}
async fn connection_response(
    state: &MonitorViewServlet,
    id: i64,
    params: &HashMap<String, String>,
    stack: bool,
) -> Response {
    let source = state.backend.source();
    let data = snapshot::load(source, false).await;
    let matches: Vec<&Value> = data
        .rows
        .iter()
        .filter(|r| r["identity"] == id && snapshot::matches(r, params))
        .collect();
    let row = match matches.as_slice() {
        [row] => *row,
        [] if data.successful == 0 && !data.failures.is_empty() => return data.response(json!({})),
        [] => return snapshot::error(StatusCode::NOT_FOUND, "未找到数据源"),
        _ => return snapshot::error(StatusCode::CONFLICT, "请指定数据源所属服务"),
    };
    if row["source"] != "poll" {
        return Json(json!({"apiVersion": "v1", "supported": false, "reason": "当前推送协议不支持连接列表及借出堆栈", "rows": null})).into_response();
    }
    let service_id = row["serviceId"].as_str();
    let result = if stack {
        source
            .active_connection_stack_trace(id, service_id)
            .await
            .map(|r| r.map(|r| (r.result_code, r.content)))
    } else {
        source
            .connection_info(id, service_id)
            .await
            .map(|r| r.map(|r| (r.result_code, json!(r.content))))
    };
    match result {
        Ok(Some((code, rows))) if code >= 0 => Json(json!({"apiVersion": "v1", "supported": true, "rows": rows})).into_response(),
        Ok(Some(_)) => Json(json!({"apiVersion": "v1", "supported": false, "reason": "节点未开放此能力；活跃堆栈需要启用 removeAbandoned", "rows": null})).into_response(),
        _ => snapshot::error(StatusCode::BAD_GATEWAY, "连接信息读取失败"),
    }
}
