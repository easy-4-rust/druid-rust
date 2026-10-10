use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType, StatementOptions,
};
use druid_wrapper::sqlx::SqlxConnectionAdapter;

async fn assert_async_direction_rejected(direction: FetchDirection) {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query("SELECT 7 AS value").await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    assert_eq!(
        cursor.result_set_type().unwrap(),
        ResultSetType::ForwardOnly
    );
    assert_eq!(cursor.fetch_direction().unwrap(), FetchDirection::Forward);
    assert!(
        cursor.set_fetch_direction(direction).is_err(),
        "JDBC forward-only cursor must reject every non-forward direction"
    );
    assert_eq!(cursor.fetch_direction().unwrap(), FetchDirection::Forward);
    assert!(cursor.next().await.unwrap());
    cursor.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn async_forward_cursor_rejects_unknown_without_changing_direction() {
    assert_async_direction_rejected(FetchDirection::Unknown).await;
}

#[tokio::test]
async fn async_forward_cursor_rejects_reverse_without_changing_direction() {
    assert_async_direction_rejected(FetchDirection::Reverse).await;
}

#[tokio::test]
async fn scroll_cursor_still_accepts_unknown_direction() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::CloseAtCommit,
    );
    let mut statement = connection.create_statement(options).await.unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query("SELECT 7 AS value").await.unwrap();
    assert_eq!(cursor.known_row_count(), Some(1));
    cursor.set_fetch_direction(FetchDirection::Unknown).unwrap();
    assert_eq!(cursor.fetch_direction().unwrap(), FetchDirection::Unknown);
    assert!(cursor.last().await.unwrap());
    cursor.close().await.unwrap();
    connection.close().await.unwrap();
}
