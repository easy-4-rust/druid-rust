//! 版本化管理 API。
//!
//! Java 兼容的 `.json` 路由混合了三种字段命名约定与一套信封错误语义，
//! 分离前端不应为此逐个特判。`/druid/api/v1` 提供一套一致的契约：
//! 字段一律 camelCase，错误一律用 HTTP 状态码表达。
//!
//! 数据来源与 Java 兼容路由一致：远端优先、仓库兜底，
//! 两者不会给出互相矛盾的结论。

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::model::dto::{DataSourceContent, SqlListContent};
use crate::service::StatQuery;

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
fn sql_row(stat: &SqlListContent) -> Value {
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

/// 构造一个 camelCase 的数据源摘要。
fn datasource_summary(source: &DataSourceContent) -> Value {
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
    })
}

/// 把 Java 形状的数据源完整配置映射为 camelCase 详情。
///
/// Java 的 datasource 页展示全部 ~74 个字段；版本化 API 的详情路由
/// 原样保留这些配置，控制台据此渲染完整面板。
fn datasource_detail(source: &DataSourceContent) -> Value {
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

/// `GET /druid/api/v1/basic` —— Admin 自身的运行信息。
pub(crate) async fn basic(State(state): State<super::MonitorViewServlet>) -> Response {
    let rows = state
        .backend
        .source()
        .datasource_rows()
        .await
        .unwrap_or_default();
    let mut drivers: Vec<String> = rows.iter().filter_map(|c| c.db_type.clone()).collect();
    drivers.sort();
    drivers.dedup();
    let sql_count = state
        .backend
        .source()
        .sql_rows(&StatQuery::default())
        .await
        .map(|rows| rows.len())
        .unwrap_or(0);
    Json(json!({
        "apiVersion": API_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "drivers": drivers,
        "datasourceCount": rows.len(),
        "sqlCount": sql_count,
        "backend": if state.backend.source().has_remote() { "poll" } else { "push" },
    }))
    .into_response()
}

/// `GET /druid/api/v1/datasources` —— 数据源列表。
pub(crate) async fn datasources(State(state): State<super::MonitorViewServlet>) -> Response {
    let rows = state
        .backend
        .source()
        .datasource_rows()
        .await
        .unwrap_or_default();
    Json(json!({
        "apiVersion": API_VERSION,
        "datasources": rows.iter().map(datasource_summary).collect::<Vec<Value>>(),
    }))
    .into_response()
}

/// `GET /druid/api/v1/datasources/{identity}` —— 单个数据源。
pub(crate) async fn datasource(
    State(state): State<super::MonitorViewServlet>,
    Path(identity): Path<i64>,
) -> Response {
    let rows = state
        .backend
        .source()
        .datasource_rows()
        .await
        .unwrap_or_default();
    match rows.iter().find(|content| content.identity == identity) {
        Some(content) => Json(json!({
            "apiVersion": API_VERSION,
            "datasource": datasource_detail(content),
        }))
        .into_response(),
        None => (StatusCode::NOT_FOUND, "no datasource matches the identity").into_response(),
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
    let descending = parameters
        .get("orderType")
        .is_some_and(|value| value == "desc");
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

    let mut rows = state
        .backend
        .source()
        .sql_rows(&StatQuery {
            service_name: parameters.get("serviceName").cloned(),
            order_by: order_by.clone(),
            order_type: if descending {
                "desc".to_owned()
            } else {
                "asc".to_owned()
            },
            page,
            per_page_count: per_page,
        })
        .await
        .unwrap_or_default();

    rows.sort_by(|left, right| {
        let ordering = compare_values(&sql_row(left), &sql_row(right), &order_by);
        if descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    let total = rows.len();
    let from = (page - 1) * per_page;
    let page_rows: Vec<Value> = if from >= total {
        Vec::new()
    } else {
        rows.iter().skip(from).take(per_page).map(sql_row).collect()
    };
    Json(json!({
        "apiVersion": API_VERSION,
        "total": total,
        "page": page,
        "perPageCount": per_page,
        "rows": page_rows,
    }))
    .into_response()
}

/// `GET /druid/api/v1/sql/{id}` —— 单条 SQL 统计 + 服务端解析的 parsed* 字段。
///
/// 语义对齐 Java 的 `sql-{id}.json`：不带 serviceId 时按 id/hash 全局查；
/// 返回时在 camelCase 行上追加 `formattedSql` 与 `parsedTable` / `parsedFields` /
/// `parsedConditions` / `parsedRelationships` / `parsedOrderbycolumns`。
/// 解析失败的字段为 null，前端回退客户端简易解析。
pub(crate) async fn sql_detail(
    State(state): State<super::MonitorViewServlet>,
    Path(id): Path<i64>,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let service_filter = parameters.get("serviceId").cloned();
    let rows = state
        .backend
        .source()
        .sql_rows(&StatQuery {
            service_name: None,
            order_by: "id".to_owned(),
            order_type: "asc".to_owned(),
            page: 1,
            per_page_count: usize::MAX,
        })
        .await
        .unwrap_or_default();
    let found = rows.iter().find(|row| {
        let service_matches = match (&service_filter, &row.service_id) {
            (Some(want), Some(have)) => want.as_str() == &have[..],
            (Some(_), None) => false,
            (None, _) => true,
        };
        service_matches && (row.id == id || row.hash == id)
    });
    match found {
        Some(row) => {
            let mut value = sql_row(row);
            if let Value::Object(ref mut map) = value {
                let stat = super::sql_parse::parse_sql_stat(row.sql.as_deref().unwrap_or_default());
                if let Some(fields) = stat.to_json().as_object() {
                    for (key, parsed) in fields {
                        map.insert(key.clone(), parsed.clone());
                    }
                }
            }
            Json(json!({
                "apiVersion": API_VERSION,
                "sql": value,
            }))
            .into_response()
        }
        None => (StatusCode::NOT_FOUND, "no sql stat matches the id").into_response(),
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

/// `GET /druid/api/v1/wall` —— SQL 防火墙统计。
pub(crate) async fn wall(State(state): State<super::MonitorViewServlet>) -> Response {
    match state.backend.source().wall().await {
        Ok(wall) => Json(json!({"apiVersion": API_VERSION, "wall": wall.content})).into_response(),
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
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
        },
    }))
    .into_response()
}
