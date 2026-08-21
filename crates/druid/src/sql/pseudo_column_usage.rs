/// Places where a database pseudo-column or hidden column may be used.
///
/// Corresponds to Java: `java.sql.PseudoColumnUsage`, returned by
/// `DatabaseMetaData#getPseudoColumns`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PseudoColumnUsage {
    /// The column may appear only in a select list.
    SelectListOnly,
    /// The column may appear only in a `WHERE` clause.
    WhereClauseOnly,
    /// The column has no usage restriction.
    NoUsageRestrictions,
    /// The driver cannot determine the usage restriction.
    UsageUnknown,
}
