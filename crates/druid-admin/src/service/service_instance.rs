use std::collections::HashMap;

/// Rust 注册中心适配器使用的实例快照。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceInstance {
    /// 可空的原生实例标识。
    pub instance_id: Option<String>,
    /// 服务标识。
    pub service_id: String,
    /// 主机名或 IP。
    pub host: String,
    /// 管理端口。
    pub port: u16,
    /// 注册中心元数据。
    pub metadata: HashMap<String, String>,
}
