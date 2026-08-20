#![allow(clippy::case_sensitive_file_extension_comparisons)]
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::config::MonitorProperties;
use crate::model::dto::{
    ConnectionResult, DataSourceResult, SqlDetailResult, SqlListResult, WallResult, WebResult,
};
use crate::model::ServiceNode;
use crate::util::{HttpClient, HttpUtil};

use super::{
    AdminStatProvider, DiscoveryClient, EmptyAdminStatProvider, K8sDiscoveryClient,
    MonitorStatServiceError, StatQuery,
};

/// 多节点 Druid 管理数据聚合服务。
///
/// 对应 Java: `com.alibaba.druid.admin.service.MonitorStatService`。
pub struct MonitorStatService {
    discovery_client: Arc<dyn DiscoveryClient>,
    monitor_properties: Arc<MonitorProperties>,
    k8s_discovery_client: Option<Arc<K8sDiscoveryClient>>,
    http_client: Arc<dyn HttpClient>,
    admin_stat_provider: Arc<dyn AdminStatProvider>,
    service_id_map: RwLock<HashMap<String, ServiceNode>>,
}

impl MonitorStatService {
    /// Java 管理协议成功码。
    pub const RESULT_CODE_SUCCESS: i32 = 1;
    /// Java 管理协议错误码。
    pub const RESULT_CODE_ERROR: i32 = -1;

    /// 创建聚合服务并注入 Rust 平台适配器。
    #[must_use]
    pub fn new(
        discovery_client: Arc<dyn DiscoveryClient>,
        monitor_properties: Arc<MonitorProperties>,
        k8s_discovery_client: Option<Arc<K8sDiscoveryClient>>,
        http_client: Arc<dyn HttpClient>,
    ) -> Self {
        Self {
            discovery_client,
            monitor_properties,
            k8s_discovery_client,
            http_client,
            admin_stat_provider: Arc::new(EmptyAdminStatProvider),
            service_id_map: RwLock::new(HashMap::new()),
        }
    }

    /// 注入 Web 与框架集成统计提供者。
    #[must_use]
    pub fn with_admin_stat_provider(
        mut self,
        admin_stat_provider: Arc<dyn AdminStatProvider>,
    ) -> Self {
        self.admin_stat_provider = admin_stat_provider;
        self
    }

    /// 返回配置的受监控应用。
    #[must_use]
    pub fn applications(&self) -> &[String] {
        &self.monitor_properties.applications
    }

    /// 按 Java `MonitorStatService#service(String)` 协议分派管理 URL。
    pub async fn service(&self, url: &str) -> Result<String, MonitorStatServiceError> {
        let parameters = Self::get_parameters(Some(url));
        let query = StatQuery::from_parameters(&parameters)?;

        if url.ends_with("serviceList.json") {
            return serde_json::to_string(self.applications()).map_err(Into::into);
        }
        if url == "/datasource.json" {
            return serde_json::to_string(&self.get_data_source_stat_data().await?)
                .map_err(Into::into);
        }
        if url.starts_with("/datasource-") {
            let result = serde_json::to_string(&self.get_data_source_stat_data().await?)?;
            return serde_json::to_string(&Self::return_json_result(
                Self::RESULT_CODE_SUCCESS,
                result,
            ))
            .map_err(Into::into);
        }
        if url.starts_with("/connectionInfo-") && url_path(url).ends_with(".json") {
            let id = parse_path_id(url, "/connectionInfo-", '&')?;
            let service_id = between(url_path(url), "&serviceId=", ".json")
                .or_else(|| parameters.get("serviceId").map(String::as_str))
                .ok_or_else(|| MonitorStatServiceError::InvalidParameter {
                    name: "serviceId",
                    value: String::new(),
                })?;
            return serde_json::to_string(
                &self
                    .get_pooling_connection_info_by_data_source_id(id, service_id)
                    .await?,
            )
            .map_err(Into::into);
        }
        if url.starts_with("/sql.json") {
            return serde_json::to_string(&self.get_sql_stat_data_list(&query).await?)
                .map_err(Into::into);
        }
        if url.starts_with("/wall.json") {
            return serde_json::to_string(&self.get_wall_stat_map(&query).await?)
                .map_err(Into::into);
        }
        if url.starts_with("/serviceId") && url_path(url).contains(".json") {
            let id = parse_embedded_id(url, "sql-", ".json")?;
            let service_id = between(url_path(url), "serviceId=", "&")
                .or_else(|| parameters.get("serviceId").map(String::as_str))
                .ok_or_else(|| MonitorStatServiceError::InvalidParameter {
                    name: "serviceId",
                    value: String::new(),
                })?;
            return serde_json::to_string(&self.get_sql_stat(id, service_id).await?)
                .map_err(Into::into);
        }
        if url.starts_with("/weburi.json") {
            return serde_json::to_string(&self.get_web_uri_stat_data_list(&query).await?)
                .map_err(Into::into);
        }
        if url.starts_with("/weburi-") && url_path(url).contains(".json") {
            let uri = between(url_path(url), "/weburi-", ".json").unwrap_or_default();
            return serde_json::to_string(&Self::return_json_result(
                Self::RESULT_CODE_SUCCESS,
                self.get_web_uri_stat_data(uri),
            ))
            .map_err(Into::into);
        }
        if url.starts_with("/webapp.json") {
            return serde_json::to_string(&Self::return_json_result(
                Self::RESULT_CODE_SUCCESS,
                self.get_web_app_stat_data_list(&query),
            ))
            .map_err(Into::into);
        }
        if url.starts_with("/websession.json") {
            return serde_json::to_string(&Self::return_json_result(
                Self::RESULT_CODE_SUCCESS,
                self.get_web_session_stat_data_list(&query),
            ))
            .map_err(Into::into);
        }
        if url.starts_with("/websession-") && url_path(url).contains(".json") {
            let id = between(url_path(url), "/websession-", ".json").unwrap_or_default();
            return serde_json::to_string(&Self::return_json_result(
                Self::RESULT_CODE_SUCCESS,
                self.get_web_session_stat_data(id),
            ))
            .map_err(Into::into);
        }
        if url.starts_with("/spring-detail.json") {
            let class = parameters
                .get("class")
                .map(String::as_str)
                .unwrap_or_default();
            let method = parameters
                .get("method")
                .map(String::as_str)
                .unwrap_or_default();
            return serde_json::to_string(&Self::return_json_result(
                Self::RESULT_CODE_SUCCESS,
                self.get_spring_method_stat_data(class, method),
            ))
            .map_err(Into::into);
        }
        if url.starts_with("/spring.json") {
            return serde_json::to_string(&Self::return_json_result(
                Self::RESULT_CODE_SUCCESS,
                self.get_spring_stat_data_list(&query),
            ))
            .map_err(Into::into);
        }
        serde_json::to_string(&Self::return_json_result(
            Self::RESULT_CODE_ERROR,
            "Do not support this request, please contact with administrator.",
        ))
        .map_err(Into::into)
    }

    /// 获取配置中全部受监控服务节点。
    pub async fn get_all_service_node_map(
        &self,
    ) -> Result<HashMap<String, ServiceNode>, MonitorStatServiceError> {
        let services = self.discovery_client.services();
        if services.is_empty() {
            if let (Some(kube_config), Some(k8s)) = (
                non_empty(self.monitor_properties.kube_config_file_path.as_deref()),
                self.k8s_discovery_client.as_ref(),
            ) {
                match k8s
                    .get_k8s_pods_info(
                        &self.monitor_properties.applications,
                        kube_config,
                        self.monitor_properties
                            .k8s_namespace
                            .as_deref()
                            .unwrap_or_default(),
                    )
                    .await
                {
                    Ok(nodes) => {
                        self.index_nodes(nodes.values());
                        return Ok(nodes);
                    }
                    Err(error) => {
                        // Java 版记录 Kubernetes 异常后继续返回注册中心结果。
                        tracing::error!(%error, "Kubernetes discovery failed");
                    }
                }
            }
        }
        self.discover_nodes(&services, |service_id| {
            self.monitor_properties
                .applications
                .iter()
                .any(|application| application == service_id)
        })
    }

    /// 获取指定服务名的全部节点。
    pub async fn get_service_all_node_map(
        &self,
        service_name: &str,
    ) -> Result<HashMap<String, ServiceNode>, MonitorStatServiceError> {
        let services = self.discovery_client.services();
        if services.is_empty() {
            if let (Some(kube_config), Some(k8s)) = (
                non_empty(self.monitor_properties.kube_config_file_path.as_deref()),
                self.k8s_discovery_client.as_ref(),
            ) {
                match k8s
                    .get_k8s_pods_info(
                        &[service_name.to_owned()],
                        kube_config,
                        self.monitor_properties
                            .k8s_namespace
                            .as_deref()
                            .unwrap_or_default(),
                    )
                    .await
                {
                    Ok(nodes) => {
                        self.index_nodes(nodes.values());
                        return Ok(nodes);
                    }
                    Err(error) => {
                        // Java 版记录 Kubernetes 异常后继续返回注册中心结果。
                        tracing::error!(%error, "Kubernetes discovery failed");
                    }
                }
            }
        }
        self.discover_nodes(&services, |service_id| {
            service_id.eq_ignore_ascii_case(service_name)
        })
    }

    fn discover_nodes(
        &self,
        services: &[String],
        include: impl Fn(&str) -> bool,
    ) -> Result<HashMap<String, ServiceNode>, MonitorStatServiceError> {
        let mut nodes = HashMap::new();
        for service in services {
            for instance in self.discovery_client.instances(service) {
                if !include(&instance.service_id) {
                    continue;
                }
                let id = instance.instance_id.or_else(|| {
                    instance
                        .metadata
                        .get("nacos.instanceId")
                        .map(|value| value.replace("@@", "-").replace('#', "-"))
                });
                let id = id.ok_or_else(|| MonitorStatServiceError::MissingInstanceId {
                    service: service.clone(),
                })?;
                let node = ServiceNode {
                    id,
                    port: u32::from(instance.port),
                    address: instance.host,
                    service_name: instance.service_id,
                };
                nodes.insert(node.map_key(), node);
            }
        }
        self.index_nodes(nodes.values());
        Ok(nodes)
    }

    fn index_nodes<'a>(&self, nodes: impl IntoIterator<Item = &'a ServiceNode>) {
        let mut service_id_map = self.service_id_map.write();
        for node in nodes {
            service_id_map.insert(node.id.clone(), node.clone());
        }
    }

    /// 聚合所有节点的数据源统计。
    pub async fn get_data_source_stat_data(
        &self,
    ) -> Result<DataSourceResult, MonitorStatServiceError> {
        let mut result = DataSourceResult::default();
        let mut content = Vec::new();
        for node in self.get_all_service_node_map().await?.values() {
            let url = node_url(node, "/druid/datasource.json");
            match HttpUtil::get::<DataSourceResult>(self.http_client.as_ref(), &url).await {
                Ok(remote) => {
                    for mut item in remote.content.into_iter().flatten() {
                        item.name = Some(node.service_name.clone());
                        item.service_id = Some(node.id.clone());
                        content.push(item);
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, node = %node.id, "skipping failed datasource node");
                }
            }
        }
        result.content = Some(content);
        Ok(result)
    }

    /// 聚合指定服务的 SQL 列表，并执行 Java 排序与分页。
    pub async fn get_sql_stat_data_list(
        &self,
        query: &StatQuery,
    ) -> Result<Value, MonitorStatServiceError> {
        let service_name = query.service_name.as_deref().unwrap_or_default();
        let mut content = Vec::new();
        for node in self.get_service_all_node_map(service_name).await?.values() {
            let url = request_url(node, "/druid/sql.json", query);
            match HttpUtil::get::<SqlListResult>(self.http_client.as_ref(), &url).await {
                Ok(remote) => {
                    for mut item in remote.content.into_iter().flatten() {
                        item.name = Value::String(node.service_name.clone());
                        item.address = Some(node.address.clone());
                        item.port = u16::try_from(node.port).ok();
                        item.service_id = Some(node.id.clone());
                        content.push(serde_json::to_value(item)?);
                    }
                }
                Err(error) => tracing::warn!(%error, node = %node.id, "skipping failed sql node"),
            }
        }
        let content = comparator_order_by(content, query);
        Ok(json!({"ResultCode": Self::RESULT_CODE_SUCCESS, "Content": content}))
    }

    /// 聚合指定服务的 Wall 统计。
    pub async fn get_wall_stat_map(
        &self,
        query: &StatQuery,
    ) -> Result<WallResult, MonitorStatServiceError> {
        let service_name = query.service_name.as_deref().unwrap_or_default();
        let mut result = WallResult::default();
        for node in self.get_service_all_node_map(service_name).await?.values() {
            let url = request_url(node, "/druid/wall.json", query);
            match HttpUtil::get::<WallResult>(self.http_client.as_ref(), &url).await {
                Ok(remote) => result.sum(&remote),
                Err(error) => tracing::warn!(%error, node = %node.id, "skipping failed wall node"),
            }
        }
        Ok(result)
    }

    /// 聚合指定服务所有节点的 Web URI 统计。
    pub async fn get_web_uri_stat_data_list(
        &self,
        query: &StatQuery,
    ) -> Result<Value, MonitorStatServiceError> {
        let service_name = query.service_name.as_deref().unwrap_or_default();
        let mut content = Vec::new();
        for node in self.get_service_all_node_map(service_name).await?.values() {
            let url = request_url(node, "/druid/weburi.json", query);
            match HttpUtil::get::<WebResult>(self.http_client.as_ref(), &url).await {
                Ok(remote) => {
                    for item in remote.content.into_iter().flatten() {
                        content.push(serde_json::to_value(item)?);
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, node = %node.id, "skipping failed web URI node");
                }
            }
        }
        Ok(json!({
            "ResultCode": Self::RESULT_CODE_SUCCESS,
            "Content": comparator_order_by(content, query)
        }))
    }

    /// 返回本进程指定 URI 的统计。
    #[must_use]
    pub fn get_web_uri_stat_data(&self, uri: &str) -> Option<Map<String, Value>> {
        self.admin_stat_provider.web_uri_stat(uri)
    }

    /// 返回本进程 Web 应用统计并分页排序。
    #[must_use]
    pub fn get_web_app_stat_data_list(&self, query: &StatQuery) -> Vec<Value> {
        comparator_order_by(
            maps_to_values(self.admin_stat_provider.web_app_stats()),
            query,
        )
    }

    /// 返回本进程 session 统计并分页排序。
    #[must_use]
    pub fn get_web_session_stat_data_list(&self, query: &StatQuery) -> Vec<Value> {
        comparator_order_by(
            maps_to_values(self.admin_stat_provider.web_session_stats()),
            query,
        )
    }

    /// 返回指定 session 统计。
    #[must_use]
    pub fn get_web_session_stat_data(&self, session_id: &str) -> Option<Map<String, Value>> {
        self.admin_stat_provider.web_session_stat(session_id)
    }

    /// 返回框架集成方法统计并分页排序。
    #[must_use]
    pub fn get_spring_stat_data_list(&self, query: &StatQuery) -> Vec<Value> {
        comparator_order_by(
            maps_to_values(self.admin_stat_provider.method_stats()),
            query,
        )
    }

    /// 返回指定类与方法的框架集成统计。
    #[must_use]
    pub fn get_spring_method_stat_data(
        &self,
        class: &str,
        method: &str,
    ) -> Option<Map<String, Value>> {
        self.admin_stat_provider.method_stat(class, method)
    }

    /// 查询指定服务节点的 SQL 详情。
    pub async fn get_sql_stat(
        &self,
        id: i64,
        service_id: &str,
    ) -> Result<SqlDetailResult, MonitorStatServiceError> {
        let node = self.node(service_id).await?;
        let url = node_url(&node, &format!("/druid/sql-{id}.json"));
        HttpUtil::get(self.http_client.as_ref(), &url)
            .await
            .map_err(Into::into)
    }

    /// 查询指定服务节点的数据源活跃连接。
    pub async fn get_pooling_connection_info_by_data_source_id(
        &self,
        id: i64,
        service_id: &str,
    ) -> Result<ConnectionResult, MonitorStatServiceError> {
        let _ = self.get_all_service_node_map().await?;
        let node = self.node(service_id).await?;
        let url = node_url(&node, &format!("/druid/connectionInfo-{id}.json"));
        HttpUtil::get(self.http_client.as_ref(), &url)
            .await
            .map_err(Into::into)
    }

    async fn node(&self, service_id: &str) -> Result<ServiceNode, MonitorStatServiceError> {
        if let Some(node) = self.service_id_map.read().get(service_id).cloned() {
            return Ok(node);
        }
        let _ = self.get_all_service_node_map().await?;
        self.service_id_map
            .read()
            .get(service_id)
            .cloned()
            .ok_or_else(|| MonitorStatServiceError::UnknownServiceId(service_id.to_owned()))
    }

    /// 构造 Java `returnJSONResult` 的有序 JSON 结构。
    pub fn return_json_result(content_code: i32, content: impl Serialize) -> Value {
        json!({"ResultCode": content_code, "Content": content})
    }

    /// 解析 Java 管理 URL 的查询参数，不执行 percent-decoding。
    #[must_use]
    pub fn get_parameters(url: Option<&str>) -> HashMap<String, String> {
        let Some(url) = url.map(str::trim).filter(|url| !url.is_empty()) else {
            return HashMap::new();
        };
        let Some((_, parameters)) = url.split_once('?') else {
            return HashMap::new();
        };
        parameters
            .split('&')
            .filter_map(|parameter| {
                let (name, value) = parameter.split_once('=')?;
                (!name.is_empty()).then(|| (name.to_owned(), value.to_owned()))
            })
            .collect()
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

fn node_url(node: &ServiceNode, path: &str) -> String {
    format!("http://{}:{}{path}", node.address, node.port)
}

fn request_url(node: &ServiceNode, path: &str, query: &StatQuery) -> String {
    format!(
        "{}?orderBy={}&orderType={}&page={}&perPageCount={}&",
        node_url(node, path),
        query.order_by,
        query.order_type,
        query.page,
        query.per_page_count
    )
}

fn comparator_order_by(mut array: Vec<Value>, query: &StatQuery) -> Vec<Value> {
    let descending = query.order_type == "desc";
    if !query.order_by.trim().is_empty() {
        array.sort_by(|left, right| {
            let ordering = compare_values(
                left.get(&query.order_by).unwrap_or(&Value::Null),
                right.get(&query.order_by).unwrap_or(&Value::Null),
            );
            if descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }
    let from = query
        .page
        .saturating_sub(1)
        .saturating_mul(query.per_page_count);
    if from >= array.len() {
        return Vec::new();
    }
    let to = from.saturating_add(query.per_page_count).min(array.len());
    array.drain(from..to).collect()
}

fn compare_values(left: &Value, right: &Value) -> Ordering {
    match (left, right) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Number(left), Value::Number(right)) => left
            .as_f64()
            .partial_cmp(&right.as_f64())
            .unwrap_or(Ordering::Equal),
        (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
        (Value::String(left), Value::String(right)) => left.cmp(right),
        _ => value_text(left).cmp(&value_text(right)),
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Object(value) => Value::Object(value.clone()).to_string(),
        Value::Array(value) => Value::Array(value.clone()).to_string(),
        _ => value.to_string(),
    }
}

fn maps_to_values(maps: Vec<Map<String, Value>>) -> Vec<Value> {
    maps.into_iter().map(Value::Object).collect()
}

fn url_path(url: &str) -> &str {
    url.split_once('?').map_or(url, |(path, _)| path)
}

fn between<'a>(value: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let (_, tail) = value.split_once(start)?;
    Some(tail.split_once(end).map_or(tail, |(value, _)| value))
}

fn parse_embedded_id(url: &str, start: &str, end: &str) -> Result<i64, MonitorStatServiceError> {
    let value = between(url_path(url), start, end).unwrap_or_default();
    value
        .parse()
        .map_err(|_| MonitorStatServiceError::InvalidParameter {
            name: "id",
            value: value.to_owned(),
        })
}

fn parse_path_id(url: &str, prefix: &str, delimiter: char) -> Result<i64, MonitorStatServiceError> {
    let value = url
        .strip_prefix(prefix)
        .unwrap_or_default()
        .split(delimiter)
        .next()
        .unwrap_or_default();
    value
        .parse()
        .map_err(|_| MonitorStatServiceError::InvalidParameter {
            name: "id",
            value: value.to_owned(),
        })
}
