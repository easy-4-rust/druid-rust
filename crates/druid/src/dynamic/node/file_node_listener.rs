//! 对应 Java 类：`com.alibaba.druid.pool.ha.node.FileNodeListener`。

use super::node_listener::NodeListenerState;
use super::{NodeEvent, NodeListener, PoolUpdater};
use crate::core::DruidError;
use crate::dynamic::PropertiesUtils;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;

/// 定时监控 Java properties 文件的节点监听器。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.node.FileNodeListener`。
pub struct FileNodeListener {
    state: NodeListenerState,
    file: RwLock<PathBuf>,
    interval_seconds: std::sync::atomic::AtomicI32,
    refresh_lock: Mutex<()>,
    shutdown_tx: watch::Sender<bool>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl FileNodeListener {
    /// 创建文件监听器；默认周期为 60 秒。
    #[must_use]
    pub fn new(file: impl Into<PathBuf>) -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            state: NodeListenerState::default(),
            file: RwLock::new(file.into()),
            interval_seconds: std::sync::atomic::AtomicI32::new(60),
            refresh_lock: Mutex::new(()),
            shutdown_tx,
            task: Mutex::new(None),
        }
    }

    /// 设置 properties 键前缀。
    pub fn set_prefix(&self, prefix: impl Into<String>) {
        self.state.set_prefix(prefix);
    }

    /// 返回 properties 键前缀。
    #[must_use]
    pub fn prefix(&self) -> String {
        self.state.prefix()
    }

    /// 设置监控文件。
    pub fn set_file(&self, file: impl Into<PathBuf>) {
        *self.file.write() = file.into();
    }

    /// 返回监控文件。
    #[must_use]
    pub fn file(&self) -> PathBuf {
        self.file.read().clone()
    }

    /// 设置刷新周期秒数；非正数在 init 时恢复为 60。
    pub fn set_interval_seconds(&self, interval_seconds: i32) {
        self.interval_seconds
            .store(interval_seconds, std::sync::atomic::Ordering::Release);
    }

    /// 返回刷新周期秒数。
    #[must_use]
    pub fn interval_seconds(&self) -> i32 {
        self.interval_seconds
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn normalized_interval_seconds(&self) -> u64 {
        let configured = self.interval_seconds();
        if configured <= 0 {
            self.interval_seconds
                .store(60, std::sync::atomic::Ordering::Release);
            60
        } else {
            u64::try_from(configured).unwrap_or(60)
        }
    }

    fn filtered_properties(&self) -> HashMap<String, String> {
        let original = PropertiesUtils::load_properties(Some(&self.file()));
        let names = PropertiesUtils::load_name_list(&original, Some(&self.state.prefix()));
        let mut properties = HashMap::new();
        for name in names {
            let url_key = format!("{name}.url");
            let username_key = format!("{name}.username");
            let password_key = format!("{name}.password");
            let url = original.get(&url_key).cloned();
            if url.as_deref().is_none_or(str::is_empty) {
                tracing::warn!(node = %name, "{url_key} 为空，忽略该节点");
                continue;
            }
            properties.insert(url_key, url.unwrap_or_default());
            for key in [username_key, password_key] {
                if let Some(value) = original.get(&key).filter(|value| !value.is_empty()) {
                    properties.insert(key, value.clone());
                } else {
                    tracing::debug!(node = %name, property = %key, "HA 节点可选属性为空");
                }
            }
        }
        properties
    }
}

#[async_trait::async_trait]
impl NodeListener for FileNodeListener {
    async fn refresh(&self) -> Vec<NodeEvent> {
        let properties = self.filtered_properties();
        let events =
            NodeEvent::get_events_by_diff_properties(&self.state.properties(), &properties);
        if !events.is_empty() {
            tracing::info!(difference_count = events.len(), "检测到 HA 节点变化");
            self.state.set_properties(properties);
        }
        events
    }

    async fn init(self: Arc<Self>) -> Result<(), DruidError> {
        self.state.init().await?;
        let interval_seconds = self.normalized_interval_seconds();
        let mut task = self.task.lock().await;
        if let Some(old) = task.take() {
            old.abort();
        }
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let listener = Arc::clone(&self);
        *task = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let Ok(_guard) = listener.refresh_lock.try_lock() else {
                            tracing::info!("未获取到 FileNodeListener 锁，本轮刷新跳过");
                            continue;
                        };
                        listener.update().await;
                    }
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        }));
        Ok(())
    }

    async fn update(&self) {
        let events = self.refresh().await;
        self.state.update(events).await;
    }

    async fn destroy(&self) {
        let _ = self.shutdown_tx.send(true);
        if let Some(task) = self.task.lock().await.take() {
            task.abort();
        }
    }

    fn set_observer(&self, observer: Arc<PoolUpdater>) {
        self.state.set_observer(observer);
    }

    fn last_update_time_millis(&self) -> u64 {
        self.state.last_update_time_millis()
    }
}
