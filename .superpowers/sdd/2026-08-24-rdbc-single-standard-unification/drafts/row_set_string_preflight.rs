//! 原完整RowSet案例中首个非NULL String转换的预检，不替代完整案例验收。
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::types::{Row, Value};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
struct BufferedWake;
impl Wake for BufferedWake { fn wake(self: Arc<Self>) {} }
#[test]
fn original_row_set_boolean_string_conversion_preflight() {
    let mut rows = BufferedRowSet::with_column_labels(
        vec![Row::new(vec![Value::Null, Value::Bool(true)])],
        vec!["c1".to_owned(), "c2".to_owned()],
    );
    let waker = Waker::from(Arc::new(BufferedWake));
    let mut context = Context::from_waker(&waker);
    let mut next = Box::pin(rows.next());
    assert!(matches!(next.as_mut().poll(&mut context), Poll::Ready(Ok(true))));
    drop(next);
    assert_eq!(rows.string(ColumnIndex::new(1).unwrap()).unwrap(), None);
    let converted = rows.string(ColumnIndex::new(2).unwrap());
    assert_eq!(converted, Ok(Some("true".to_owned())), "原Bool列的String读取：{converted:?}");
}
