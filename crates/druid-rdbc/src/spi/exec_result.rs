//! SQL 执行结果。

/// SQL 执行结果。
///
/// 对应 Java: `java.sql.Statement` 的执行结果语义。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecResult {
    /// 受影响行数，对应 `Statement#getUpdateCount`。
    pub rows_affected: u64,
    /// 最后插入的 ID，对应 `Statement#getGeneratedKeys`。
    pub last_insert_id: Option<i64>,
    /// 查询返回行数；非查询语句为 `None`。
    pub row_count: Option<u64>,
}
