use crate::util::HttpError;

/// 管理端聚合服务错误。
#[derive(Debug, thiserror::Error)]
pub enum MonitorStatServiceError {
    /// 注册中心返回缺少实例标识。
    #[error("service {service} instance has neither instanceId nor nacos.instanceId")]
    MissingInstanceId { service: String },
    /// 请求引用了未知节点。
    #[error("unknown serviceId: {0}")]
    UnknownServiceId(String),
    /// 下游 HTTP 或 JSON 错误。
    #[error(transparent)]
    Http(#[from] HttpError),
    /// JSON 序列化错误。
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Kubernetes 发现失败。
    #[error("kubernetes discovery failed: {0}")]
    Kubernetes(String),
    /// Java URL 协议参数无效。
    #[error("invalid parameter {name}: {value}")]
    InvalidParameter { name: &'static str, value: String },
}
