//! 查询结果行。

use super::value::Value;

/// 查询结果中的一行。
///
/// 对应 Java: `java.sql.ResultSet` 的当前行数据。
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// 当前行按列顺序保存的值。
    pub values: Vec<Value>,
}

impl Row {
    /// 创建结果行。
    ///
    /// 参数 `values` 为按列顺序排列的值。
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    /// 读取指定下标的列值；越界时返回 `None`。
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    /// 返回当前行的列数。
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// 判断当前行是否没有列。
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
