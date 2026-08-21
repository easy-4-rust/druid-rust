//! 对应 Java 类：`com.alibaba.druid.pool.ha.node.PoolUpdater`。

use super::{NodeEvent, NodeEventTypeEnum};
use crate::dynamic::high_available_data_source::HighAvailableDataSourceInner;
use crate::dynamic::DataSourceCreator;
use dashmap::DashSet;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;

/// 接收节点事件并动态增删 HA 子池。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.node.PoolUpdater`。删除采用”先加入
/// blacklist，等 activeCount 归零再关闭并移除”的两阶段协议。
pub struct PoolUpdater {
    high_available_data_source: Weak<HighAvailableDataSourceInner>,
    data_source_creator: DataSourceCreator,
    nodes_to_delete: DashSet<String>,
    update_lock: Mutex<()>,
    interval_seconds: AtomicI32,
    allow_empty_pool: AtomicBool,
    initialized: AtomicBool,
    shutdown_tx: watch::Sender<bool>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl PoolUpdater {
    /// Java 默认清理周期，单位秒。
    pub const DEFAULT_INTERVAL: i32 = 60;

    /// 为指定 HA 数据源创建更新器。
    #[must_use]
    pub(crate) fn new(
        high_available_data_source: Weak<HighAvailableDataSourceInner>,
        data_source_creator: DataSourceCreator,
    ) -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            high_available_data_source,
            data_source_creator,
            nodes_to_delete: DashSet::new(),
            update_lock: Mutex::new(()),
            interval_seconds: AtomicI32::new(Self::DEFAULT_INTERVAL),
            allow_empty_pool: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            shutdown_tx,
            task: Mutex::new(None),
        }
    }

    /// 启动固定周期的延迟摘除任务；重复调用无副作用。
    pub async fn init(self: &Arc<Self>) {
        if self.initialized.swap(true, Ordering::AcqRel) {
            return;
        }
        let configured = self.interval_seconds.load(Ordering::Acquire);
        if configured < 10 {
            tracing::warn!(
                interval_seconds = configured,
                "HA pool purge 周期过小，请确认配置"
            );
        }
        let interval_seconds = if configured <= 0 {
            self.interval_seconds
                .store(Self::DEFAULT_INTERVAL, Ordering::Release);
            Self::DEFAULT_INTERVAL
        } else {
            configured
        };
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let updater = Arc::clone(self);
        *self.task.lock().await = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(
                u64::try_from(interval_seconds).unwrap_or(60),
            ));
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = interval.tick() => updater.remove_data_sources().await,
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        }));
    }

    /// 停止定时任务。
    pub async fn destroy(&self) {
        if !self.initialized.swap(false, Ordering::AcqRel) {
            return;
        }
        let _ = self.shutdown_tx.send(true);
        if let Some(task) = self.task.lock().await.take() {
            task.abort();
        }
    }

    /// 串行处理一批节点事件。
    pub async fn update(&self, events: Vec<NodeEvent>) {
        if events.is_empty() {
            return;
        }
        let _guard = self.update_lock.lock().await;
        tracing::info!(event_count = events.len(), "开始处理 HA 节点事件");
        for event in events {
            match event.event_type() {
                NodeEventTypeEnum::Add => self.add_node(&event).await,
                NodeEventTypeEnum::Delete => self.delete_node(&event),
            }
        }
    }

    /// 清理已列入删除集合且没有活动连接的节点。
    pub async fn remove_data_sources(&self) {
        if self.nodes_to_delete.is_empty() {
            return;
        }
        let _guard = self.update_lock.lock().await;
        let Some(data_source) = self.high_available_data_source.upgrade() else {
            return;
        };
        let nodes: Vec<String> = self
            .nodes_to_delete
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        for node_name in nodes {
            let Some(pool) = data_source
                .data_sources
                .get(&node_name)
                .map(|entry| Arc::clone(entry.value()))
            else {
                self.cancel_blacklist_node(&data_source, &node_name);
                continue;
            };
            match pool.close_for_removal_if_idle().await {
                Ok(false) => {
                    tracing::warn!(
                        node = %node_name,
                        active_count = pool.state().active_count,
                        "HA 节点仍有活动连接，下次再尝试摘除"
                    );
                    continue;
                }
                Ok(true) => {}
                Err(error) => {
                    // Java 关闭失败仍从 map 删除，避免坏节点永久占位。
                    tracing::error!(node = %node_name, error = %error, "关闭 HA 子池失败，继续摘除");
                }
            }
            data_source.data_sources.remove(&node_name);
            self.cancel_blacklist_node(&data_source, &node_name);
        }
    }

    async fn add_node(&self, event: &NodeEvent) {
        let node_name = event.node_name();
        if node_name.is_empty() {
            return;
        }
        let Some(data_source) = self.high_available_data_source.upgrade() else {
            return;
        };
        if data_source.data_sources.contains_key(node_name) {
            self.cancel_blacklist_node(&data_source, node_name);
            return;
        }
        match self
            .data_source_creator
            .create(
                node_name,
                event.url(),
                event.username(),
                event.password(),
                &data_source,
            )
            .await
        {
            Ok(pool) => {
                data_source.data_sources.insert(node_name.to_owned(), pool);
                tracing::info!(node = %node_name, url = ?event.url(), username = ?event.username(), "已创建 HA 节点");
            }
            Err(error) => {
                tracing::error!(node = %node_name, error = %error, "无法创建 HA 数据源，忽略节点");
            }
        }
    }

    fn delete_node(&self, event: &NodeEvent) {
        let node_name = event.node_name();
        let Some(data_source) = self.high_available_data_source.upgrade() else {
            return;
        };
        if node_name.is_empty() || !data_source.data_sources.contains_key(node_name) {
            return;
        }
        let available = data_source.available_data_source_map();
        if !self.allow_empty_pool.load(Ordering::Acquire)
            && available.len() == 1
            && available.contains_key(node_name)
        {
            tracing::warn!(node = %node_name, "该节点是最后一个可用数据源，不执行摘除");
            return;
        }
        self.nodes_to_delete.insert(node_name.to_owned());
        data_source.blacklist.insert(node_name.to_owned());
    }

    fn cancel_blacklist_node(&self, data_source: &HighAvailableDataSourceInner, node_name: &str) {
        self.nodes_to_delete.remove(node_name);
        data_source.blacklist.remove(node_name);
    }

    /// 返回待延迟摘除节点快照。
    #[must_use]
    pub fn nodes_to_delete(&self) -> Vec<String> {
        self.nodes_to_delete
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// 设置清理周期秒数。
    pub fn set_interval_seconds(&self, interval_seconds: i32) {
        self.interval_seconds
            .store(interval_seconds, Ordering::Release);
    }

    /// 设置是否允许动态更新后池为空。
    pub fn set_allow_empty_pool(&self, allow_empty_pool: bool) {
        self.allow_empty_pool
            .store(allow_empty_pool, Ordering::Release);
    }
}
