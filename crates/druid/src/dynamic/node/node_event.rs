//! 对应 Java 类：`com.alibaba.druid.pool.ha.node.NodeEvent`。

use super::NodeEventTypeEnum;
use crate::dynamic::PropertiesUtils;
use std::collections::HashMap;

/// HA 数据源节点变更事件。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.node.NodeEvent`。调试输出只暴露密码
/// 长度，不输出密码内容。
#[derive(Clone, PartialEq, Eq)]
pub struct NodeEvent {
    event_type: NodeEventTypeEnum,
    node_name: String,
    url: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

impl NodeEvent {
    /// 创建节点事件。
    #[must_use]
    pub fn new(
        event_type: NodeEventTypeEnum,
        node_name: impl Into<String>,
        url: Option<String>,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            event_type,
            node_name: node_name.into(),
            url,
            username,
            password,
        }
    }

    /// 比较前后 properties，仅生成新增和删除事件。
    ///
    /// 与 Java 保持一致：已存在节点的 URL 或凭据变化不会生成更新事件。
    #[must_use]
    pub fn get_events_by_diff_properties(
        previous: &HashMap<String, String>,
        next: &HashMap<String, String>,
    ) -> Vec<Self> {
        let previous_names = PropertiesUtils::load_name_list(previous, Some(""));
        let next_names = PropertiesUtils::load_name_list(next, Some(""));
        let names_to_delete: Vec<String> = previous_names
            .iter()
            .filter(|name| !name.trim().is_empty() && !next_names.contains(name))
            .cloned()
            .collect();
        let names_to_add: Vec<String> = next_names
            .iter()
            .filter(|name| !name.trim().is_empty() && !previous_names.contains(name))
            .cloned()
            .collect();

        let mut events = Self::generate_events(next, &names_to_add, NodeEventTypeEnum::Add);
        events.extend(Self::generate_events(
            previous,
            &names_to_delete,
            NodeEventTypeEnum::Delete,
        ));
        events
    }

    /// 根据节点名和属性生成同一类型的事件。
    #[must_use]
    pub fn generate_events(
        properties: &HashMap<String, String>,
        names: &[String],
        event_type: NodeEventTypeEnum,
    ) -> Vec<Self> {
        names
            .iter()
            .map(|name| {
                Self::new(
                    event_type,
                    name,
                    properties.get(&format!("{name}.url")).cloned(),
                    properties.get(&format!("{name}.username")).cloned(),
                    properties.get(&format!("{name}.password")).cloned(),
                )
            })
            .collect()
    }

    /// 返回事件类型。
    #[must_use]
    pub const fn event_type(&self) -> NodeEventTypeEnum {
        self.event_type
    }

    /// 返回节点名。
    #[must_use]
    pub fn node_name(&self) -> &str {
        &self.node_name
    }

    /// 返回节点 URL。
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// 返回用户名。
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    /// 返回密码；调用方不得写入日志。
    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

impl std::fmt::Debug for NodeEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("NodeEvent");
        debug
            .field("event_type", &self.event_type)
            .field("node_name", &self.node_name)
            .field("url", &self.url)
            .field("username", &self.username);
        if let Some(password) = &self.password {
            debug.field("password_length", &password.len());
        }
        debug.finish()
    }
}
