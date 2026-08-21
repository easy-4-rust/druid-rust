//! 对应 Java 类：`com.alibaba.druid.pool.ha.node.ZookeeperNodeRegister`。

use super::ZookeeperNodeInfo;
use crate::core::DruidError;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use zookeeper_client::{Acls, Client, CreateMode, Error as ZookeeperError};

/// 将数据库端点注册为 `ZooKeeper` 临时成员节点。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.node.ZookeeperNodeRegister`。一个实例
/// 同时只允许注册一个 member；使用外部 client 时 destroy 不接管其生命周期。
pub struct ZookeeperNodeRegister {
    zk_connect_string: RwLock<Option<String>>,
    path: RwLock<String>,
    client: RwLock<Option<Arc<Client>>>,
    member_path: RwLock<Option<String>>,
    private_zk_client: AtomicBool,
    lock: Mutex<()>,
}

impl Default for ZookeeperNodeRegister {
    fn default() -> Self {
        Self {
            zk_connect_string: RwLock::new(None),
            path: RwLock::new("/ha-druid-datasources".to_owned()),
            client: RwLock::new(None),
            member_path: RwLock::new(None),
            private_zk_client: AtomicBool::new(false),
            lock: Mutex::new(()),
        }
    }
}

impl ZookeeperNodeRegister {
    /// 创建未初始化的注册器。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 没有外部 client 时建立私有异步 `ZooKeeper` client。
    pub async fn init(&self) -> Result<(), DruidError> {
        if self.client.read().is_some() {
            return Ok(());
        }
        let connect_string =
            self.zk_connect_string.read().clone().ok_or_else(|| {
                DruidError::InvalidArgument("zkConnectString is required".to_owned())
            })?;
        let client = Client::connect(&connect_string)
            .await
            .map_err(Self::client_error)?;
        *self.client.write() = Some(Arc::new(client));
        self.private_zk_client.store(true, Ordering::Release);
        Ok(())
    }

    /// 注册唯一临时成员；空 payload 或已有 member 时返回 false。
    pub async fn register(
        &self,
        node_id: &str,
        payload: &[ZookeeperNodeInfo],
    ) -> Result<bool, DruidError> {
        if payload.is_empty() {
            return Ok(false);
        }
        let _guard = self.lock.lock().await;
        if self.member_path.read().is_some() {
            tracing::warn!("ZooKeeper GroupMember 已注册，请先 deregister");
            return Ok(false);
        }
        let client = self.client.read().clone().ok_or_else(|| {
            DruidError::Other("ZookeeperNodeRegister has not been initialized".to_owned())
        })?;
        let path = self.path.read().clone();
        client
            .mkdir(&path, &CreateMode::Persistent.with_acls(Acls::anyone_all()))
            .await
            .map_err(Self::client_error)?;
        let member_path = format!("{}/{}", path.trim_end_matches('/'), node_id);
        let payload = Self::properties_bytes(payload)?;
        client
            .create(
                &member_path,
                &payload,
                &CreateMode::Ephemeral.with_acls(Acls::anyone_all()),
            )
            .await
            .map_err(Self::client_error)?;
        *self.member_path.write() = Some(member_path.clone());
        tracing::info!(node = %node_id, path = %path, "已注册 ZooKeeper HA 节点");
        Ok(true)
    }

    /// 删除当前临时成员；私有 client 同时释放，外部 client 保留。
    pub async fn deregister(&self) -> Result<(), DruidError> {
        let _guard = self.lock.lock().await;
        let member_path = self.member_path.write().take();
        let client = self.client.read().clone();
        if let (Some(client), Some(member_path)) = (client, member_path) {
            match client.delete(&member_path, None).await {
                Ok(()) | Err(ZookeeperError::NoNode) => {}
                Err(error) => return Err(Self::client_error(error)),
            }
        }
        if self.private_zk_client.swap(false, Ordering::AcqRel) {
            self.client.write().take();
        }
        Ok(())
    }

    /// 销毁注册器，等同 deregister。
    pub async fn destroy(&self) -> Result<(), DruidError> {
        self.deregister().await
    }

    /// 注入外部 client；注册器不接管其生命周期。
    pub fn set_client(&self, client: Arc<Client>) {
        *self.client.write() = Some(client);
        self.private_zk_client.store(false, Ordering::Release);
    }

    /// 返回 client。
    #[must_use]
    pub fn client(&self) -> Option<Arc<Client>> {
        self.client.read().clone()
    }

    /// 设置 `ZooKeeper` ensemble 连接串。
    pub fn set_zk_connect_string(&self, value: impl Into<String>) {
        *self.zk_connect_string.write() = Some(value.into());
    }

    /// 返回 `ZooKeeper` ensemble 连接串。
    #[must_use]
    pub fn zk_connect_string(&self) -> Option<String> {
        self.zk_connect_string.read().clone()
    }

    /// 设置父路径。
    pub fn set_path(&self, value: impl Into<String>) {
        *self.path.write() = value.into();
    }

    /// 返回父路径。
    #[must_use]
    pub fn path(&self) -> String {
        self.path.read().clone()
    }

    fn properties_bytes(payload: &[ZookeeperNodeInfo]) -> Result<Vec<u8>, DruidError> {
        let mut properties = HashMap::new();
        for node in payload {
            for (name, value) in [
                ("host", node.host().map(ToOwned::to_owned)),
                ("port", node.port().map(|port| port.to_string())),
                ("database", node.database().map(ToOwned::to_owned)),
                ("username", node.username().map(ToOwned::to_owned)),
                ("password", node.password().map(ToOwned::to_owned)),
            ] {
                if let Some(value) = value {
                    properties.insert(format!("{}{name}", node.prefix()), value);
                }
            }
        }
        let mut bytes = Vec::new();
        java_properties::write(&mut bytes, &properties)
            .map_err(|error| DruidError::Other(error.to_string()))?;
        Ok(bytes)
    }

    fn client_error(error: ZookeeperError) -> DruidError {
        DruidError::Other(format!("ZooKeeper: {error}"))
    }
}
