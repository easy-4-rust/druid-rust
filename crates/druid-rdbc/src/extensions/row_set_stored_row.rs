use crate::types::RdbcObject;
use std::collections::BTreeMap;

/// RowSet 内部单行：原始值、当前值及尚未提交的列更新保持分离。
/// 对应 Java: javax.sql.RowSetInternal#getOriginalRow 的行状态部分（Rust 内部扩展）。
#[derive(Clone)]
pub(crate) struct RowSetStoredRow {
    pub(crate) values: Vec<RdbcObject>,
    pub(crate) original: Option<Vec<RdbcObject>>,
    pub(crate) pending: BTreeMap<usize, RdbcObject>,
    pub(crate) updated: bool,
    pub(crate) deleted: bool,
}

impl RowSetStoredRow {
    pub(crate) fn original(values: Vec<RdbcObject>) -> Self {
        Self {
            original: Some(values.clone()),
            values,
            pending: BTreeMap::new(),
            updated: false,
            deleted: false,
        }
    }

    pub(crate) fn inserted(values: Vec<RdbcObject>) -> Self {
        Self {
            original: None,
            values,
            pending: BTreeMap::new(),
            updated: false,
            deleted: false,
        }
    }
}
