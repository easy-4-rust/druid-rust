//! Druid resource lifecycle state。

/// 所有 handle clone 共享的 Druid owner 状态。
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ResourceState {
    Open = 0,
    Releasing = 1,
    Freed = 2,
    Invalid = 3,
}

impl ResourceState {
    pub(crate) const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Open,
            1 => Self::Releasing,
            2 => Self::Freed,
            _ => Self::Invalid,
        }
    }
}
