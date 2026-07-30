//! 对应 Java 类：`com.alibaba.druid.pool.ha.node.ZookeeperNodeListener`。

use super::node_listener::NodeListenerState;
use super::{NodeEvent, NodeEventTypeEnum, NodeListener, PoolUpdater};
use crate::core::DruidError;
use crate::dynamic::PropertiesUtils;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use zookeeper_client::{
    AddWatchMode, Client, Error as ZookeeperError, EventType, SessionState, WatchedEvent,
};

/// 监听 ZooKeeper 路径直接子节点的 HA 节点监听器。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.node.ZookeeperNodeListener`。Curator
/// PathChildrenCache 映射为 persistent recursive watcher 加本地直接子节点快照；
/// CHILD_UPDATED 仍被忽略，重连后完整重建快照。
pub struct ZookeeperNodeListener {
    state: NodeListenerState,
    zk_connect_string: RwLock<Option<String>>,
    path: RwLock<String>,
    url_template: RwLock<Option<String>>,
    client: RwLock<Option<Arc<Client>>>,
    private_zk_client: AtomicBool,
    cache: RwLock<HashMap<String, Vec<u8>>>,
    event_lock: Mutex<()>,
    shutdown_tx: watch::Sender<bool>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl Default for ZookeeperNodeListener {
    fn default() -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            state: NodeListenerState::default(),
            zk_connect_string: RwLock::new(None),
            path: RwLock::new("/ha-druid-datasources".to_owned()),
            url_template: RwLock::new(None),
            client: RwLock::new(None),
            private_zk_client: AtomicBool::new(false),
            cache: RwLock::new(HashMap::new()),
            event_lock: Mutex::new(()),
            shutdown_tx,
            task: Mutex::new(None),
        }
    }
}

impl ZookeeperNodeListener {
    /// 创建未初始化的 ZooKeeper listener。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 properties 前缀。
    pub fn set_prefix(&self, prefix: impl Into<String>) {
        self.state.set_prefix(prefix);
    }

    /// 设置 ZooKeeper ensemble 连接串。
    pub fn set_zk_connect_string(&self, connect_string: impl Into<String>) {
        *self.zk_connect_string.write() = Some(connect_string.into());
    }

    /// 设置监听路径。
    pub fn set_path(&self, path: impl Into<String>) {
        *self.path.write() = path.into();
    }

    /// 设置 URL 模板。
    pub fn set_url_template(&self, url_template: impl Into<String>) {
        *self.url_template.write() = Some(url_template.into());
    }

    /// 注入外部 ZooKeeper client；listener 不接管其生命周期。
    pub fn set_client(&self, client: Arc<Client>) {
        *self.client.write() = Some(client);
        self.private_zk_client.store(false, Ordering::Release);
    }

    /// 返回 ZooKeeper client。
    #[must_use]
    pub fn client(&self) -> Option<Arc<Client>> {
        self.client.read().clone()
    }

    fn check_parameters(&self) -> Result<(), DruidError> {
        if self.client.read().is_none()
            && self
                .zk_connect_string
                .read()
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(DruidError::Other(
                "ZK Client is NULL, Please set the zkConnectString.".to_owned(),
            ));
        }
        if self.path.read().is_empty() {
            return Err(DruidError::Other(
                "Please set the ZooKeeper node path.".to_owned(),
            ));
        }
        if self
            .url_template
            .read()
            .as_deref()
            .is_none_or(str::is_empty)
        {
            return Err(DruidError::Other("Please set the urlTemplate.".to_owned()));
        }
        Ok(())
    }

    async fn ensure_client(&self) -> Result<Arc<Client>, DruidError> {
        if let Some(client) = self.client.read().clone() {
            return Ok(client);
        }
        let connect_string = self.zk_connect_string.read().clone().ok_or_else(|| {
            DruidError::Other("ZK Client is NULL, Please set the zkConnectString.".to_owned())
        })?;
        let client = Arc::new(
            Client::connect(&connect_string)
                .await
                .map_err(Self::client_error)?,
        );
        *self.client.write() = Some(Arc::clone(&client));
        self.private_zk_client.store(true, Ordering::Release);
        Ok(client)
    }

    async fn load_cache(
        client: &Client,
        path: &str,
    ) -> Result<HashMap<String, Vec<u8>>, ZookeeperError> {
        let children = match client.get_children(path).await {
            Ok((children, _)) => children,
            Err(ZookeeperError::NoNode) => return Ok(HashMap::new()),
            Err(error) => return Err(error),
        };
        let mut cache = HashMap::new();
        for child in children {
            let child_path = format!("{}/{}", path.trim_end_matches('/'), child);
            match client.get_data(&child_path).await {
                Ok((data, _)) => {
                    cache.insert(child, data);
                }
                Err(ZookeeperError::NoNode) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(cache)
    }

    fn properties_from_cache(&self) -> HashMap<String, String> {
        let mut properties = HashMap::new();
        for (node_name, data) in self.cache.read().iter() {
            properties.extend(self.properties_from_child_data(node_name, data));
        }
        properties
    }

    fn properties_from_child_data(&self, node_name: &str, data: &[u8]) -> HashMap<String, String> {
        let full = match java_properties::read(Cursor::new(data)) {
            Ok(properties) => properties,
            Err(error) => {
                tracing::error!(node = %node_name, error = %error, "无法解析 ZooKeeper 节点 properties");
                HashMap::new()
            }
        };
        let prefix = self.state.prefix();
        let filtered = PropertiesUtils::filter_prefix(&full, Some(&prefix));
        let mut properties = HashMap::new();
        for (name, value) in filtered {
            // 保留 Java replaceFirst(prefix, prefix + "." + nodeName) 的首次替换。
            let mapped_name = if prefix.is_empty() {
                format!(".{node_name}{name}")
            } else {
                name.replacen(&prefix, &format!("{prefix}.{node_name}"), 1)
            };
            properties.insert(mapped_name, value);
        }
        let url_key = format!("{prefix}.{node_name}.url");
        if !properties.contains_key(&url_key) {
            properties.insert(url_key, self.format_url(&full));
        }
        properties
    }

    fn format_url(&self, properties: &HashMap<String, String>) -> String {
        let mut url = self.url_template.read().clone().unwrap_or_default();
        let prefix = self.state.prefix();
        for name in ["host", "port", "database"] {
            let key = format!("{prefix}.{name}");
            if let Some(value) = properties.get(&key) {
                for placeholder in [
                    format!("${{{name}}}"),
                    format!("#{{{name}}}"),
                    format!("#{name}#"),
                ] {
                    url = url.replace(&placeholder, value);
                }
            }
        }
        url
    }

    fn direct_child_name(path: &str, event_path: &str) -> Option<String> {
        let prefix = format!("{}/", path.trim_end_matches('/'));
        let child = event_path.strip_prefix(&prefix)?;
        (!child.is_empty() && !child.contains('/')).then(|| child.to_owned())
    }

    async fn process_event(&self, event: WatchedEvent) {
        let _guard = self.event_lock.lock().await;
        let path = self.path.read().clone();
        match event.event_type {
            EventType::NodeCreated => {
                let Some(node_name) = Self::direct_child_name(&path, &event.path) else {
                    return;
                };
                let Some(client) = self.client.read().clone() else {
                    return;
                };
                match client.get_data(&event.path).await {
                    Ok((data, _)) => {
                        self.cache.write().insert(node_name.clone(), data.clone());
                        self.update_single_node(&node_name, &data, NodeEventTypeEnum::Add)
                            .await;
                    }
                    Err(error) => {
                        tracing::error!(path = %event.path, error = %error, "读取新增 ZooKeeper 节点失败");
                    }
                }
            }
            EventType::NodeDeleted => {
                let Some(node_name) = Self::direct_child_name(&path, &event.path) else {
                    return;
                };
                if let Some(data) = self.cache.write().remove(&node_name) {
                    self.update_single_node(&node_name, &data, NodeEventTypeEnum::Delete)
                        .await;
                }
            }
            EventType::NodeDataChanged | EventType::NodeChildrenChanged | EventType::Session => {
                // Java PathChildrenCache 明确忽略 CHILD_UPDATED/普通状态事件。
            }
        }
    }

    async fn update_single_node(
        &self,
        node_name: &str,
        data: &[u8],
        event_type: NodeEventTypeEnum,
    ) {
        let properties = self.properties_from_child_data(node_name, data);
        let names = [format!("{}.{}", self.state.prefix(), node_name)];
        let events = NodeEvent::generate_events(&properties, &names, event_type);
        if events.is_empty() {
            return;
        }
        let mut current = self.state.properties();
        match event_type {
            NodeEventTypeEnum::Add => current.extend(properties),
            NodeEventTypeEnum::Delete => {
                for name in properties.keys() {
                    current.remove(name);
                }
            }
        }
        self.state.set_properties(current);
        self.state.update(events).await;
    }

    async fn refresh_all_nodes(&self) {
        let Some(client) = self.client.read().clone() else {
            return;
        };
        let path = self.path.read().clone();
        match Self::load_cache(&client, &path).await {
            Ok(cache) => {
                *self.cache.write() = cache;
                let properties = self.properties_from_cache();
                let events =
                    NodeEvent::get_events_by_diff_properties(&self.state.properties(), &properties);
                if !events.is_empty() {
                    self.state.set_properties(properties);
                    self.state.update(events).await;
                }
            }
            Err(error) => {
                tracing::error!(path = %path, error = %error, "重连后无法刷新 ZooKeeper 节点");
            }
        }
    }

    fn client_error(error: ZookeeperError) -> DruidError {
        DruidError::Other(format!("ZooKeeper: {error}"))
    }
}

#[async_trait::async_trait]
impl NodeListener for ZookeeperNodeListener {
    async fn refresh(&self) -> Vec<NodeEvent> {
        let properties = self.properties_from_cache();
        let events =
            NodeEvent::get_events_by_diff_properties(&self.state.properties(), &properties);
        if !events.is_empty() {
            self.state.set_properties(properties);
        }
        events
    }

    async fn init(self: Arc<Self>) -> Result<(), DruidError> {
        self.check_parameters()?;
        self.state.init().await?;
        let client = self.ensure_client().await?;
        let path = self.path.read().clone();
        match Self::load_cache(&client, &path).await {
            Ok(cache) => *self.cache.write() = cache,
            Err(error) => {
                tracing::error!(path = %path, error = %error, "无法构建 ZooKeeper 初始子节点缓存");
            }
        }
        let mut watcher = client
            .watch(&path, AddWatchMode::PersistentRecursive)
            .await
            .map_err(Self::client_error)?;
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let listener = Arc::clone(&self);
        *self.task.lock().await = Some(tokio::spawn(async move {
            let mut disconnected = false;
            loop {
                tokio::select! {
                    event = watcher.changed() => {
                        if event.event_type == EventType::Session {
                            match event.session_state {
                                SessionState::Disconnected => disconnected = true,
                                SessionState::SyncConnected | SessionState::ConnectedReadOnly
                                    if disconnected =>
                                {
                                    disconnected = false;
                                    listener.refresh_all_nodes().await;
                                }
                                state if state.is_terminated() => break,
                                _ => {}
                            }
                        } else {
                            listener.process_event(event).await;
                        }
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
        if self.private_zk_client.swap(false, Ordering::AcqRel) {
            self.client.write().take();
        }
    }

    fn set_observer(&self, observer: Arc<PoolUpdater>) {
        self.state.set_observer(observer);
    }

    fn last_update_time_millis(&self) -> u64 {
        self.state.last_update_time_millis()
    }
}
