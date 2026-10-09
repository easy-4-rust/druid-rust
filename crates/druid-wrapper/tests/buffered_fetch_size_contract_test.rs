//! 真实驱动缓冲结果的抓取提示与生命周期合同。

use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::{RdbcObject, RdbcTargetType, Value};
use druid_wrapper::toasty::ToastyConnectionFactory;

#[tokio::test]
async fn buffered_fetch_hint_preserves_rows_validation_and_close_priority() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("SELECT 7 AS value UNION ALL SELECT 8")
        .await
        .unwrap();
    assert_eq!(rows.fetch_size().unwrap(), 0);
    rows.set_fetch_size(100).unwrap();
    assert_eq!(rows.fetch_size().unwrap(), 100);
    assert!(matches!(
        rows.set_fetch_size(-1),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert_eq!(rows.fetch_size().unwrap(), 100);
    // 提示既不定位游标，也不截断或重新执行已缓冲的查询。
    let column = ColumnIndex::new(1).unwrap();
    assert!(matches!(
        rows.value(column),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.value(column).unwrap(), Value::Int(7));
    rows.set_fetch_size(0).unwrap();
    assert_eq!(rows.fetch_size().unwrap(), 0);
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.value(column).unwrap(), Value::Int(8));
    assert!(!rows.next().await.unwrap());
    rows.close().await.unwrap();
    assert!(matches!(
        rows.fetch_size(),
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        rows.set_fetch_size(1),
        Err(RdbcError::ConnectionDiscarded)
    ));
    // 原合同先拒绝非法参数，再检查游标关闭状态。
    assert!(matches!(
        rows.set_fetch_size(-1),
        Err(RdbcError::InvalidArgument(_))
    ));
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn buffered_fetch_hint_rejects_expired_driver_lease() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement.execute_query("SELECT 1").await.unwrap();
    rows.set_fetch_size(10).unwrap();
    connection.close().await.unwrap();
    assert!(matches!(
        rows.fetch_size(),
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        rows.set_fetch_size(20),
        Err(RdbcError::ConnectionDiscarded)
    ));
    rows.close().await.unwrap();
}

/// 验证真实缓冲驱动的标准值与兼容对象入口保留各自的原游标错误。
#[tokio::test]
async fn buffered_value_and_object_preserve_distinct_cursor_error_contracts() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement.execute_query("SELECT 7 AS value").await.unwrap();
    let column = ColumnIndex::new(1).unwrap();
    let custom = RdbcTargetType::Custom("X".to_owned());
    assert!(
        matches!(rows.value(column), Err(RdbcError::InvalidArgument(message))
        if message == "result set has no current row")
    );
    assert!(matches!(rows.object(column), Err(RdbcError::Other(message))
        if message == "result set cursor is not positioned on a row"));
    assert!(
        matches!(rows.object_as(column, &custom), Err(RdbcError::Other(message))
        if message == "result set cursor is not positioned on a row")
    );
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.value(column).unwrap(), Value::Int(7));
    assert!(matches!(
        rows.object(column).unwrap(),
        RdbcObject::Scalar(Value::Int(7))
    ));
    assert!(matches!(
        rows.object_as(column, &custom),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_typed_custom"
        })
    ));
    rows.close().await.unwrap();
    assert!(matches!(
        rows.value(column),
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        rows.object(column),
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        rows.object_as(column, &custom),
        Err(RdbcError::ConnectionDiscarded)
    ));
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
