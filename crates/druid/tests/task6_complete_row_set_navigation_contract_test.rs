//! 保留原缓冲结果集的标签、导航、整数参数边界和精确关闭错误合同。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType,
};
use druid_rdbc::types::{Row, Value};

fn task6_buffered_value_at_index(
    result_set: &BufferedRowSet,
    column_index: usize,
) -> Result<Value, RdbcError> {
    result_set.value(ColumnIndex::new(column_index)?)
}

fn task6_set_buffered_fetch_direction(
    result_set: &mut BufferedRowSet,
    direction: i32,
) -> Result<(), RdbcError> {
    result_set.set_fetch_direction(FetchDirection::try_from(direction)?)
}

/// 完整保留固定 BASE 案例，不用成功导航替代参数拒绝和关闭错误观察。
/// 对应 Java: java.sql.ResultSet。
#[tokio::test]
async fn task6_row_set_result_set_preserves_labels_navigation_and_validation() {
    let mut result_set = BufferedRowSet::with_column_labels(
        vec![
            Row::new(vec![Value::Int(1), Value::Null]),
            Row::new(vec![Value::Int(2), Value::String("two".to_string())]),
        ],
        vec!["ID".to_string(), "label".to_string()],
    );
    assert!(result_set.is_before_first().unwrap());
    assert_eq!(result_set.find_column("id").unwrap().get(), 1);
    assert!(result_set.first().await.unwrap());
    assert_eq!(result_set.row().unwrap(), 1);
    assert_eq!(
        task6_buffered_value_at_index(&result_set, 2).unwrap(),
        Value::Null
    );
    assert!(result_set.was_null().unwrap());
    assert!(result_set.last().await.unwrap());
    assert_eq!(result_set.row().unwrap(), 2);
    assert!(result_set.absolute(-2).await.unwrap());
    assert_eq!(result_set.row().unwrap(), 1);
    assert!(result_set.relative(1).await.unwrap());
    assert_eq!(result_set.row().unwrap(), 2);
    result_set.before_first().await.unwrap();
    assert!(result_set.next().await.unwrap());
    result_set.after_last().await.unwrap();
    assert!(result_set.is_after_last().unwrap());
    assert!(result_set.previous().await.unwrap());
    task6_set_buffered_fetch_direction(&mut result_set, 1001).unwrap();
    assert_eq!(result_set.fetch_direction().unwrap().rdbc_code(), 1001);
    result_set.set_fetch_size(32).unwrap();
    assert_eq!(result_set.fetch_size().unwrap(), 32);
    assert_eq!(result_set.result_set_type().unwrap().rdbc_code(), 1004);
    assert_eq!(result_set.concurrency().unwrap().rdbc_code(), 1007);
    assert_eq!(result_set.holdability().unwrap().rdbc_code(), 1);
    assert!(matches!(
        result_set.find_column("missing"),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        task6_buffered_value_at_index(&result_set, 0),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        task6_set_buffered_fetch_direction(&mut result_set, 999),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        result_set.set_fetch_size(-1),
        Err(RdbcError::InvalidArgument(_))
    ));
    // 非法输入必须拒绝且不改变已有配置或当前游标。
    assert_eq!(result_set.fetch_direction().unwrap().rdbc_code(), 1001);
    assert_eq!(result_set.fetch_size().unwrap(), 32);
    assert_eq!(result_set.row().unwrap(), 2);
    result_set.close().await.unwrap();
    assert!(result_set.is_closed().unwrap());
    assert!(
        matches!(result_set.next().await, Err(RdbcError::Other(message)) if message == "result set is closed")
    );
    assert!(
        matches!(result_set.meta_data(), Err(RdbcError::Other(message)) if message == "result set is closed")
    );
}

#[test]
fn result_set_descriptor_codes_and_fetch_direction_validation_preserve_jdbc_identity() {
    for (direction, code) in [
        (FetchDirection::Forward, 1000),
        (FetchDirection::Reverse, 1001),
        (FetchDirection::Unknown, 1002),
    ] {
        assert_eq!(direction.rdbc_code(), code);
        assert_eq!(FetchDirection::try_from(code).unwrap(), direction);
    }
    for code in [i32::MIN, -1, 0, 999, 1003, i32::MAX] {
        assert!(matches!(
            FetchDirection::try_from(code),
            Err(RdbcError::InvalidArgument(_))
        ));
    }
    assert_eq!(ResultSetType::ForwardOnly.rdbc_code(), 1003);
    assert_eq!(ResultSetType::ScrollInsensitive.rdbc_code(), 1004);
    assert_eq!(ResultSetType::ScrollSensitive.rdbc_code(), 1005);
    assert_eq!(ResultSetConcurrency::ReadOnly.rdbc_code(), 1007);
    assert_eq!(ResultSetConcurrency::Updatable.rdbc_code(), 1008);
    assert_eq!(ResultSetHoldability::HoldOverCommit.rdbc_code(), 1);
    assert_eq!(ResultSetHoldability::CloseAtCommit.rdbc_code(), 2);
}

#[test]
fn buffered_constructor_defaults_preserve_original_forward_hint() {
    for result_set in [
        BufferedRowSet::new(Vec::new()),
        BufferedRowSet::with_column_labels(Vec::new(), vec!["id".to_owned()]),
    ] {
        assert_eq!(result_set.fetch_direction().unwrap().rdbc_code(), 1000);
        assert_eq!(result_set.fetch_size().unwrap(), 0);
        assert_eq!(result_set.row().unwrap(), 0);
        assert!(!result_set.is_closed().unwrap());
    }
}
