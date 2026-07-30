//! 对应 Java 枚举：`com.alibaba.druid.pool.ha.node.NodeEventTypeEnum`。

/// HA 节点变更类型。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.node.NodeEventTypeEnum`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeEventTypeEnum {
    /// 新增节点。
    Add,
    /// 删除节点。
    Delete,
}
