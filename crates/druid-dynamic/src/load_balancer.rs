//! 对应 Java 类：com.alibaba.druid.pool.ha.selector.DataSourceSelector
//!
//! 负载均衡器 trait，替代 Druid Java 的 DataSourceSelector。

use druid_core::Pool;
use std::sync::Arc;

/// 负载均衡器 trait。
///
/// 对应 Druid Java 的 `DataSourceSelector`，从多个从库中选择一个。
#[async_trait::async_trait]
pub trait LoadBalancer: Send + Sync {
    fn name(&self) -> &str;
    fn pick<'a>(&self, pools: &'a [Arc<dyn Pool>]) -> Option<&'a Arc<dyn Pool>>;
}

/// 轮询负载均衡器。
pub struct RoundRobinBalancer {
    index: std::sync::atomic::AtomicUsize,
}

impl RoundRobinBalancer {
    pub fn new() -> Self {
        Self { index: std::sync::atomic::AtomicUsize::new(0) }
    }
}

#[async_trait::async_trait]
impl LoadBalancer for RoundRobinBalancer {
    fn name(&self) -> &str { "round_robin" }
    fn pick<'a>(&self, pools: &'a [Arc<dyn Pool>]) -> Option<&'a Arc<dyn Pool>> {
        if pools.is_empty() { return None; }
        let idx = self.index.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % pools.len();
        pools.get(idx)
    }
}

/// 随机负载均衡器。
pub struct RandomBalancer;

#[async_trait::async_trait]
impl LoadBalancer for RandomBalancer {
    fn name(&self) -> &str { "random" }
    fn pick<'a>(&self, pools: &'a [Arc<dyn Pool>]) -> Option<&'a Arc<dyn Pool>> {
        if pools.is_empty() { return None; }
        let idx = fastrand::usize(0..pools.len());
        pools.get(idx)
    }
}
