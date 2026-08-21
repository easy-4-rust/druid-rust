use crate::core::Value;

/// Standard value mapping for an SQL structured type.
///
/// Corresponds to Java: `java.sql.Struct`. Attributes follow declaration order. A connection
/// type map may cause the driver to map nested UDT attributes to `SQLData` implementations.
#[derive(Clone, Debug, PartialEq)]
pub struct Struct {
    sql_type_name: String,
    attributes: Vec<Value>,
}

impl Struct {
    /// Creates a structured value from its fully qualified SQL name and ordered attributes.
    #[must_use]
    pub fn new(sql_type_name: impl Into<String>, attributes: Vec<Value>) -> Self {
        Self {
            sql_type_name: sql_type_name.into(),
            attributes,
        }
    }
    /// Returns the fully qualified SQL type name. Corresponds to Java: `getSQLTypeName`.
    #[must_use]
    pub fn sql_type_name(&self) -> &str {
        &self.sql_type_name
    }
    /// Returns attributes in declaration order. Corresponds to Java: `getAttributes`.
    #[must_use]
    pub fn attributes(&self) -> &[Value] {
        &self.attributes
    }
}
