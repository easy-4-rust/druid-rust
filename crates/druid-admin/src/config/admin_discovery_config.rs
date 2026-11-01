//! Admin 数据来源发现配置。
//!
//! Rust Admin 的数据可以来自两条路：gRPC 推送（仓库视图）或轮询被监控
//! 节点的 `/druid/*.json`（Java Admin 模型）。本配置决定用哪条，
//! 并把静态节点地址 / 注册中心应用名翻译成 `ManagementSource`。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::config::{AdminConfigError, MonitorProperties};
use crate::repository::MetricsRepository;
use crate::service::{
    ManagementSource, MonitorStatService, ServiceInstance, StaticDiscoveryClient,
};
use crate::util::ReqwestHttpClient;

/// Admin 数据来源模式。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum AdminDiscoveryMode {
    /// 只用 gRPC 推送的仓库视图（standalone 默认）。
    #[default]
    Push,
    /// 只轮询被监控节点的 `/druid/*.json`（Java Admin 模型）。
    Poll,
    /// 两者都用：列表类先查远端，远端无数据时回落仓库。
    Both,
}

/// Admin 数据来源发现配置。
#[derive(Clone, Debug)]
pub struct AdminDiscoveryConfig {
    /// 数据来源模式。
    pub mode: AdminDiscoveryMode,
    /// 注册中心应用名；静态端点归入第一个应用名下。
    pub applications: Vec<String>,
    /// 静态节点地址，形如 `http://10.0.0.7:8080`。
    ///
    /// Kubernetes 发现留作后续；本切片只支持静态节点。
    pub endpoints: Vec<String>,
}

impl AdminDiscoveryConfig {
    /// 默认配置：只用推送。
    #[must_use]
    pub fn push_only() -> Self {
        Self {
            mode: AdminDiscoveryMode::Push,
            applications: Vec::new(),
            endpoints: Vec::new(),
        }
    }

    /// 从环境变量读取发现配置。
    pub fn from_env() -> Result<Self, AdminConfigError> {
        let mode = match std::env::var("DRUID_ADMIN_DISCOVERY_MODE").ok().as_deref() {
            None | Some("") | Some("push") => AdminDiscoveryMode::Push,
            Some("poll") => AdminDiscoveryMode::Poll,
            Some("both") => AdminDiscoveryMode::Both,
            Some(_) => return Err(AdminConfigError::UnsupportedAuthMode),
        };
        let applications = split_env("DRUID_ADMIN_APPLICATIONS");
        let endpoints = split_env("DRUID_ADMIN_ENDPOINTS");
        if mode != AdminDiscoveryMode::Push && endpoints.is_empty() {
            // 轮询模式至少需要配置一个被监控节点。
            return Err(AdminConfigError::InvalidDiscovery);
        }
        Ok(Self {
            mode,
            applications,
            endpoints,
        })
    }

    /// 返回配置的数据来源模式。
    #[must_use]
    pub const fn mode(&self) -> &AdminDiscoveryMode {
        &self.mode
    }

    /// 构造统一的来源。
    ///
    /// `Poll` 与 `Both` 需要能构造远端服务；`Push` 直接返回仓库视图。
    pub fn build_management_source(
        &self,
        repository: MetricsRepository,
    ) -> Result<ManagementSource, AdminConfigError> {
        match self.mode {
            AdminDiscoveryMode::Push => Ok(ManagementSource::repository(repository)),
            AdminDiscoveryMode::Poll | AdminDiscoveryMode::Both => {
                let remote = self.build_remote_service()?;
                Ok(if self.mode == AdminDiscoveryMode::Poll {
                    ManagementSource::remote(remote)
                } else {
                    ManagementSource::both(remote, repository)
                })
            }
        }
    }

    /// 构造 Java 模型下的远端聚合服务。
    fn build_remote_service(&self) -> Result<Arc<MonitorStatService>, AdminConfigError> {
        let http_client = Arc::new(
            ReqwestHttpClient::new(Duration::from_secs(5), Duration::from_secs(10))
                .map_err(|_| AdminConfigError::HttpClientUnavailable)?,
        );
        let monitor_properties = Arc::new(MonitorProperties {
            applications: self.applications.clone(),
            context_path: Some("/druid".to_owned()),
            ..Default::default()
        });
        let discovery_client = Arc::new(StaticDiscoveryClient::new(self.static_instances()));
        Ok(Arc::new(MonitorStatService::new(
            discovery_client,
            monitor_properties,
            None,
            http_client,
        )))
    }

    /// 把静态节点地址映射成注册中心实例。
    fn static_instances(&self) -> HashMap<String, Vec<ServiceInstance>> {
        let mut instances: HashMap<String, Vec<ServiceInstance>> = HashMap::new();
        let service_name = self
            .applications
            .first()
            .cloned()
            .unwrap_or_else(|| "default".to_owned());
        let nodes: Vec<ServiceInstance> = self
            .endpoints
            .iter()
            .enumerate()
            .filter_map(|(index, endpoint)| parse_endpoint(endpoint, index, &service_name))
            .collect();
        if !nodes.is_empty() {
            instances.insert(service_name, nodes);
        }
        instances
    }
}

/// 按逗号拆分一个环境变量。
fn split_env(name: &str) -> Vec<String> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// 解析一个 `http://host:port` 形式的节点地址。
fn parse_endpoint(endpoint: &str, index: usize, service_name: &str) -> Option<ServiceInstance> {
    let without_scheme = endpoint
        .strip_prefix("http://")
        .or_else(|| endpoint.strip_prefix("https://"))?;
    let (host, port) = without_scheme.split_once(':')?;
    Some(ServiceInstance {
        instance_id: Some(format!("{service_name}-{index}")),
        service_id: service_name.to_owned(),
        host: host.to_owned(),
        port: port.parse().ok()?,
        metadata: HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_endpoint_extracts_host_and_port() {
        let node = parse_endpoint("http://10.0.0.7:8080", 0, "orders").unwrap();
        assert_eq!(node.host, "10.0.0.7");
        assert_eq!(node.port, 8080);
        assert_eq!(node.service_id, "orders");
    }

    #[test]
    fn static_instances_groups_under_first_application() {
        let config = AdminDiscoveryConfig {
            mode: AdminDiscoveryMode::Poll,
            applications: vec!["orders".to_owned()],
            endpoints: vec![
                "http://10.0.0.7:8080".to_owned(),
                "http://10.0.0.8:8080".to_owned(),
            ],
        };
        let instances = config.static_instances();
        assert_eq!(instances.get("orders").map(Vec::len), Some(2));
    }
}
