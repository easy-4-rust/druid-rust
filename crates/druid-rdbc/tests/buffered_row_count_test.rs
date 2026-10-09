use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::{Row, Value};

#[tokio::test]
async fn buffered_count_preserves_cursor_and_is_unavailable_after_close() {
    let mut result = BufferedRowSet::new(vec![
        Row::new(vec![Value::Int(10)]),
        Row::new(vec![Value::Int(20)]),
    ]);
    assert_eq!(result.known_row_count(), Some(2));
    assert!(result.is_before_first().unwrap());
    assert!(result.next().await.unwrap());
    assert_eq!(result.known_row_count(), Some(2));
    assert_eq!(result.row().unwrap(), 1);
    assert_eq!(
        result.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(10)
    );
    assert!(result.next().await.unwrap());
    assert!(!result.next().await.unwrap());
    assert_eq!(result.known_row_count(), Some(2));
    result.close().await.unwrap();
    assert_eq!(result.known_row_count(), None);
    let empty = BufferedRowSet::new(Vec::new());
    assert_eq!(empty.known_row_count(), Some(0));
}
