use crate::core::{
    DruidError, ResultSetFilter, ResultSetFilterContext, ResultSetPropertyOperation,
    ResultSetPropertyValue,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSetNavigation};
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// 保留原导航与属性26个精确短路观察，不请求原生游标。
/// 对应 Java: Filter 的 ResultSet 导航和属性方法。
pub(super) struct NavigationShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl ResultSetFilter for NavigationShortCircuitFilter {
    async fn standard_result_set_navigation<'a>(
        &'a self,
        _context: &'a ResultSetFilterContext,
        navigation: ResultSetNavigation,
        _next: Pin<Box<dyn Future<Output = Result<bool, DruidError>> + Send + 'a>>,
    ) -> Result<bool, DruidError> {
        let (event, result) = match navigation {
            ResultSetNavigation::Previous => ("result_set_previous".to_owned(), true),
            ResultSetNavigation::BeforeFirst => ("result_set_before_first".to_owned(), false),
            ResultSetNavigation::AfterLast => ("result_set_after_last".to_owned(), false),
            ResultSetNavigation::First => ("result_set_first".to_owned(), false),
            ResultSetNavigation::Last => ("result_set_last".to_owned(), false),
            ResultSetNavigation::Absolute(row) => (format!("result_set_absolute:{row}"), row == 50),
            ResultSetNavigation::Relative(rows) => {
                (format!("result_set_relative:{rows}"), rows == -20)
            }
            _ => return _next.await,
        };
        self.calls.lock().unwrap().push(event);
        Ok(result)
    }

    fn standard_result_set_property(
        &self,
        _context: &ResultSetFilterContext,
        request: ResultSetPropertyOperation<'_>,
        _next: &mut dyn FnMut() -> Result<ResultSetPropertyValue, DruidError>,
    ) -> Result<ResultSetPropertyValue, DruidError> {
        use ResultSetPropertyOperation as R;
        use ResultSetPropertyValue as V;
        let (event, value) = match request {
            R::WasNull => ("result_set_was_null".to_owned(), V::Boolean(false)),
            R::IsBeforeFirst => ("result_set_is_before_first".to_owned(), V::Boolean(false)),
            R::IsAfterLast => ("result_set_is_after_last".to_owned(), V::Boolean(true)),
            R::IsFirst => ("result_set_is_first".to_owned(), V::Boolean(true)),
            R::IsLast => ("result_set_is_last".to_owned(), V::Boolean(false)),
            R::Row => ("result_set_get_row".to_owned(), V::Integer(99)),
            R::SetFetchDirection(direction) => (
                format!("result_set_set_fetch_direction:{}", direction.rdbc_code()),
                V::Unit,
            ),
            R::FetchDirection => (
                "result_set_get_fetch_direction".to_owned(),
                V::FetchDirection(FetchDirection::DriverValue(2000)),
            ),
            R::SetFetchSize(rows) => (format!("result_set_set_fetch_size:{rows}"), V::Unit),
            R::FetchSize => ("result_set_get_fetch_size".to_owned(), V::Integer(128)),
            R::ResultSetType => (
                "result_set_get_type".to_owned(),
                V::ResultSetType(ResultSetType::DriverValue(2004)),
            ),
            R::Concurrency => (
                "result_set_get_concurrency".to_owned(),
                V::Concurrency(ResultSetConcurrency::DriverValue(2007)),
            ),
            R::Holdability => (
                "result_set_get_holdability".to_owned(),
                V::Holdability(ResultSetHoldability::CloseAtCommit),
            ),
            R::CursorName => (
                "result_set_get_cursor_name".to_owned(),
                V::String(Some("filtered-cursor".to_owned())),
            ),
            R::RowUpdated => ("result_set_row_updated".to_owned(), V::Boolean(false)),
            R::RowInserted => ("result_set_row_inserted".to_owned(), V::Boolean(true)),
            R::RowDeleted => ("result_set_row_deleted".to_owned(), V::Boolean(false)),
            R::IsClosed => ("result_set_is_closed".to_owned(), V::Boolean(false)),
            R::FindColumn(label) => {
                self.calls
                    .lock()
                    .unwrap()
                    .push(format!("result_set_find_column:{label}"));
                return if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered findColumn failure".to_owned(),
                    ))
                } else {
                    Ok(V::Column(ColumnIndex::new(88).unwrap()))
                };
            }
        };
        self.calls.lock().unwrap().push(event);
        Ok(value)
    }
}
