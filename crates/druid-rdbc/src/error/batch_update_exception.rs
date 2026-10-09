use super::SqlException;

/// 批处理中至少一条更新命令失败时的异常，保留原 SQL 异常和命令顺序。
///
/// 对应 Java: java.sql.BatchUpdateException。驱动可以在首次失败时停止，或继续执行
/// 并使用 EXECUTE_FAILED(-3)；成功但计数未知的命令使用 SUCCESS_NO_INFO(-2)。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchUpdateException {
    /// 原始 SQL 异常，保留 SQLState、厂商代码、原因及 next-exception 链。
    pub exception: SqlException,
    /// 按命令顺序排列的 64 位更新计数，包含成功未知和失败标记。
    pub update_counts: Vec<i64>,
}

impl BatchUpdateException {
    /// 使用原 SQL 异常和有序更新计数创建批处理异常。
    ///
    /// 参数 exception 是底层 SQL 异常，update_counts 是命令顺序的 64 位计数；
    /// 返回保存两者的异常对象。对应 Java: BatchUpdateException 构造方法。
    #[must_use]
    pub fn new(exception: SqlException, update_counts: Vec<i64>) -> Self {
        Self {
            exception,
            update_counts,
        }
    }

    /// 返回 Java int[] 视图；超范围值按 Java long 到 int 强制转换截断高位。
    ///
    /// 返回的计数顺序及 -2/-3 标记保持不变，原 64 位计数不被修改。
    /// 对应 Java: BatchUpdateException#getUpdateCounts。
    #[must_use]
    pub fn update_counts(&self) -> Vec<i32> {
        self.update_counts
            .iter()
            .map(|value| *value as i32)
            .collect()
    }

    /// 返回按命令顺序排列的无损 64 位更新计数切片。
    ///
    /// 返回值借用原计数，不进行截断或饱和。对应 Java: BatchUpdateException#getLargeUpdateCounts。
    #[must_use]
    pub fn large_update_counts(&self) -> &[i64] {
        &self.update_counts
    }
}
