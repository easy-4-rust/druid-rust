/// Identifies the standard RDBC resource represented by a resource context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum ResourceType {
    /// An SQL `BLOB` resource.
    Blob,
    /// An SQL `CLOB` resource.
    Clob,
    /// An SQL `NCLOB` resource.
    NClob,
    /// An SQL `SQLXML` resource.
    SqlXml,
}

impl ResourceType {
    /// Returns the Java SQL standard name used in diagnostics.
    #[must_use]
    pub const fn standard_name(self) -> &'static str {
        match self {
            Self::Blob => "Blob",
            Self::Clob => "Clob",
            Self::NClob => "NClob",
            Self::SqlXml => "SQLXML",
        }
    }
}
