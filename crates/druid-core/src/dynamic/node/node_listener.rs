//! 对应 Java 抽象类：`com.alibaba.druid.pool.ha.node.NodeListener`。

use super::{NodeEvent, PoolUpdater};
use crate::core::DruidError;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 节点监听器共享状态。
///
/// 这是 Java 抽象类字段在 Rust trait 组合模式下的内部承载体，不是额外的迁移
/// 对象。
#[derive(Default)]
pub(crate) struct NodeListenerState {
    prefix: RwLock<String>,
    properties: RwLock<HashMap<String, String>>,
    last_update_time_millis: AtomicU64,
    observer: RwLock<Option<Arc<PoolUpdater>>>,
}

impl NodeListenerState {
    pub(crate) fn prefix(&self) -> String {
        self.prefix.read().clone()
    }

    pub(crate) fn set_prefix(&self, prefix: impl Into<String>) {
        *self.prefix.write() = prefix.into();
    }

    pub(crate) fn properties(&self) -> HashMap<String, String> {
        self.properties.read().clone()
    }

    pub(crate) fn set_properties(&self, properties: HashMap<String, String>) {
        *self.properties.write() = properties;
    }

    pub(crate) fn last_update_time_millis(&self) -> u64 {
        self.last_update_time_millis.load(Ordering::Acquire)
    }

    pub(crate) fn set_observer(&self, observer: Arc<PoolUpdater>) {
        *self.observer.write() = Some(observer);
    }

    pub(crate) fn observer(&self) -> Option<Arc<PoolUpdater>> {
        self.observer.read().clone()
    }

    pub(crate) async fn init(&self) -> Result<(), DruidError> {
        if self.observer.read().is_none() {
            return Err(DruidError::Other(
                "No Observer(such as PoolUpdater) specified.".to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn update(&self, events: Vec<NodeEvent>) {
        if events.is_empty() {
            return;
        }
        self.last_update_time_millis
            .store(crate::dynamic::epoch_millis(), Ordering::Release);
        if let Some(observer) = self.observer() {
            observer.update(events).await;
        }
    }
}

/// 监控数据源节点变化的异步监听器协议。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.node.NodeListener`。Java
/// `Observable/Observer` 被显式、类型安全的 [`PoolUpdater`] 观察者替代，事件
/// 通知条件与更新时间语义保持不变。
#[async_trait::async_trait]
pub trait NodeListener: Send + Sync {
    /// 读取外部节点状态并返回新增/删除事件。
    async fn refresh(&self) -> Vec<NodeEvent>;

    /// 初始化监听器和后台资源。
    async fn init(self: Arc<Self>) -> Result<(), DruidError>;

    /// 立即刷新并通知 PoolUpdater。
    async fn update(&self);

    /// 停止监听器后台资源。
    async fn destroy(&self);

    /// 设置唯一 PoolUpdater 观察者。
    fn set_observer(&self, observer: Arc<PoolUpdater>);

    /// 返回最近实际发布非空事件的时间。
    fn last_update_time_millis(&self) -> u64;
}
