//! 版本化管理 API。
//!
//! Java 兼容的 `.json` 路由混合了三种字段命名约定与一套信封错误语义，
//! 分离前端不应为此逐个特判。`/druid/api/v1` 提供一套一致的契约：
//! 字段一律 camelCase，错误一律用 HTTP 状态码表达。
//!
//! 同时读取远端与推送仓库，逐来源保留字段存在性和失败信息。
//! Java 兼容路由继续保留原先远端优先的读取语义。

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use super::v1_snapshot as snapshot;
use crate::model::dto::{DataSourceContent, SqlListContent};

/// 管理 API 的版本。
pub const API_VERSION: &str = "v1";

/// 序列化任意值为 JSON。
fn v<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// 把 Java 形状的 SQL 统计映射为版本化 API 的 camelCase 形状。
///
/// Java 兼容路由必须保持 PascalCase 字段名，因此这里做一次显式映射。
/// 用 `serde_json::Map` 逐项插入（`json!` 宏超过约四十个字段会爆递归上限）。
pub(super) fn sql_row(stat: &SqlListContent) -> Value {
    let mut row = serde_json::Map::new();
    let pairs: [(&str, Value); 45] = [
        ("id", v(&stat.id)),
        ("hash", v(&stat.hash)),
        ("sql", v(&stat.sql)),
        ("executeCount", v(&stat.execute_count)),
        ("errorCount", v(&stat.error_count)),
        ("totalTime", v(&stat.total_time)),
        ("maxTimespan", v(&stat.max_timespan)),
        ("maxTimespanOccurTime", v(&stat.max_timespan_occur_time)),
        ("lastTime", v(&stat.last_time)),
        ("effectedRowCount", v(&stat.effected_row_count)),
        ("effectedRowCountMax", v(&stat.effected_row_count_max)),
        ("fetchRowCount", v(&stat.fetch_row_count)),
        ("fetchRowCountMax", v(&stat.fetch_row_count_max)),
        ("batchSizeMax", v(&stat.batch_size_max)),
        ("batchSizeTotal", v(&stat.batch_size_total)),
        ("concurrentMax", v(&stat.concurrent_max)),
        ("runningCount", v(&stat.running_count)),
        ("inTransactionCount", v(&stat.in_transaction_count)),
        ("resultSetHoldTime", v(&stat.result_set_hold_time)),
        (
            "executeAndResultSetHoldTime",
            v(&stat.execute_and_result_set_hold_time),
        ),
        ("dbType", v(&stat.db_type)),
        ("dataSource", v(&stat.data_source)),
        ("url", v(&stat.url)),
        ("name", v(&stat.name)),
        ("file", v(&stat.file)),
        ("lastError", v(&stat.last_error)),
        ("lastErrorMessage", v(&stat.last_error_message)),
        ("lastErrorClass", v(&stat.last_error_class)),
        ("lastErrorStackTrace", v(&stat.last_error_stack_trace)),
        ("lastErrorTime", v(&stat.last_error_time)),
        ("lastSlowParameters", v(&stat.last_slow_parameters)),
        ("clobOpenCount", v(&stat.clob_open_count)),
        ("blobOpenCount", v(&stat.blob_open_count)),
        ("readStringLength", v(&stat.read_string_length)),
        ("readBytesLength", v(&stat.read_bytes_length)),
        ("inputStreamOpenCount", v(&stat.input_stream_open_count)),
        ("readerOpenCount", v(&stat.reader_open_count)),
        ("histogram", v(&stat.histogram)),
        ("fetchRowCountHistogram", v(&stat.fetch_row_count_histogram)),
        (
            "effectedRowCountHistogram",
            v(&stat.effected_row_count_histogram),
        ),
        (
            "executeAndResultHoldTimeHistogram",
            v(&stat.execute_and_result_hold_time_histogram),
        ),
        ("serviceId", v(&stat.service_id)),
        ("serviceName", v(&stat.service_name)),
        ("address", v(&stat.address)),
        ("port", v(&stat.port)),
    ];
    for (name, value) in pairs {
        row.insert(name.to_owned(), value);
    }
    Value::Object(row)
}

/// 把 Java 形状的数据源完整配置映射为 camelCase 详情。
///
/// Java 的 datasource 页展示全部 ~74 个字段；版本化 API 的详情路由
/// 原样保留这些配置，控制台据此渲染完整面板。
pub(super) fn datasource_detail(source: &DataSourceContent) -> Value {
    let mut row = serde_json::Map::new();
    let pairs: Vec<(&str, Value)> = vec![
        ("serviceId", v(&source.service_id)),
        ("serviceName", v(&source.service_name)),
        ("instanceId", v(&source.instance_id)),
        ("bootId", v(&source.boot_id)),
        ("identity", v(&source.identity)),
        ("name", v(&source.name)),
        ("dbType", v(&source.db_type)),
        ("driverClassName", v(&source.driver_class_name)),
        ("url", v(&source.url)),
        ("userName", v(&source.user_name)),
        ("waitThreadCount", v(&source.wait_thread_count)),
        ("notEmptyWaitCount", v(&source.not_empty_wait_count)),
        ("notEmptyWaitMillis", v(&source.not_empty_wait_millis)),
        ("poolingCount", v(&source.pooling_count)),
        ("poolingPeak", v(&source.pooling_peak)),
        ("poolingPeakTime", v(&source.pooling_peak_time)),
        ("activeCount", v(&source.active_count)),
        ("activePeak", v(&source.active_peak)),
        ("activePeakTime", v(&source.active_peak_time)),
        ("initialSize", v(&source.initial_size)),
        ("minIdle", v(&source.min_idle)),
        ("maxActive", v(&source.max_active)),
        ("queryTimeout", v(&source.query_timeout)),
        (
            "transactionQueryTimeout",
            v(&source.transaction_query_timeout),
        ),
        ("loginTimeout", v(&source.login_timeout)),
        (
            "validConnectionCheckerClassName",
            v(&source.valid_connection_checker_class_name),
        ),
        (
            "exceptionSorterClassName",
            v(&source.exception_sorter_class_name),
        ),
        ("testOnBorrow", v(&source.test_on_borrow)),
        ("testOnReturn", v(&source.test_on_return)),
        ("testWhileIdle", v(&source.test_while_idle)),
        ("defaultAutoCommit", v(&source.default_auto_commit)),
        ("defaultReadOnly", v(&source.default_read_only)),
        (
            "defaultTransactionIsolation",
            v(&source.default_transaction_isolation),
        ),
        ("logicConnectCount", v(&source.logic_connect_count)),
        ("logicCloseCount", v(&source.logic_close_count)),
        (
            "logicConnectErrorCount",
            v(&source.logic_connect_error_count),
        ),
        ("driverConnectCount", v(&source.driver_connect_count)),
        ("driverCloseCount", v(&source.driver_close_count)),
        (
            "driverConnectErrorCount",
            v(&source.driver_connect_error_count),
        ),
        ("executeCount", v(&source.execute_count)),
        ("executeUpdateCount", v(&source.execute_update_count)),
        ("executeQueryCount", v(&source.execute_query_count)),
        ("executeBatchCount", v(&source.execute_batch_count)),
        ("errorCount", v(&source.error_count)),
        ("commitCount", v(&source.commit_count)),
        ("rollbackCount", v(&source.rollback_count)),
        ("psCacheAccessCount", v(&source.ps_cache_access_count)),
        ("psCacheHitCount", v(&source.ps_cache_hit_count)),
        ("psCacheMissCount", v(&source.ps_cache_miss_count)),
        ("startTransactionCount", v(&source.start_transaction_count)),
        ("removeAbandoned", v(&source.remove_abandoned)),
        ("clobOpenCount", v(&source.clob_open_count)),
        ("blobOpenCount", v(&source.blob_open_count)),
        ("keepAliveCheckCount", v(&source.keep_alive_check_count)),
        ("keepAlive", v(&source.keep_alive)),
        ("failFast", v(&source.fail_fast)),
        ("maxWait", v(&source.max_wait)),
        ("maxWaitThreadCount", v(&source.max_wait_thread_count)),
        (
            "poolPreparedStatements",
            v(&source.pool_prepared_statements),
        ),
        (
            "maxPoolPreparedStatementPerConnectionSize",
            v(&source.max_pool_prepared_statement_per_connection_size),
        ),
        (
            "minEvictableIdleTimeMillis",
            v(&source.min_evictable_idle_time_millis),
        ),
        (
            "maxEvictableIdleTimeMillis",
            v(&source.max_evictable_idle_time_millis),
        ),
        ("logDifferentThread", v(&source.log_different_thread)),
        ("recycleErrorCount", v(&source.recycle_error_count)),
        (
            "preparedStatementOpenCount",
            v(&source.prepared_statement_open_count),
        ),
        (
            "preparedStatementClosedCount",
            v(&source.prepared_statement_closed_count),
        ),
        ("useUnfairLock", v(&source.use_unfair_lock)),
        ("initGlobalVariants", v(&source.init_global_variants)),
        ("initVariants", v(&source.init_variants)),
        ("filterClassNames", v(&source.filter_class_names)),
        ("transactionHistogram", v(&source.transaction_histogram)),
        (
            "connectionHoldTimeHistogram",
            v(&source.connection_hold_time_histogram),
        ),
    ];
    for (name, value) in pairs {
        row.insert(name.to_owned(), value);
    }
    Value::Object(row)
}

/// 返回真实版本、来源与本次读取的数据总量。
pub(crate) async fn basic(State(state): State<super::MonitorViewServlet>) -> Response {
    let data = snapshot::load(state.backend.source(), false).await;
    let sql = snapshot::load(state.backend.source(), true).await;
    let mut drivers: Vec<String> = data
        .rows
        .iter()
        .filter_map(|row| row["dbType"].as_str().map(str::to_owned))
        .collect();
    drivers.sort();
    drivers.dedup();
    let backend = if state.backend.source().has_remote() {
        if state.backend.source().repository_view().is_empty() {
            "poll"
        } else {
            "mixed"
        }
    } else {
        "push"
    };
    let body = json!({"apiVersion": API_VERSION, "version": env!("CARGO_PKG_VERSION"),
        "datasourceCount": data.rows.len(), "sqlCount": sql.rows.len(), "drivers": drivers, "backend": backend, "sqlTextVisible": !state.redaction_policy.is_enabled()});
    let combined = snapshot::Snapshot {
        rows: Vec::new(),
        failures: data.failures.into_iter().chain(sql.failures).collect(),
        successful: data.successful + sql.successful,
    };
    combined.response(body)
}

/// 列表携带来源及字段存在性；旧 camelCase 数值字段保持兼容。
pub(crate) async fn datasources(
    State(state): State<super::MonitorViewServlet>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let mut data = snapshot::load(state.backend.source(), false).await;
    data.rows.retain(|row| snapshot::matches(row, &params));
    data.response(json!({"apiVersion": API_VERSION, "datasources": data.rows}))
}

/// 带 serviceId 精确定位；裸数值身份不唯一时拒绝选取首条。
pub(crate) async fn datasource(
    State(state): State<super::MonitorViewServlet>,
    Path(identity): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let data = snapshot::load(state.backend.source(), false).await;
    let rows: Vec<&Value> = data
        .rows
        .iter()
        .filter(|row| row["identity"] == identity && snapshot::matches(row, &params))
        .collect();
    match rows.as_slice() {
        [row] => data.response(json!({"apiVersion": API_VERSION, "datasource": row})),
        [] if data.successful == 0 && !data.failures.is_empty() => data.response(json!({})),
        [] => snapshot::error(StatusCode::NOT_FOUND, "未找到数据源"),
        _ => snapshot::error(StatusCode::CONFLICT, "数据源身份不唯一，请指定来源"),
    }
}

/// SQL 先筛选再稳定排序与有界分页；缺失指标始终排在最后。
pub(crate) async fn sql_list(
    State(state): State<super::MonitorViewServlet>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let mut data = snapshot::load(state.backend.source(), true).await;
    let order = params.get("orderBy").map_or("executeCount", String::as_str);
    let descending = params.get("orderType").is_some_and(|v| v == "desc");
    let page = params
        .get("page")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let size = params
        .get("perPageCount")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50)
        .clamp(1, 500);
    let min_max = params
        .get("minMaxTimespan")
        .and_then(|v| v.parse::<f64>().ok());
    let search = params.get("search").map(|v| v.to_lowercase());
    data.rows.retain(|row| {
        snapshot::matches(row, &params)
            && min_max.is_none_or(|min| {
                !snapshot::unavailable(row, "maxTimespan")
                    && row["maxTimespan"].as_f64().is_some_and(|n| n >= min)
            })
            && search.as_ref().is_none_or(|q| {
                // secure 模式禁止通过查询结果推断被隐藏的原文。
                if state.redaction_policy.is_enabled() {
                    return false;
                }
                !snapshot::unavailable(row, "sql")
                    && row["sql"]
                        .as_str()
                        .is_some_and(|s| s.to_lowercase().contains(q))
            })
    });
    data.rows.sort_by(|a, b| {
        let missing = snapshot::unavailable(a, order).cmp(&snapshot::unavailable(b, order));
        if !missing.is_eq() {
            return missing;
        }
        let cmp = compare_values(a, b, order);
        (if descending { cmp.reverse() } else { cmp })
            .then_with(|| snapshot::identity(a).cmp(&snapshot::identity(b)))
    });
    let total = data.rows.len();
    let from = page.saturating_sub(1).saturating_mul(size);
    let rows: Vec<&Value> = data.rows.iter().skip(from).take(size).collect();
    data.response(json!({"apiVersion": API_VERSION, "total": total, "page": page, "perPageCount": size, "rows": rows}))
}

/// 单条 SQL 读取保留服务/池上下文和现有服务端解析结果。
pub(crate) async fn sql_detail(
    State(state): State<super::MonitorViewServlet>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let data = snapshot::load(state.backend.source(), true).await;
    let rows: Vec<&Value> = data
        .rows
        .iter()
        .filter(|row| (row["id"] == id || row["hash"] == id) && snapshot::matches(row, &params))
        .collect();
    match rows.as_slice() {
        [row] => {
            let mut value = (*row).clone();
            if !snapshot::unavailable(row, "sql") {
                let parsed =
                    super::sql_parse::parse_sql_stat(row["sql"].as_str().unwrap_or_default())
                        .to_json();
                if let (Some(target), Some(fields)) = (value.as_object_mut(), parsed.as_object()) {
                    target.extend(fields.clone());
                }
            }
            data.response(json!({"apiVersion": API_VERSION, "sql": value}))
        }
        [] if data.successful == 0 && !data.failures.is_empty() => data.response(json!({})),
        [] => snapshot::error(StatusCode::NOT_FOUND, "未找到 SQL"),
        _ => snapshot::error(StatusCode::CONFLICT, "SQL 身份不唯一，请指定服务与数据源"),
    }
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

/// 汇总真实 Wall 内容；来源不支持的字段不伪造零值。
pub(crate) async fn wall(
    State(state): State<super::MonitorViewServlet>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    super::v1_health::wall_response(state.backend.source(), &params).await
}

/// `GET /druid/api/v1/services` —— 已接入的服务。
pub(crate) async fn services(State(state): State<super::MonitorViewServlet>) -> Response {
    Json(json!({
        "apiVersion": API_VERSION,
        "services": state.backend.source().services(),
    }))
    .into_response()
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
            "/druid/api/v1/sql/{id}": {"get": {"summary": "One SQL stat with parsed metadata"}},
            "/druid/api/v1/wall": {"get": {"summary": "Wall statistics"}},
            "/druid/api/v1/services": {"get": {"summary": "Attached services"}},
            "/druid/api/v1/openapi.json": {"get": {"summary": "This document"}},
            "/druid/api/v1/health": {"get": {"summary": "采集来源、仓库容量与拒绝计数"}},
            "/druid/api/v1/history": {"get": {"summary": "有界进程内数值采样历史；serviceId、datasourceId、limit 查询"}},
            "/druid/api/v1/datasources/{identity}/connections": {"get": {"summary": "池化连接与能力状态"}},
            "/druid/api/v1/datasources/{identity}/stack": {"get": {"summary": "活跃连接借出堆栈"}},
        },
    }))
    .into_response()
}
