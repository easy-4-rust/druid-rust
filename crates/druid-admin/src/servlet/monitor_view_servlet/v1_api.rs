//! 版本化管理 API。
//!
//! Java 兼容的 `.json` 路由混合了三种字段命名约定与一套信封错误语义，
//! 分离前端不应为此逐个特判。`/druid/api/v1` 提供一套一致的契约：
//! 字段一律 camelCase，错误一律用 HTTP 状态码表达。
//!
//! 数据来源与 Java 兼容路由完全一致，两条路由不会给出互相矛盾的结论。

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{Value, json};

use crate::service::MonitorStatService;

use super::MonitorBackend;

/// 管理 API 的版本。
pub const API_VERSION: &str = "v1";

/// 构造一个 camelCase 的数据源摘要。
fn datasource_summary(entry: &crate::repository::DataSourceEntry) -> Value {
    let source = &entry.datasource;
    json!({
        "serviceId": source.service_id,
        "serviceName": source.service_name,
        "identity": source.identity,
        "name": source.name,
        "dbType": source.db_type,
        "driverClassName": source.driver_class_name,
        "url": source.url,
        "userName": source.user_name,
        "activeCount": source.active_count,
        "poolingCount": source.pooling_count,
        "maxActive": source.max_active,
        "waitThreadCount": source.wait_thread_count,
        "executeCount": source.execute_count,
        "errorCount": source.error_count,
        "commitCount": source.commit_count,
        "rollbackCount": source.rollback_count,
        "sqlCount": entry.sql_stats.len(),
        "wallCheckCount": entry.wall.content.check_count,
        "wallViolationCount": entry.wall.content.violation_count,
        "lastUpdatedMs": entry.last_updated_ms,
    })
}


/// 把 Java 形状的 SQL 统计映射为版本化 API 的 camelCase 形状。
///
/// Java 兼容路由必须保持 PascalCase 字段名，因此这里做一次显式映射，
/// 而不是复用 DTO 的序列化结果。
fn sql_row(stat: &crate::model::dto::SqlListContent) -> Value {
    let mut row = serde_json::Map::new();
    row.insert("id".to_owned(), serde_json::to_value(&stat.id).unwrap_or(Value::Null));
    row.insert("hash".to_owned(), serde_json::to_value(&stat.hash).unwrap_or(Value::Null));
    row.insert("sql".to_owned(), serde_json::to_value(&stat.sql).unwrap_or(Value::Null));
    row.insert("executeCount".to_owned(), serde_json::to_value(&stat.execute_count).unwrap_or(Value::Null));
    row.insert("errorCount".to_owned(), serde_json::to_value(&stat.error_count).unwrap_or(Value::Null));
    row.insert("totalTime".to_owned(), serde_json::to_value(&stat.total_time).unwrap_or(Value::Null));
    row.insert("maxTimespan".to_owned(), serde_json::to_value(&stat.max_timespan).unwrap_or(Value::Null));
    row.insert("maxTimespanOccurTime".to_owned(), serde_json::to_value(&stat.max_timespan_occur_time).unwrap_or(Value::Null));
    row.insert("lastTime".to_owned(), serde_json::to_value(&stat.last_time).unwrap_or(Value::Null));
    row.insert("effectedRowCount".to_owned(), serde_json::to_value(&stat.effected_row_count).unwrap_or(Value::Null));
    row.insert("effectedRowCountMax".to_owned(), serde_json::to_value(&stat.effected_row_count_max).unwrap_or(Value::Null));
    row.insert("fetchRowCount".to_owned(), serde_json::to_value(&stat.fetch_row_count).unwrap_or(Value::Null));
    row.insert("fetchRowCountMax".to_owned(), serde_json::to_value(&stat.fetch_row_count_max).unwrap_or(Value::Null));
    row.insert("batchSizeMax".to_owned(), serde_json::to_value(&stat.batch_size_max).unwrap_or(Value::Null));
    row.insert("batchSizeTotal".to_owned(), serde_json::to_value(&stat.batch_size_total).unwrap_or(Value::Null));
    row.insert("concurrentMax".to_owned(), serde_json::to_value(&stat.concurrent_max).unwrap_or(Value::Null));
    row.insert("runningCount".to_owned(), serde_json::to_value(&stat.running_count).unwrap_or(Value::Null));
    row.insert("inTransactionCount".to_owned(), serde_json::to_value(&stat.in_transaction_count).unwrap_or(Value::Null));
    row.insert("resultSetHoldTime".to_owned(), serde_json::to_value(&stat.result_set_hold_time).unwrap_or(Value::Null));
    row.insert("executeAndResultSetHoldTime".to_owned(), serde_json::to_value(&stat.execute_and_result_set_hold_time).unwrap_or(Value::Null));
    row.insert("dbType".to_owned(), serde_json::to_value(&stat.db_type).unwrap_or(Value::Null));
    row.insert("dataSource".to_owned(), serde_json::to_value(&stat.data_source).unwrap_or(Value::Null));
    row.insert("url".to_owned(), serde_json::to_value(&stat.url).unwrap_or(Value::Null));
    row.insert("name".to_owned(), serde_json::to_value(&stat.name).unwrap_or(Value::Null));
    row.insert("file".to_owned(), serde_json::to_value(&stat.file).unwrap_or(Value::Null));
    row.insert("lastError".to_owned(), serde_json::to_value(&stat.last_error).unwrap_or(Value::Null));
    row.insert("lastErrorMessage".to_owned(), serde_json::to_value(&stat.last_error_message).unwrap_or(Value::Null));
    row.insert("lastErrorClass".to_owned(), serde_json::to_value(&stat.last_error_class).unwrap_or(Value::Null));
    row.insert("lastErrorStackTrace".to_owned(), serde_json::to_value(&stat.last_error_stack_trace).unwrap_or(Value::Null));
    row.insert("lastErrorTime".to_owned(), serde_json::to_value(&stat.last_error_time).unwrap_or(Value::Null));
    row.insert("lastSlowParameters".to_owned(), serde_json::to_value(&stat.last_slow_parameters).unwrap_or(Value::Null));
    row.insert("clobOpenCount".to_owned(), serde_json::to_value(&stat.clob_open_count).unwrap_or(Value::Null));
    row.insert("blobOpenCount".to_owned(), serde_json::to_value(&stat.blob_open_count).unwrap_or(Value::Null));
    row.insert("readStringLength".to_owned(), serde_json::to_value(&stat.read_string_length).unwrap_or(Value::Null));
    row.insert("readBytesLength".to_owned(), serde_json::to_value(&stat.read_bytes_length).unwrap_or(Value::Null));
    row.insert("inputStreamOpenCount".to_owned(), serde_json::to_value(&stat.input_stream_open_count).unwrap_or(Value::Null));
    row.insert("readerOpenCount".to_owned(), serde_json::to_value(&stat.reader_open_count).unwrap_or(Value::Null));
    row.insert("histogram".to_owned(), serde_json::to_value(&stat.histogram).unwrap_or(Value::Null));
    row.insert("fetchRowCountHistogram".to_owned(), serde_json::to_value(&stat.fetch_row_count_histogram).unwrap_or(Value::Null));
    row.insert("effectedRowCountHistogram".to_owned(), serde_json::to_value(&stat.effected_row_count_histogram).unwrap_or(Value::Null));
    row.insert("executeAndResultHoldTimeHistogram".to_owned(), serde_json::to_value(&stat.execute_and_result_hold_time_histogram).unwrap_or(Value::Null));
    row.insert("serviceId".to_owned(), serde_json::to_value(&stat.service_id).unwrap_or(Value::Null));
    row.insert("serviceName".to_owned(), serde_json::to_value(&stat.service_name).unwrap_or(Value::Null));
    row.insert("address".to_owned(), serde_json::to_value(&stat.address).unwrap_or(Value::Null));
    row.insert("port".to_owned(), serde_json::to_value(&stat.port).unwrap_or(Value::Null));
    Value::Object(row)
}
/// `GET /druid/api/v1/basic` —— Admin 自身的运行信息。
pub(crate) async fn basic(State(state): State<super::MonitorViewServlet>) -> Response {
    let mut drivers: Vec<String> = Vec::new();
    let mut datasource_count = 0usize;
    let mut sql_count = 0usize;
    if let MonitorBackend::Repository(repository) = &state.backend {
        for entry in repository.all_datasources() {
            datasource_count += 1;
            sql_count += entry.sql_stats.len();
            if let Some(db_type) = entry.datasource.db_type {
                drivers.push(db_type);
            }
        }
        drivers.sort();
        drivers.dedup();
    }
    Json(json!({
        "apiVersion": API_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "drivers": drivers,
        "datasourceCount": datasource_count,
        "sqlCount": sql_count,
        "backend": match &state.backend {
            MonitorBackend::Remote(_) => "poll",
            MonitorBackend::Repository(_) => "push",
        },
    }))
    .into_response()
}

/// `GET /druid/api/v1/datasources` —— 数据源列表。
pub(crate) async fn datasources(State(state): State<super::MonitorViewServlet>) -> Response {
    match &state.backend {
        MonitorBackend::Remote(service) => match service.get_data_source_stat_data().await {
            Ok(result) => Json(json!({
                "apiVersion": API_VERSION,
                "datasources": result.content.unwrap_or_default(),
            }))
            .into_response(),
            Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        },
        MonitorBackend::Repository(repository) => Json(json!({
            "apiVersion": API_VERSION,
            "datasources": repository
                .all_datasources()
                .iter()
                .map(datasource_summary)
                .collect::<Vec<Value>>(),
        }))
        .into_response(),
    }
}

/// `GET /druid/api/v1/datasources/{identity}` —— 单个数据源。
pub(crate) async fn datasource(
    State(state): State<super::MonitorViewServlet>,
    Path(identity): Path<i64>,
) -> Response {
    match &state.backend {
        MonitorBackend::Remote(service) => match service.get_data_source_stat_data().await {
            Ok(result) => {
                let matched = result
                    .content
                    .unwrap_or_default()
                    .into_iter()
                    .find(|content| content.identity == identity);
                match matched {
                    Some(content) => Json(json!({"apiVersion": API_VERSION, "datasource": content}))
                        .into_response(),
                    None => (StatusCode::NOT_FOUND, "no datasource matches the identity").into_response(),
                }
            }
            Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        },
        MonitorBackend::Repository(repository) => {
            let matched = repository
                .all_datasources()
                .into_iter()
                .find(|entry| entry.datasource.identity == identity);
            match matched {
                Some(entry) => Json(json!({"apiVersion": API_VERSION, "datasource": datasource_summary(&entry)}))
                    .into_response(),
                None => (StatusCode::NOT_FOUND, "no datasource matches the identity").into_response(),
            }
        }
    }
}

/// `GET /druid/api/v1/sql` —— SQL 统计，支持排序与分页。
pub(crate) async fn sql_list(
    State(state): State<super::MonitorViewServlet>,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let order_by = parameters
        .get("orderBy")
        .cloned()
        .unwrap_or_else(|| "executeCount".to_owned());
    let descending = parameters.get("orderType").is_some_and(|value| value == "desc");
    let page = parameters
        .get("page")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let per_page = parameters
        .get("perPageCount")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(50)
        .max(1);

    let mut rows: Vec<Value> = match &state.backend {
        MonitorBackend::Remote(service) => {
            let query = crate::service::StatQuery {
                service_name: parameters.get("serviceName").cloned(),
                order_by: order_by.clone(),
                order_type: if descending { "desc".to_owned() } else { "asc".to_owned() },
                page,
                per_page_count: per_page,
            };
            match service.get_sql_stat_data_list(&query).await {
                Ok(result) => result
                    .get("Content")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                Err(error) => return (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
            }
        }
        MonitorBackend::Repository(repository) => repository
            .all_sql_stats()
            .into_iter()
            .map(|stat| sql_row(&stat))
            .collect(),
    };

    if matches!(state.backend, MonitorBackend::Repository(_)) {
        rows.sort_by(|left, right| {
            let ordering = compare_values(left, right, &order_by);
            if descending { ordering.reverse() } else { ordering }
        });
        let total = rows.len();
        let from = (page - 1) * per_page;
        rows = if from >= total {
            Vec::new()
        } else {
            rows.into_iter().skip(from).take(per_page).collect()
        };
        let mut body = json!({
            "apiVersion": API_VERSION,
            "total": total,
            "page": page,
            "perPageCount": per_page,
            "rows": rows,
        });
        body["total"] = json!(total);
        return Json(body).into_response();
    }

    Json(json!({
        "apiVersion": API_VERSION,
        "rows": rows,
    }))
    .into_response()
}

/// 比较两个 JSON 对象的某个字段。
fn compare_values(left: &Value, right: &Value, key: &str) -> std::cmp::Ordering {
    let left_value = left.get(key);
    let right_value = right.get(key);
    match (left_value, right_value) {
        (Some(Value::Number(left_number)), Some(Value::Number(right_number))) => left_number
            .as_f64()
            .unwrap_or_default()
            .partial_cmp(&right_number.as_f64().unwrap_or_default())
            .unwrap_or(std::cmp::Ordering::Equal),
        (Some(Value::String(left_text)), Some(Value::String(right_text))) => {
            left_text.cmp(right_text)
        }
        _ => std::cmp::Ordering::Equal,
    }
}

/// `GET /druid/api/v1/wall` —— SQL 防火墙统计。
pub(crate) async fn wall(State(state): State<super::MonitorViewServlet>) -> Response {
    match &state.backend {
        MonitorBackend::Remote(service) => match service.get_wall_stat_map(&Default::default()).await {
            Ok(result) => Json(json!({"apiVersion": API_VERSION, "wall": result.content}))
                .into_response(),
            Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        },
        MonitorBackend::Repository(repository) => {
            let mut wall = repository.aggregated_wall();
            wall.result_code = MonitorStatService::RESULT_CODE_SUCCESS;
            Json(json!({"apiVersion": API_VERSION, "wall": wall.content})).into_response()
        }
    }
}

/// `GET /druid/api/v1/services` —— 已接入的服务。
pub(crate) async fn services(State(state): State<super::MonitorViewServlet>) -> Response {
    match &state.backend {
        MonitorBackend::Remote(service) => Json(json!({
            "apiVersion": API_VERSION,
            "services": service.applications(),
        }))
        .into_response(),
        MonitorBackend::Repository(repository) => {
            let mut names: Vec<String> = repository
                .all_datasources()
                .into_iter()
                .filter_map(|entry| entry.datasource.service_name.map(|name| name.to_string()))
                .collect();
            names.sort();
            names.dedup();
            Json(json!({"apiVersion": API_VERSION, "services": names})).into_response()
        }
    }
}

/// `GET /druid/api/v1/openapi.json` —— 契约文档。
pub(crate) async fn openapi() -> Response {
    Json(json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Druid Admin management API",
            "version": API_VERSION,
        },
        "paths": {
            "/druid/api/v1/basic": {"get": {"summary": "Admin runtime information"}},
            "/druid/api/v1/datasources": {"get": {"summary": "Datasource list"}},
            "/druid/api/v1/datasources/{identity}": {"get": {"summary": "One datasource"}},
            "/druid/api/v1/sql": {"get": {"summary": "SQL statistics"}},
            "/druid/api/v1/wall": {"get": {"summary": "Wall statistics"}},
            "/druid/api/v1/services": {"get": {"summary": "Attached services"}},
            "/druid/api/v1/openapi.json": {"get": {"summary": "This document"}},
        },
    }))
    .into_response()
}
