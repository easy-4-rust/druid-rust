use serde::Deserialize;

/// 数据库产品使用 Druid Wall SQL 检查器的证据等级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallMode {
    Dedicated,
    FamilyCompatible,
    Generic,
    Disabled,
}
