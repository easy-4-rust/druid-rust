use serde_json::{Map, Value};

/// Wall 函数聚合统计快照。
///
/// 对应 Java: `com.alibaba.druid.wall.WallFunctionStatValue`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallFunctionStatValue {
    pub name: String,
    pub invoke_count: u64,
}

impl WallFunctionStatValue {
    /// 转为 Java 管理协议字段。
    #[must_use]
    pub fn to_map(&self) -> Map<String, Value> {
        Map::from_iter([
            ("name".to_owned(), self.name.clone().into()),
            ("invokeCount".to_owned(), self.invoke_count.into()),
        ])
    }
}
