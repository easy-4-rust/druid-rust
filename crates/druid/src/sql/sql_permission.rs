/// Permission identifier for sensitive RDBC operations.
///
/// Corresponds to Java: `java.sql.SQLPermission`. Java protects driver logging, deregistration,
/// network timeout, and connection abort operations with this permission. Rust delegates the
/// check to an explicit guardrail or policy layer.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SqlPermission {
    name: String,
    actions: Option<String>,
}

impl SqlPermission {
    /// Creates an identifier from a permission `name` and optional action string.
    ///
    /// Standard RDBC permissions normally ignore actions; the field remains for policy extensions.
    #[must_use]
    pub fn new(name: impl Into<String>, actions: Option<String>) -> Self {
        Self {
            name: name.into(),
            actions,
        }
    }
    /// Returns the permission name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the action string; standard RDBC permissions normally do not use it.
    #[must_use]
    pub fn actions(&self) -> Option<&str> {
        self.actions.as_deref()
    }
}
