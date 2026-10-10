use crate::types::RdbcObject;

/// 供 RowSet writer 检查的已提交本地行变化；值保持完整 RDBC 对象身份。
/// 对应 Java: javax.sql.RowSetWriter#writeData 的 disconnected 行状态输入（Rust 扩展）。
/// row_index 是存储中的 1-based 稳定槽位，不能当作隐藏删除行后的游标行号。
#[derive(Clone, PartialEq)]
pub enum RowSetRowChange {
    /// 尚无原始值的新行，包含槽位 row_index 与完整当前 values。
    Inserted {
        /// 1-based 内部槽位。
        row_index: usize,
        /// 新增的完整行属性。
        values: Vec<RdbcObject>,
    },
    /// 更新行，包含原始 original 与当前 values，供 provider 冲突检测。
    Updated {
        /// 1-based 内部槽位。
        row_index: usize,
        /// 数据源读取时的原始行属性。
        original: Vec<RdbcObject>,
        /// 本地 update_row 后的当前属性。
        values: Vec<RdbcObject>,
    },
    /// 删除原有行；保留 original 供 provider 检查冲突。
    Deleted {
        /// 1-based 内部槽位。
        row_index: usize,
        /// 数据源读取时的原始行属性。
        original: Vec<RdbcObject>,
    },
}

impl std::fmt::Debug for RowSetRowChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, row_index) = match self {
            Self::Inserted { row_index, .. } => ("Inserted", row_index),
            Self::Updated { row_index, .. } => ("Updated", row_index),
            Self::Deleted { row_index, .. } => ("Deleted", row_index),
        };
        f.debug_struct(kind)
            .field("row_index", row_index)
            .finish_non_exhaustive()
    }
}
