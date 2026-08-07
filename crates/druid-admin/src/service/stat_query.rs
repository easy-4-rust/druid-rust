use serde::{Deserialize, Serialize};
use validator::Validate;

use std::collections::HashMap;

use super::MonitorStatServiceError;

/// 统计列表的排序、分页与服务筛选参数。
#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StatQuery {
    /// 目标服务名。
    #[validate(length(min = 1))]
    pub service_name: Option<String>,
    /// 排序字段。
    #[serde(default = "default_order_by")]
    #[validate(length(min = 1))]
    pub order_by: String,
    /// `asc` 或 `desc`；其他值按 Java 语义归一为 `asc`。
    #[serde(default = "default_order_type")]
    pub order_type: String,
    /// 从 1 开始的页号。
    #[serde(default = "default_page")]
    #[validate(range(min = 1))]
    pub page: usize,
    /// 每页条数。
    #[serde(default = "default_per_page_count")]
    #[validate(range(min = 1))]
    pub per_page_count: usize,
}

impl Default for StatQuery {
    fn default() -> Self {
        Self {
            service_name: None,
            order_by: default_order_by(),
            order_type: default_order_type(),
            page: default_page(),
            per_page_count: default_per_page_count(),
        }
    }
}

impl StatQuery {
    /// 从 Java `service(String)` 查询参数映射构造统计查询。
    pub fn from_parameters(
        parameters: &HashMap<String, String>,
    ) -> Result<Self, MonitorStatServiceError> {
        let mut query = Self {
            service_name: parameters.get("serviceName").cloned(),
            ..Self::default()
        };
        if let Some(order_by) = parameters.get("orderBy") {
            query.order_by.clone_from(order_by);
        }
        if let Some(order_type) = parameters.get("orderType") {
            query.order_type = if order_type == "desc" {
                "desc".to_owned()
            } else {
                "asc".to_owned()
            };
        }
        if let Some(page) = parameters.get("page").filter(|value| !value.is_empty()) {
            query.page = page
                .parse()
                .map_err(|_| MonitorStatServiceError::InvalidParameter {
                    name: "page",
                    value: page.clone(),
                })?;
        }
        if let Some(per_page_count) = parameters
            .get("perPageCount")
            .filter(|value| !value.is_empty())
        {
            query.per_page_count =
                per_page_count
                    .parse()
                    .map_err(|_| MonitorStatServiceError::InvalidParameter {
                        name: "perPageCount",
                        value: per_page_count.clone(),
                    })?;
        }
        Ok(query)
    }
}

fn default_order_by() -> String {
    "SQL".to_owned()
}

fn default_order_type() -> String {
    "asc".to_owned()
}

const fn default_page() -> usize {
    1
}

const fn default_per_page_count() -> usize {
    1000
}
