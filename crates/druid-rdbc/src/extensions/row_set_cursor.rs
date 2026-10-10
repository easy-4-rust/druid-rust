use super::{RowSetMetaDataBuffer, RowSetRowStore, SharedRowSetMetaData};
use crate::error::{RdbcError, SqlException};
use crate::statement::FetchDirection;
use crate::types::RdbcObject;
use std::collections::BTreeMap;

/// 已发布或执行暂存的游标及行状态；不独立代表另一逻辑 RowSet。
/// 对应 Java: javax.sql.RowSet 的 ResultSet 状态与 RowSetInternal 原始行视图。
#[derive(Clone)]
pub(crate) struct RowSetCursor {
    pub(crate) rows: RowSetRowStore,
    pub(crate) metadata: SharedRowSetMetaData,
    pub(crate) cursor: i64,
    pub(crate) insert: Option<BTreeMap<usize, RdbcObject>>,
    pub(crate) saved_cursor: i64,
    pub(crate) was_null: bool,
    pub(crate) fetch_direction: FetchDirection,
    pub(crate) fetch_size: i32,
}
impl RowSetCursor {
    pub(crate) fn new() -> Self {
        Self {
            rows: RowSetRowStore::new(0),
            metadata: SharedRowSetMetaData::new(Box::new(RowSetMetaDataBuffer::new())),
            cursor: -1,
            insert: None,
            saved_cursor: -1,
            was_null: false,
            fetch_direction: FetchDirection::Forward,
            fetch_size: 0,
        }
    }
    pub(crate) fn current_slot(&self) -> Result<usize, RdbcError> {
        if self.insert.is_some() {
            return Err(Self::error("RowSet cursor is on insert row"));
        }
        usize::try_from(self.cursor)
            .ok()
            .and_then(|index| self.rows.visible_slots().get(index).copied())
            .ok_or_else(|| Self::error("RowSet has no current row"))
    }
    pub(crate) fn error(message: &'static str) -> RdbcError {
        RdbcError::SqlException(Box::new(SqlException::new(0, None, Some(message.into()))))
    }
}
