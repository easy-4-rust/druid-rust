use super::DruidStatManagerFacade;
use serde_json::{json, Value};

/// Java Druid 管理 JSON 协议服务。
///
/// 对应 Java：`com.alibaba.druid.stat.DruidStatService`。输入是 servlet 风格
/// path/query，输出保持 `ResultCode` / `Content` 包装。
#[derive(Debug, Default, Clone, Copy)]
pub struct DruidStatService;

impl DruidStatService {
    pub const RESULT_CODE_SUCCESS: i32 = 1;
    pub const RESULT_CODE_ERROR: i32 = -1;

    /// 分派管理 URL 并返回 JSON。
    #[must_use]
    pub fn service(&self, url: &str) -> String {
        let (path, query) = url.split_once('?').unwrap_or((url, ""));
        let parameters = form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect::<std::collections::HashMap<_, _>>();
        let facade = DruidStatManagerFacade::global();
        let result = match path {
            "/basic.json" => Self::success(facade.basic_stat()),
            "/reset-all.json" => {
                facade.reset_all();
                Self::success(Value::Null)
            }
            "/log-and-reset.json" => {
                facade.reset_all();
                Self::success(Value::Null)
            }
            "/datasource.json" => Self::success(facade.data_source_stat_data_list()),
            "/sql.json" => Self::success(Self::page(
                facade.sql_stat_data_list(parse_u64(parameters.get("dataSourceId"))),
                &parameters,
            )),
            "/wall.json" => {
                Self::success(facade.wall_stat_data(parse_u64(parameters.get("dataSourceId"))))
            }
            _ if path.starts_with("/sql-") && path.ends_with(".json") => {
                let id = between_id(path, "/sql-", ".json");
                match id.and_then(|id| facade.sql_stat_data(id)) {
                    Some(content) => Self::success(content),
                    None => Self::error(Value::Null),
                }
            }
            _ if path.starts_with("/datasource-") && path.ends_with(".json") => {
                let id = between_id(path, "/datasource-", ".json");
                match id.and_then(|id| facade.data_source_stat_data(id)) {
                    Some(content) => Self::success(content),
                    None => Self::error(Value::Null),
                }
            }
            _ if path.starts_with("/wall-") && path.ends_with(".json") => {
                let id = between_id(path, "/wall-", ".json");
                match id {
                    Some(id) => Self::success(facade.wall_stat_data(Some(id))),
                    None => Self::error(Value::Null),
                }
            }
            _ if path.starts_with("/connectionInfo-") && path.ends_with(".json") => {
                let id = between_id(path, "/connectionInfo-", ".json");
                match id.and_then(|id| facade.pooling_connection_info(id)) {
                    Some(content) => Self::success(content),
                    None => Self::error(Value::Null),
                }
            }
            "/activeConnectionStackTrace.json" => {
                Self::success(facade.active_connection_stack_trace_list())
            }
            _ if path.starts_with("/activeConnectionStackTrace-") && path.ends_with(".json") => {
                let id = between_id(path, "/activeConnectionStackTrace-", ".json");
                match id.and_then(|id| facade.active_connection_stack_trace(id)) {
                    Some(content) => Self::success(content),
                    None => Self::error(Value::Null),
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
    ) -> Vec<Value> {
        let order_by = parameters.get("orderBy").map_or("SQL", String::as_str);
        let descending = parameters
            .get("orderType")
            .is_some_and(|value| value == "desc");
        values.sort_by(|left, right| {
            let left = left.get(order_by).map(Value::to_string).unwrap_or_default();
            let right = right
                .get(order_by)
                .map(Value::to_string)
                .unwrap_or_default();
            if descending {
                right.cmp(&left)
            } else {
                left.cmp(&right)
            }
        });
        let page = parse_usize(parameters.get("page")).unwrap_or(1).max(1);
        let per_page = parse_usize(parameters.get("perPageCount")).unwrap_or(usize::MAX);
        let from = page.saturating_sub(1).saturating_mul(per_page);
        values.into_iter().skip(from).take(per_page).collect()
    }
}

fn between_id(path: &str, prefix: &str, suffix: &str) -> Option<u64> {
    path.strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse()
        .ok()
}

fn parse_u64(value: Option<&String>) -> Option<u64> {
    value?.parse().ok()
}

fn parse_usize(value: Option<&String>) -> Option<usize> {
    value?.parse().ok()
}
