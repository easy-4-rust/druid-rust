use super::ServiceInstance;

/// Spring Cloud `DiscoveryClient` 的 Rust SPI。
pub trait DiscoveryClient: Send + Sync {
    /// 返回所有已发现服务名。
    fn services(&self) -> Vec<String>;

    /// 返回指定服务的全部实例。
    fn instances(&self, service: &str) -> Vec<ServiceInstance>;
}
