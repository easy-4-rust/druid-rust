//! Druid owner registry resource kind。

/// Connection-bound resource 的 Druid 私有分类。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Task 7 consumes every kind after Task 5 moves owner policy into Druid.
pub(crate) enum ResourceKind {
    Array,
    Blob,
    Clob,
    NClob,
    Ref,
    SqlXml,
}

#[allow(dead_code)]
impl ResourceKind {
    pub(crate) const fn standard_name(self) -> &'static str {
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
