use std::collections::HashMap;

use super::{DiscoveryClient, ServiceInstance};

/// 用于静态部署与测试的注册中心实现。
#[derive(Clone, Debug, Default)]
pub struct StaticDiscoveryClient {
    instances: HashMap<String, Vec<ServiceInstance>>,
}

impl StaticDiscoveryClient {
    /// 创建静态注册中心。
    #[must_use]
    pub fn new(instances: HashMap<String, Vec<ServiceInstance>>) -> Self {
        Self { instances }
    }
}

impl DiscoveryClient for StaticDiscoveryClient {
    fn services(&self) -> Vec<String> {
        self.instances.keys().cloned().collect()
    }

    fn instances(&self, service: &str) -> Vec<ServiceInstance> {
        self.instances.get(service).cloned().unwrap_or_default()
    }
}
