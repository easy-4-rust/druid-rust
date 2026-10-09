use super::cursor_state::CursorState;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::Value;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 拥有同一驱动游标的一个标准视图；释放视图不提前关闭共享游标。
/// 对应 Java: java.sql.ResultSet 的驱动测试句柄。
pub struct Cursor {
    pub state: Arc<CursorState>,
    pub from_getter: bool,
}
druid_rdbc::impl_standard_wrapper!(Cursor => ResultSet);
#[async_trait::async_trait]
impl ResultSet for Cursor {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        if self.from_getter {
            &self.state.getter_reads
        } else {
            &self.state.original_reads
        }
        .fetch_add(1, Ordering::AcqRel);
        self.state.rows.lock().await.next().await
    }
    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.state
            .rows
            .try_lock()
            .map_err(|_| RdbcError::Other("cursor busy".to_owned()))?
            .value(column)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.state
            .rows
            .try_lock()
            .map_err(|_| RdbcError::Other("cursor busy".to_owned()))?
            .meta_data()
    }
    fn was_null(&self) -> Result<bool, RdbcError> {
        self.state
            .rows
            .try_lock()
            .map_err(|_| RdbcError::Other("cursor busy".to_owned()))?
            .was_null()
    }
    fn known_row_count(&self) -> Option<u64> {
        Some(self.state.row_count)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.state.close_calls.fetch_add(1, Ordering::AcqRel);
        self.state.rows.lock().await.close().await
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.state
            .rows
            .try_lock()
            .map_err(|_| RdbcError::Other("cursor busy".to_owned()))?
            .is_closed()
    }
}
impl Drop for Cursor {
    fn drop(&mut self) {
        if self.from_getter {
            &self.state.getter_drops
        } else {
            &self.state.original_drops
        }
        .fetch_add(1, Ordering::AcqRel);
    }
}
