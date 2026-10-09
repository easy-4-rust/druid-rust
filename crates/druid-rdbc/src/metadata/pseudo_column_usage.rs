/// 数据库伪列或隐藏列允许使用的位置。对应 Java: `java.sql.PseudoColumnUsage`。
///
/// 表达 `DatabaseMetaData#getPseudoColumns` 返回的四种使用限制。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PseudoColumnUsage {
    /// 仅能出现在 SELECT 选择列表，对应 `SELECT_LIST_ONLY`。
    SelectListOnly,
    /// 仅能出现在 WHERE 条件，对应 `WHERE_CLAUSE_ONLY`。
    WhereClauseOnly,
    /// 没有使用位置限制，对应 `NO_USAGE_RESTRICTIONS`。
    NoUsageRestrictions,
    /// 驱动无法确定使用限制，对应 `USAGE_UNKNOWN`。
    UsageUnknown,
}
