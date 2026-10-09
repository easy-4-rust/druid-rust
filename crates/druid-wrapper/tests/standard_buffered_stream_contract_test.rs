//! 真实Toasty缓冲结果的流读取、NULL和关闭优先合同。
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::StatementOptions;
use druid_wrapper::toasty::ToastyConnectionFactory;

#[tokio::test]
async fn toasty_buffered_streams_preserve_original_payloads_labels_and_nulls() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("SELECT X'0102' AS payload, 'first' AS label, NULL AS absent")
        .await
        .unwrap();
    assert!(rows.next().await.unwrap());
    let payload = ColumnIndex::new(1).unwrap();
    let label = ColumnIndex::new(2).unwrap();
    let absent = ColumnIndex::new(3).unwrap();
    assert_eq!(
        rows.binary_stream(payload)
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        vec![1, 2]
    );
    assert_eq!(
        rows.binary_stream_by_label("payload")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        vec![1, 2]
    );
    assert_eq!(
        rows.ascii_stream(label)
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"first"
    );
    assert_eq!(
        rows.ascii_stream_by_label("label")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"first"
    );
    assert_eq!(
        rows.unicode_stream(label)
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"first"
    );
    assert_eq!(
        rows.unicode_stream_by_label("label")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"first"
    );
    assert_eq!(
        rows.character_stream(label)
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "first"
    );
    assert_eq!(
        rows.character_stream_by_label("label")
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "first"
    );
    assert_eq!(
        rows.n_character_stream(label)
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "first"
    );
    assert_eq!(
        rows.n_character_stream_by_label("label")
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "first"
    );
    assert!(rows.binary_stream(absent).unwrap().is_none());
    assert!(rows.ascii_stream(absent).unwrap().is_none());
    assert!(rows.unicode_stream(absent).unwrap().is_none());
    assert!(rows.character_stream(absent).unwrap().is_none());
    assert!(rows.n_character_stream(absent).unwrap().is_none());
    assert!(rows.was_null().unwrap());
    assert_eq!(
        rows.binary_stream_by_label("missing"),
        Err(RdbcError::InvalidArgument(
            "unknown column: missing".to_owned()
        ))
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn toasty_buffered_streams_preserve_position_utf8_and_closed_errors() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("SELECT X'FF' AS invalid")
        .await
        .unwrap();
    let column = ColumnIndex::new(1).unwrap();
    assert_eq!(
        rows.binary_stream(column),
        Err(RdbcError::InvalidArgument(
            "result set has no current row".to_owned()
        ))
    );
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.binary_stream(column)
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        vec![255]
    );
    assert!(matches!(
        rows.character_stream(column),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        rows.n_character_stream(column),
        Err(RdbcError::DriverError(_))
    ));
    rows.close().await.unwrap();
    assert_eq!(
        rows.binary_stream(column),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert_eq!(
        rows.character_stream(column),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert_eq!(
        rows.n_character_stream(column),
        Err(RdbcError::ConnectionDiscarded)
    );
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
