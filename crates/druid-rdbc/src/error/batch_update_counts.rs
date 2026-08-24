//! JDBC 4.2 批处理更新计数的无损值对象。

use super::RdbcError;

/// 保存 JDBC 批处理的 64 位更新计数，并提供两种明确的 32 位视图。
///
/// 对应 Java：`java.sql.BatchUpdateException#getLargeUpdateCounts()` 与
/// `java.sql.BatchUpdateException#getUpdateCounts()`。内部只保存 `i64`，避免先截断后无法
/// 恢复；Java 视图严格模拟 narrowing primitive conversion，checked 视图则拒绝溢出。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BatchUpdateCounts {
    large: Vec<i64>,
}

impl BatchUpdateCounts {
    /// 从 JDBC 4.2 `long[]` 语义创建更新计数。
    ///
    /// # 参数
    /// - `large`：按批处理命令顺序排列的 64 位更新计数。
    #[must_use]
    pub fn from_large(large: Vec<i64>) -> Self {
        Self { large }
    }

    /// 返回无损的 JDBC 4.2 大更新计数视图。
    #[must_use]
    pub fn large(&self) -> &[i64] {
        &self.large
    }

    /// 返回批处理命令计数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.large.len()
    }

    /// 返回是否尚未记录任何批处理命令。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.large.is_empty()
    }

    /// 返回模拟 Java `long` 到 `int` 强制转换的计数视图。
    ///
    /// 超出 `i32` 范围的高位会像 Java narrowing conversion 一样被截断；需要拒绝溢出的
    /// 调用方应使用 [`Self::try_i32`]。
    #[must_use]
    pub fn jdbc_i32(&self) -> Vec<i32> {
        self.large.iter().map(|value| *value as i32).collect()
    }

    /// 返回经过范围检查的 32 位计数视图。
    ///
    /// # 错误
    /// 任一更新计数超出 `i32` 范围时返回 [`RdbcError::InvalidArgument`]，并保留发生溢出的
    /// 命令位置和值。
    pub fn try_i32(&self) -> Result<Vec<i32>, RdbcError> {
        self.large
            .iter()
            .enumerate()
            .map(|(index, value)| {
                i32::try_from(*value).map_err(|_| {
                    RdbcError::InvalidArgument(format!(
                        "batch update count at index {index} exceeds i32: {value}"
                    ))
                })
            })
            .collect()
    }
}

impl From<Vec<i64>> for BatchUpdateCounts {
    fn from(value: Vec<i64>) -> Self {
        Self::from_large(value)
    }
}

impl From<Vec<i32>> for BatchUpdateCounts {
    fn from(value: Vec<i32>) -> Self {
        Self::from_large(value.into_iter().map(i64::from).collect())
    }
}
