/// `kube` 生产发现适配器错误。
#[derive(Debug, thiserror::Error)]
pub enum KubeRsDiscoveryError {
    /// kubeconfig 读取或解析失败。
    #[error(transparent)]
    Config(#[from] kube::config::KubeconfigError),
    /// Kubernetes API 请求失败。
    #[error(transparent)]
    Api(#[from] kube::Error),
    /// 阻塞 kubeconfig 读取任务失败。
    #[error("kubeconfig reader task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    /// Java 依赖的具名 Service port 不存在。
    #[error("service {service_name} has no port named {service_name}")]
    MissingServicePort { service_name: String },
    /// Pod 元数据缺失。
    #[error("pod for service {service_name} has no {field}")]
    MissingPodField {
        service_name: String,
        field: &'static str,
    },
    /// Kubernetes 端口超出 ServiceNode 表达范围。
    #[error("service {service_name} has invalid port {port}")]
    InvalidPort { service_name: String, port: i32 },
}
