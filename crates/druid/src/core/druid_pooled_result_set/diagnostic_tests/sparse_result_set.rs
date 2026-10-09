//! 稀疏驱动原案例的标准接口迁移。
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::result_set::{ResultSet, ResultSetNavigation};
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
/// 明确拒绝原34种能力的驱动；记录真实调用与关闭。
/// 对应 Java: java.sql.ResultSet；保留 BASE SparsePhysicalResultSet 的拒绝合同。
#[derive(Debug)]
pub(super) struct SparseResultSet {
    calls: Arc<Mutex<Vec<&'static str>>>,
    closes: Arc<AtomicUsize>,
}
impl SparseResultSet {
    /// 参数为共享调用及关闭观察，返回尚未调用的稀疏驱动。
    pub(super) fn new(calls: Arc<Mutex<Vec<&'static str>>>, closes: Arc<AtomicUsize>) -> Self {
        Self { calls, closes }
    }
    fn refuse<T>(&self, operation: &'static str) -> Result<T, RdbcError> {
        self.calls.lock().unwrap().push(operation);
        Err(RdbcError::UnsupportedOperation { operation })
    }
}
druid_rdbc::impl_standard_wrapper!(SparseResultSet => ResultSet);
#[async_trait::async_trait]
impl ResultSet for SparseResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.refuse("result_set_next")
    }
    async fn navigate(&mut self, navigation: ResultSetNavigation) -> Result<bool, RdbcError> {
        let operation = match navigation {
            ResultSetNavigation::Next => "result_set_next",
            ResultSetNavigation::Previous => "result_set_previous",
            ResultSetNavigation::First => "result_set_first",
            ResultSetNavigation::Last => "result_set_last",
            ResultSetNavigation::BeforeFirst => "result_set_before_first",
            ResultSetNavigation::AfterLast => "result_set_after_last",
            ResultSetNavigation::Absolute(row) => {
                assert_eq!(row, -2);
                "result_set_absolute"
            }
            ResultSetNavigation::Relative(rows) => {
                assert_eq!(rows, 3);
                "result_set_relative"
            }
            _ => "result_set_navigation",
        };
        self.refuse(operation)
    }
    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.refuse("result_set_is_before_first")
    }
    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.refuse("result_set_is_after_last")
    }
    fn is_first(&self) -> Result<bool, RdbcError> {
        self.refuse("result_set_is_first")
    }
    fn is_last(&self) -> Result<bool, RdbcError> {
        self.refuse("result_set_is_last")
    }
    fn row(&self) -> Result<i32, RdbcError> {
        self.refuse("result_set_row")
    }
    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        assert_eq!(direction, FetchDirection::Forward);
        self.refuse("result_set_set_fetch_direction")
    }
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.refuse("result_set_fetch_direction")
    }
    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        assert_eq!(rows, 10);
        self.refuse("result_set_set_fetch_size")
    }
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.refuse("result_set_fetch_size")
    }
    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        self.refuse("result_set_type")
    }
    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        self.refuse("result_set_concurrency")
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.refuse("result_set_holdability")
    }
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.refuse("result_set_warnings")
    }
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.refuse("result_set_clear_warnings")
    }
    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        self.refuse("result_set_cursor_name")
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.refuse("result_set_meta_data")
    }
    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.refuse("result_set_row_updated")
    }
    fn row_inserted(&self) -> Result<bool, RdbcError> {
        self.refuse("result_set_row_inserted")
    }
    fn row_deleted(&self) -> Result<bool, RdbcError> {
        self.refuse("result_set_row_deleted")
    }
    fn insert_row(&mut self) -> Result<(), RdbcError> {
        self.refuse("result_set_insert_row")
    }
    fn update_row(&mut self) -> Result<(), RdbcError> {
        self.refuse("result_set_update_row")
    }
    fn delete_row(&mut self) -> Result<(), RdbcError> {
        self.refuse("result_set_delete_row")
    }
    fn refresh_row(&mut self) -> Result<(), RdbcError> {
        self.refuse("result_set_refresh_row")
    }
    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        self.refuse("result_set_cancel_row_updates")
    }
    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        self.refuse("result_set_move_to_insert_row")
    }
    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        self.refuse("result_set_move_to_current_row")
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closes.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closes.load(Ordering::Acquire) != 0)
    }
}
