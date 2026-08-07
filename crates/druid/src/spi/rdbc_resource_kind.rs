/// Identifies the standard RDBC resource represented by a resource context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RdbcResourceKind {
    /// An SQL `ARRAY` resource.
    Array,
    /// An SQL `BLOB` resource.
    Blob,
    /// An SQL `CLOB` resource.
    Clob,
    /// An SQL `NCLOB` resource.
    NClob,
    /// An SQL `REF` resource.
    Ref,
    /// An SQL `SQLXML` resource.
    SqlXml,
}

impl RdbcResourceKind {
    /// Returns the Java SQL standard name used in diagnostics.
    #[must_use]
    pub const fn standard_name(self) -> &'static str {
        match self {
            Self::Array => "Array",
            Self::Blob => "Blob",
            Self::Clob => "Clob",
            Self::NClob => "NClob",
            Self::Ref => "Ref",
            Self::SqlXml => "SQLXML",
        }
    }
}
