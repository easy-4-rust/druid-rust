use std::collections::HashMap;
use std::sync::Arc;

use crate::model::ServiceNode;

use super::K8sDiscoveryProvider;

/// Kubernetes 节点发现与 Java 去重规则。
///
/// 对应 Java: `com.alibaba.druid.admin.service.K8sDiscoveryClient`。
pub struct K8sDiscoveryClient {
    provider: Arc<dyn K8sDiscoveryProvider>,
}

impl K8sDiscoveryClient {
    /// 创建 Kubernetes 发现客户端。
    #[must_use]
    pub fn new(provider: Arc<dyn K8sDiscoveryProvider>) -> Self {
        Self { provider }
    }

    /// 发现节点并按 `serviceName-address-port` 去重，后出现者覆盖先出现者。
    pub async fn get_k8s_pods_info(
        &self,
        service_names: &[String],
        kube_config_file_path: &str,
        namespace: &str,
    ) -> Result<HashMap<String, ServiceNode>, Box<dyn std::error::Error + Send + Sync>> {
        let nodes = self
            .provider
            .service_nodes(service_names, kube_config_file_path, namespace)
            .await?;
        let mut result = HashMap::with_capacity(nodes.len());
        for node in nodes {
            tracing::info!(node = ?node, "discovered kubernetes druid node");
            result.insert(node.map_key(), node);
        }
        Ok(result)
    }
}
