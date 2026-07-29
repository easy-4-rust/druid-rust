use prost::Message;
use serde::{Deserialize, Serialize};

/// 可被管理端采集的服务节点。
///
/// 对应 Java: `com.alibaba.druid.admin.model.ServiceNode`。
#[derive(Clone, Eq, PartialEq, Message, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceNode {
    /// 注册中心实例标识。
    #[prost(string, tag = "1")]
    pub id: String,
    /// 管理 HTTP 端口。
    #[prost(uint32, tag = "2")]
    pub port: u32,
    /// 主机名或 IP。
    #[prost(string, tag = "3")]
    pub address: String,
    /// 服务名称。
    #[prost(string, tag = "4")]
    pub service_name: String,
}

impl ServiceNode {
    /// 返回 Java `toMap` 使用的服务去重键。
    #[must_use]
    pub fn map_key(&self) -> String {
        format!("{}-{}-{}", self.service_name, self.address, self.port)
    }
}
