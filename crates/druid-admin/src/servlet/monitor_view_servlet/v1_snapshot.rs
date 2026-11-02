//! 版本化读取快照：复用 DTO，保留字段存在性、来源身份及部分失败。
use super::v1_api::{datasource_detail, sql_row};
use crate::model::dto::{DataSourceContent, SqlListContent};
use crate::service::ManagementSource;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

/// 一个资源的有界聚合快照；对应 Rust 控制台扩展，非 Java 迁移对象。
pub(super) struct Snapshot {
    pub rows: Vec<Value>,
    pub failures: Vec<Value>,
    pub successful: usize,
}
impl Snapshot {
    pub fn response(&self, mut body: Value) -> Response {
        body["partial"] = json!(!self.failures.is_empty());
        body["failures"] = json!(self.failures);
        body["observedAt"] = json!(chrono::Utc::now().to_rfc3339());
        if self.successful == 0 && !self.failures.is_empty() {
            body["message"] = json!("所有采集来源读取失败");
            return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
        }
        Json(body).into_response()
    }
}

/// 判断一个值是否仍是 DTO 默认值。
///
/// push 协议未携带的字段会被 DTO 反序列化为 null、空串、空数组、0 或 false；
/// 只有仍为默认值的字段才标为未上报。调用方直接注入的非默认值属于真实数据，
/// 不能仅因 transport 是 push 就被隐藏。
fn default_like(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        Value::Number(number) => number.as_f64() == Some(0.0),
        Value::Bool(flag) => !flag,
    }
}

fn mapped(raw: &Value, sql: bool) -> Option<Value> {
    if !raw.is_object() {
        return None;
    }
    let mut row = if sql {
        sql_row(&serde_json::from_value::<SqlListContent>(raw.clone()).ok()?)
    } else {
        datasource_detail(&serde_json::from_value::<DataSourceContent>(raw.clone()).ok()?)
    };
    let present: Vec<String> = raw
        .as_object()?
        .keys()
        .map(|key| key.to_ascii_lowercase())
        .collect();
    let unavailable: Vec<String> = row
        .as_object()?
        .keys()
        .filter(|key| !present.contains(&key.to_ascii_lowercase()))
        .cloned()
        .collect();
    row["unavailableFields"] = json!(unavailable);
    Some(row)
}

/// 从真实 poll 资源与推送仓库读取；不再以远端非空作为替代仓库的依据。
pub(super) async fn load(source: &ManagementSource, sql: bool) -> Snapshot {
    let mut snapshot = Snapshot {
        rows: Vec::new(),
        failures: Vec::new(),
        successful: usize::from(!source.has_remote()),
    };
    if let Some(remote) = source.remote_service() {
        let path = if sql {
            "/druid/sql.json?page=1&perPageCount=10000&orderBy=ID&orderType=asc"
        } else {
            "/druid/datasource.json"
        };
        let response = remote.console_read(path).await;
        snapshot.failures = response["failures"].as_array().cloned().unwrap_or_default();
        for observation in response["observations"].as_array().into_iter().flatten() {
            let Some(rows) = observation["content"].as_array() else {
                snapshot.failures.push(
                    json!({"serviceId": observation["serviceId"], "reason": "节点未返回列表"}),
                );
                continue;
            };
            snapshot.successful += 1;
            if sql && rows.len() >= 10000 {
                snapshot.failures.push(json!({"serviceId": observation["serviceId"], "reason": "节点 SQL 超过单次读取预算，统计仅覆盖本次取得的记录"}));
            }
            for raw in rows.iter().take(10000) {
                if let Some(mut row) = mapped(raw, sql) {
                    for key in ["serviceId", "serviceName", "address", "port"] {
                        row[key] = observation[key].clone();
                    }
                    row["source"] = json!("poll");
                    // 这几个来源字段由注册节点身份补齐，不属于指标缺失。
                    if let Some(fields) = row["unavailableFields"].as_array_mut() {
                        fields.retain(|key| {
                            !["serviceId", "serviceName", "address", "port"]
                                .contains(&key.as_str().unwrap_or_default())
                        });
                    }
                    snapshot.rows.push(row);
                } else {
                    snapshot.failures.push(json!({"serviceId": observation["serviceId"], "reason": "统计记录无法解析"}));
                }
            }
        }
    }
    for (id, entry) in source.repository_view().console_entries() {
        snapshot.successful += 1;
        let rows = if sql {
            entry.sql_stats.values().map(sql_row).collect::<Vec<_>>()
        } else {
            vec![datasource_detail(&entry.datasource)]
        };
        for mut row in rows {
            let available = if sql {
                vec![
                    "id",
                    "hash",
                    "dataSource",
                    "executeCount",
                    "totalTime",
                    "fetchRowCount",
                    "effectedRowCount",
                    "dbType",
                ]
            } else {
                vec![
                    "identity",
                    "name",
                    "dbType",
                    "driverClassName",
                    "activeCount",
                    "poolingCount",
                    "maxActive",
                    "waitThreadCount",
                    "executeCount",
                ]
            };
            let missing: Vec<String> = row
                .as_object()
                .unwrap()
                .keys()
                .filter(|key| {
                    !available.contains(&key.as_str())
                        && !["serviceId", "serviceName", "instanceId", "bootId"]
                            .contains(&key.as_str())
                        && row
                            .as_object()
                            .and_then(|object| object.get(key.as_str()))
                            .is_none_or(default_like)
                })
                .cloned()
                .collect();
            row["unavailableFields"] = json!(missing);
            row["serviceId"] = json!(id.service_identity.shared_service_id());
            row["serviceName"] = json!(id.service_identity.shared_service_name());
            row["instanceId"] = json!(id.service_identity.shared_instance_id());
            row["bootId"] = json!(id.service_identity.shared_boot_id());
            row["source"] = json!("push");
            if sql {
                row["dataSource"] = json!(id.identity);
            }
            snapshot.rows.push(row);
        }
    }
    snapshot.rows.sort_by_key(identity);
    snapshot
}

pub(super) fn identity(row: &Value) -> String {
    format!(
        "{}|{}|{}",
        row["serviceId"],
        row["dataSource"].as_i64().unwrap_or(0),
        row["identity"]
            .as_i64()
            .or_else(|| row["id"].as_i64())
            .unwrap_or(0)
    )
}
pub(super) fn unavailable(row: &Value, key: &str) -> bool {
    row[key].is_null()
        || row["unavailableFields"]
            .as_array()
            .is_some_and(|keys| keys.iter().any(|v| v == key))
}
pub(super) fn matches(row: &Value, params: &HashMap<String, String>) -> bool {
    for key in ["serviceId", "serviceName"] {
        if let Some(value) = params.get(key).filter(|v| !v.is_empty()) {
            if row[key].as_str() != Some(value.as_str()) {
                return false;
            }
        }
    }
    if let Some(id) = params.get("datasourceId").filter(|v| !v.is_empty()) {
        let actual = row["dataSource"]
            .as_i64()
            .or_else(|| row["identity"].as_i64());
        if actual.map(|n| n.to_string()).as_ref() != Some(id) {
            return false;
        }
    }
    true
}
pub(super) fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({"apiVersion": "v1", "message": message})),
    )
        .into_response()
}
