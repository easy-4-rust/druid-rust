/// Lifecycle state shared by all clones of an RDBC resource handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RdbcResourceState {
    /// The resource accepts operations.
    Open,
    /// One task is releasing the resource.
    Releasing,
    /// The resource was explicitly released.
    Freed,
    /// The owning connection or transaction invalidated the resource.
    Invalid,
}

impl RdbcResourceState {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Open => 0,
            Self::Releasing => 1,
            Self::Freed => 2,
            Self::Invalid => 3,
        }
    }

    pub(crate) const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Open,
            1 => Self::Releasing,
            2 => Self::Freed,
            _ => Self::Invalid,
        }
    }
}
