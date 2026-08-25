use std::fmt;

/// Stable driver or Agent identifier for a connection-bound RDBC resource.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct ResourceIdentity(String);

impl ResourceIdentity {
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

impl fmt::Debug for ResourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ResourceIdentity")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for ResourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
