use std::fmt;

/// Stable driver or Agent identifier for a connection-bound RDBC resource.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RdbcResourceId(String);

impl RdbcResourceId {
    /// Creates an identifier supplied by a driver or remote Agent.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Creates a locally unique identifier for a detached or materialized resource.
    #[must_use]
    pub fn local() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RdbcResourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RdbcResourceId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for RdbcResourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
