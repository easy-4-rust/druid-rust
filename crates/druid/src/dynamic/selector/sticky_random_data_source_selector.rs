//! 对应 Java 类：`com.alibaba.druid.pool.ha.selector.StickyRandomDataSourceSelector`。

use super::{DataSourceSelector, RandomDataSourceSelector, StickyDataSourceHolder};
use crate::core::Pool;
use crate::dynamic::HighAvailableDataSource;
use dashmap::DashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ExecutionKey {
    Task(tokio::task::Id),
    Thread(u64),
}

fn execution_key() -> ExecutionKey {
    if let Some(id) = tokio::task::try_id() {
        return ExecutionKey::Task(id);
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    ExecutionKey::Thread(hasher.finish())
}

/// 在过期时间内为同一 Rust task 固定随机数据源。
pub struct StickyRandomDataSourceSelector {
    random: RandomDataSourceSelector,
    holders: DashMap<ExecutionKey, StickyDataSourceHolder>,
    expire_seconds: AtomicI32,
}

impl StickyRandomDataSourceSelector {
    /// 创建 sticky random 选择器。
    #[must_use]
    pub fn new(data_source: &HighAvailableDataSource) -> Self {
        Self {
            random: RandomDataSourceSelector::new(data_source),
            holders: DashMap::new(),
            expire_seconds: AtomicI32::new(5),
        }
    }

    fn is_available(&self, holder: &StickyDataSourceHolder) -> bool {
        let Some(data_source) = holder.data_source() else {
            return false;
        };
        if !holder.is_valid()
            || self.random.contains_in_blacklist(data_source)
            || crate::dynamic::epoch_millis().saturating_sub(holder.retrieving_time_millis())
                > u64::try_from(self.expire_seconds.load(Ordering::Acquire))
                    .unwrap_or(0)
                    .saturating_mul(1_000)
        {
            return false;
        }
        let state = data_source.state();
        state.idle_count > 0 && state.active_count < state.max_open
    }

    /// 返回 sticky 过期秒数。
    #[must_use]
    pub fn expire_seconds(&self) -> i32 {
        self.expire_seconds.load(Ordering::Acquire)
    }

    /// 设置 sticky 过期秒数。
    pub fn set_expire_seconds(&self, value: i32) {
        self.expire_seconds.store(value, Ordering::Release);
    }

    /// 返回底层随机选择器。
    #[must_use]
    pub fn random_selector(&self) -> &RandomDataSourceSelector {
        &self.random
    }
}

impl DataSourceSelector for StickyRandomDataSourceSelector {
    fn get(&self) -> Option<Arc<dyn Pool>> {
        let key = execution_key();
        if let Some(holder) = self.holders.get(&key) {
            if self.is_available(&holder) {
                return holder.data_source().cloned();
            }
        }
        let data_source = self.random.get();
        self.holders.insert(
            key,
            StickyDataSourceHolder::with_data_source(data_source.clone()),
        );
        data_source
    }

    fn set_target(&self, _name: Option<String>) {}

    fn name(&self) -> &'static str {
        "stickyRandom"
    }

    fn init(&self) {
        self.random.init();
    }

    fn destroy(&self) {
        self.random.destroy();
        self.holders.clear();
    }
}
