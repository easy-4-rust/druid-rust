use crate::model::ServiceNode;

/// Kubernetes SDK/控制面适配器的最小 SPI。
#[async_trait::async_trait]
pub trait K8sDiscoveryProvider: Send + Sync {
    /// 从指定 kubeconfig 与 namespace 返回目标服务的 Pod 节点。
    async fn service_nodes(
        &self,
        service_names: &[String],
        kube_config_file_path: &str,
        namespace: &str,
    ) -> Result<Vec<ServiceNode>, Box<dyn std::error::Error + Send + Sync>>;
}
