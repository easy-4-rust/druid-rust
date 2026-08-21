//! 对应 Java 类：`com.alibaba.druid.pool.ha.selector.NamedDataSourceSelector`。

use super::DataSourceSelector;
use crate::core::Pool;
use crate::dynamic::high_available_data_source::HighAvailableDataSourceInner;
use crate::dynamic::HighAvailableDataSource;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

/// Java `ThreadLocal` 在 Rust async 中的执行上下文键。
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

/// 按执行上下文目标名称选择数据源。
pub struct NamedDataSourceSelector {
    high_available_data_source: Weak<HighAvailableDataSourceInner>,
    targets: DashMap<ExecutionKey, String>,
    default_name: RwLock<String>,
}

impl NamedDataSourceSelector {
    pub const DEFAULT_NAME: &'static str = "default";

    /// 创建绑定到 HA 数据源的命名选择器。
    #[must_use]
    pub fn new(data_source: &HighAvailableDataSource) -> Self {
        Self {
            high_available_data_source: data_source.weak_inner(),
            targets: DashMap::new(),
            default_name: RwLock::new(Self::DEFAULT_NAME.to_owned()),
        }
    }

    /// 返回当前执行上下文目标。
    #[must_use]
    pub fn target(&self) -> Option<String> {
        self.targets
            .get(&execution_key())
            .map(|value| value.clone())
    }

    /// 清除当前执行上下文目标。
    pub fn reset_data_source_name(&self) {
        self.targets.remove(&execution_key());
    }

    /// 返回默认节点名称。
    #[must_use]
    pub fn default_name(&self) -> String {
        self.default_name.read().clone()
    }

    /// 设置默认节点名称。
    pub fn set_default_name(&self, default_name: impl Into<String>) {
        *self.default_name.write() = default_name.into();
    }
}

impl DataSourceSelector for NamedDataSourceSelector {
    fn get(&self) -> Option<Arc<dyn Pool>> {
        let data_source = self.high_available_data_source.upgrade()?;
        let available = data_source.available_data_source_map();
        if available.is_empty() {
            return None;
        }
        if available.len() == 1 {
            return available.into_values().next();
        }
        match self.target() {
            Some(name) => available.get(&name).cloned(),
            None => available.get(&self.default_name()).cloned(),
        }
    }

    fn set_target(&self, name: Option<String>) {
        match name {
            Some(name) => {
                self.targets.insert(execution_key(), name);
            }
            None => {
                self.targets.remove(&execution_key());
            }
        }
    }

    fn name(&self) -> &'static str {
        "byName"
    }

    fn init(&self) {}

    fn destroy(&self) {
        self.targets.clear();
    }
}
