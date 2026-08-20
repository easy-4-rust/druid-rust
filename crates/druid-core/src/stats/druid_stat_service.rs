use super::DruidStatManagerFacade;
use serde_json::{json, Value};
use std::cmp::Ordering;

/// Java Druid 管理 JSON 协议服务。
///
/// 对应 Java：`com.alibaba.druid.stat.DruidStatService`。输入是 servlet 风格
/// path/query，输出保持 `ResultCode` / `Content` 包装。
#[derive(Debug, Default, Clone, Copy)]
pub struct DruidStatService;

impl DruidStatService {
    pub const RESULT_CODE_SUCCESS: i32 = 1;
    pub const RESULT_CODE_ERROR: i32 = -1;

    /// 返回管理 reset 总门禁。
    #[must_use]
    pub fn is_reset_enable(&self) -> bool {
        DruidStatManagerFacade::global().is_reset_enable()
    }

    /// 设置管理 reset 总门禁。
    pub fn set_reset_enable(&self, value: bool) {
        DruidStatManagerFacade::global().set_reset_enable(value);
    }

    /// 分派管理 URL 并返回 JSON。
    #[must_use]
    pub fn service(&self, url: &str) -> String {
        let parameters = parameters(url);
        let facade = DruidStatManagerFacade::global();
        let result = match url {
            "/basic.json" => Self::success(facade.basic_stat()),
            "/reset-all.json" => {
                facade.reset_all();
                Self::success(Value::Null)
            }
            "/log-and-reset.json" => {
                facade.log_and_reset_data_source();
                Self::success(Value::Null)
            }
            "/datasource.json" => Self::success(facade.data_source_stat_data_list()),
            _ if url.starts_with("/sql.json") => Self::success(Self::page(
                facade.sql_stat_data_list(parse_u64(parameters.get("dataSourceId"))),
                &parameters,
            )),
            _ if url.starts_with("/wall.json") => Self::success(Self::sort_wall_stat(
                facade.wall_stat_data(parse_u64(parameters.get("dataSourceId"))),
                &parameters,
            )),
            _ if url.starts_with("/sql-") && url.contains(".json") => {
                let id = between_id(url, "/sql-", ".json");
                match id.and_then(|id| facade.sql_stat_data(id)) {
                    Some(content) => Self::success(Self::sql_detail(content)),
                    None => Self::error(Value::Null),
                }
            }
            _ if url.starts_with("/datasource-") => {
                let id = between_id(url, "/datasource-", ".");
                match id.and_then(|id| facade.data_source_stat_data(id)) {
                    Some(content) => Self::success(content),
                    None => Self::error(Value::Null),
                }
            }
            _ if url.starts_with("/wall-") && url.contains(".json") => {
                let id = between_id(url, "/wall-", ".json");
                match id {
                    Some(id) => Self::success(facade.wall_stat_data(Some(id))),
                    None => Self::error(Value::Null),
                }
            }
            _ if url.starts_with("/connectionInfo-") && url.ends_with(".json") => {
                let id = between_id(url, "/connectionInfo-", ".");
                match id.and_then(|id| facade.pooling_connection_info(id)) {
                    Some(content) => Self::success(content),
                    None => Self::error(Value::Null),
                }
            }
            "/activeConnectionStackTrace.json" => {
                Self::success(facade.active_connection_stack_trace_list())
            }
            _ if url.starts_with("/activeConnectionStackTrace-") && url.ends_with(".json") => {
                let id = between_id(url, "/activeConnectionStackTrace-", ".");
                match id.and_then(|id| facade.active_connection_stack_trace(id)) {
                    Some(content) => Self::success(content),
                    None => Self::error("require set removeAbandoned=true".into()),
                }
            }
            _ => Self::error(
                "Do not support this request, please contact with administrator.".into(),
            ),
        };
        serde_json::to_string(&result)
            .unwrap_or_else(|error| format!(r#"{{"ResultCode":-1,"Content":"{error}"}}"#))
    }

    fn success(content: impl Into<Value>) -> Value {
        json!({"ResultCode": Self::RESULT_CODE_SUCCESS, "Content": content.into()})
    }

    fn error(content: Value) -> Value {
        json!({"ResultCode": Self::RESULT_CODE_ERROR, "Content": content})
    }

    fn page(
        mut values: Vec<Value>,
        parameters: &std::collections::HashMap<String, String>,
    ) -> Value {
        // Java comparatorOrderBy 在空列表时返回 null。
        if values.is_empty() {
            return Value::Null;
        }
        let order_by = parameters.get("orderBy").map_or("SQL", String::as_str);
        let descending = parameters
            .get("orderType")
            .is_some_and(|value| value == "desc");
        if !order_by.trim().is_empty() {
            values.sort_by(|left, right| {
                let order =
                    compare_map_value(value_by_key(left, order_by), value_by_key(right, order_by));
                if descending {
                    order.reverse()
                } else {
                    order
                }
            });
        }
        let page = parse_usize(parameters.get("page")).unwrap_or(1);
        let per_page = parse_usize(parameters.get("perPageCount")).unwrap_or(usize::MAX);
        let from = page.saturating_sub(1).saturating_mul(per_page);
        values
            .into_iter()
            .skip(from)
            .take(per_page)
            .collect::<Vec<_>>()
            .into()
    }

    fn sort_wall_stat(
        mut wall: Value,
        parameters: &std::collections::HashMap<String, String>,
    ) -> Value {
        let Some(map) = wall.as_object_mut() else {
            return wall;
        };
        for key in ["tables", "functions"] {
            let Some(values) = map.remove(key).and_then(|value| value.as_array().cloned()) else {
                continue;
            };
            map.insert(key.to_owned(), Self::page(values, parameters));
        }
        wall
    }

    fn sql_detail(mut content: Value) -> Value {
        let Some(map) = content.as_object_mut() else {
            return content;
        };
        let Some(sql) = map.get("SQL").and_then(Value::as_str).map(str::to_owned) else {
            return content;
        };
        let db_type = map
            .get("DbType")
            .and_then(Value::as_str)
            .and_then(crate::sql::DbType::of)
            .unwrap_or(crate::sql::DbType::Other);
        if let Ok(formatted) = crate::sql::SqlUtils::format(&sql, db_type) {
            map.insert("formattedSql".to_owned(), formatted.into());
        }
        if let Some(millis) = map
            .get("MaxTimespanOccurTime")
            .and_then(Value::as_u64)
            .and_then(|millis| i64::try_from(millis).ok())
        {
            use chrono::{Local, TimeZone};
            if let Some(time) = Local.timestamp_millis_opt(millis).single() {
                map.insert(
                    "MaxTimespanOccurTime".to_owned(),
                    time.format("%Y/%m/%d %H:%M:%S:%3f").to_string().into(),
                );
            }
        }
        content
    }
}

fn between_id(path: &str, prefix: &str, suffix: &str) -> Option<u64> {
    let rest = path.strip_prefix(prefix)?;
    let end = rest.find(suffix)?;
    rest[..end].parse().ok()
}

fn parse_u64(value: Option<&String>) -> Option<u64> {
    value?.parse().ok()
}

fn parse_usize(value: Option<&String>) -> Option<usize> {
    value?.parse().ok()
}

/// 按 Java `getParameters` 解析查询串：不 URL-decode，重复键以后者覆盖。
fn parameters(url: &str) -> std::collections::HashMap<String, String> {
    let trimmed = url.trim();
    let Some((_, query)) = trimmed.split_once('?') else {
        return std::collections::HashMap::new();
    };
    if query.is_empty() {
        return std::collections::HashMap::new();
    }
    query
        .split('&')
        .filter_map(|parameter| {
            let index = parameter.find('=')?;
            (index > 0).then(|| {
                (
                    parameter[..index].to_owned(),
                    parameter[index + 1..].to_owned(),
                )
            })
        })
        .collect()
}

fn value_by_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let map = value.as_object()?;
    let Some(bracket) = key.find('[').filter(|index| *index > 0) else {
        return map.get(key);
    };
    let close = key[bracket..].find(']')? + bracket;
    let index = key[bracket + 1..close].parse::<usize>().ok()?;
    map.get(&key[..bracket])?.as_array()?.get(index)
}

fn compare_map_value(left: Option<&Value>, right: Option<&Value>) -> Ordering {
    match (left, right) {
        (None | Some(Value::Null), None | Some(Value::Null)) => Ordering::Equal,
        (None | Some(Value::Null), _) => Ordering::Less,
        (_, None | Some(Value::Null)) => Ordering::Greater,
        (Some(Value::Number(left)), Some(Value::Number(right))) => {
            match (left.as_i64(), right.as_i64()) {
                (Some(left), Some(right)) => {
                    let delta = left.wrapping_sub(right) as i32;
                    delta.cmp(&0)
                }
                _ => left
                    .as_f64()
                    .partial_cmp(&right.as_f64())
                    .unwrap_or(Ordering::Equal),
            }
        }
        (Some(Value::String(left)), Some(Value::String(right))) => left.cmp(right),
        _ => Ordering::Equal,
    }
}
