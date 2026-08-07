use serde::Deserialize;

/// 数据库档案的证据化支持状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriverSupportStatus {
    Declared,
    Experimental,
    Verified,
    Certified,
}

impl DriverSupportStatus {
    /// 返回该状态是否可以计入公开支持数量。
    #[must_use]
    pub const fn counts_as_supported(self) -> bool {
        matches!(self, Self::Verified | Self::Certified)
    }
}
