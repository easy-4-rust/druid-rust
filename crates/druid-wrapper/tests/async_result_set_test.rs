use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{
    GeneratedKeys, ParameterIndex, PreparedParameter, ResultSetType, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionAdapter;
use std::time::Duration;

const ROWS: &str = "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200) SELECT x AS value FROM n";
const LATE_ERROR: &str = "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200) SELECT CASE WHEN x=200 THEN abs(-9223372036854775808) ELSE x END AS value FROM n";

#[tokio::test]
async fn forward_query_with_bounded_fetch_reports_unknown_total_and_real_rows() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    assert_eq!(
        cursor.known_row_count(),
        None,
        "bounded query must not materialize every row"
    );
    assert_eq!(
        cursor.result_set_type().unwrap(),
        ResultSetType::ForwardOnly
    );
    assert_eq!(cursor.meta_data().unwrap().column_count(), 1);
    assert_eq!(
        cursor.find_column("value").unwrap(),
        ColumnIndex::new(1).unwrap()
    );
    for expected in 1..=200 {
        assert!(cursor.next().await.unwrap());
        assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), expected);
    }
    assert!(!cursor.next().await.unwrap());
    assert!(!cursor.next().await.unwrap());
    cursor.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn database_error_in_later_row_does_not_prevent_first_row_consumption() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement
        .execute_query(LATE_ERROR)
        .await
        .expect("late driver error belongs to next, not eager query setup");
    for expected in 1..200 {
        assert!(cursor.next().await.unwrap());
        assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), expected);
    }
    assert!(cursor.next().await.is_err());
    cursor.close().await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_error(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn explicit_cursor_close_releases_same_connection_before_exhaustion() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    assert!(cursor.next().await.unwrap());
    cursor.close().await.unwrap();
    cursor.close().await.unwrap();
    assert!(cursor.next().await.is_err());
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_close(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn dropped_cursor_releases_actor_and_parent_close_finishes_with_live_cursor() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    assert!(cursor.next().await.unwrap());
    drop(cursor);
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_drop(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), connection.close())
        .await
        .unwrap()
        .unwrap();
    assert!(cursor.next().await.is_err());
    cursor.close().await.unwrap();
}

#[tokio::test]
async fn prepared_bounded_query_preserves_parameters_and_generic_execute_stays_buffered() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement("SELECT ? AS value", StatementOptions::default())
        .await
        .unwrap();
    prepared.set_fetch_size(1).unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(42),
        })
        .unwrap();
    let mut cursor = prepared.execute_prepared_query().await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    assert!(cursor.next().await.unwrap());
    assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), 42);
    cursor.close().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let results = statement.execute(ROWS, GeneratedKeys::None).await.unwrap();
    match &results[0] {
        druid_rdbc::statement::ExecutionResult::ResultSet(rows) => {
            assert_eq!(rows.known_row_count(), Some(200));
        }
        druid_rdbc::statement::ExecutionResult::UpdateCount(_) => {
            panic!("query must remain a result set")
        }
    }
    connection.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_next_retains_accepted_row_and_same_cursor_can_resume() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    let mut next = cursor.next();
    let first_poll =
        futures::future::poll_fn(|context| std::task::Poll::Ready(next.as_mut().poll(context)))
            .await;
    assert!(
        first_poll.is_pending(),
        "current-thread worker cannot run before request is accepted"
    );
    drop(next);
    // 已接纳的请求仍属于游标，取消调用方 future 不能跳过其第一行。
    assert!(cursor.next().await.unwrap());
    assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    assert!(cursor.next().await.unwrap());
    assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), 2);
    cursor.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_cursor_close_can_be_awaited_again_without_losing_worker() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    let mut close = cursor.close();
    let first_poll =
        futures::future::poll_fn(|context| std::task::Poll::Ready(close.as_mut().poll(context)))
            .await;
    assert!(first_poll.is_pending());
    drop(close);
    cursor.close().await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_cancelled_close(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn zero_hint_and_scroll_queries_preserve_buffered_contract() {
    use druid_rdbc::statement::{ResultSetConcurrency, ResultSetHoldability};
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut default = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = default.execute_query(ROWS).await.unwrap();
    assert_eq!(rows.known_row_count(), Some(200));
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    rows.close().await.unwrap();
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::CloseAtCommit,
    );
    let mut scroll = connection.create_statement(options).await.unwrap();
    scroll.set_fetch_size(1).unwrap();
    let mut rows = scroll.execute_query(ROWS).await.unwrap();
    assert_eq!(rows.known_row_count(), Some(200));
    assert!(rows.last().await.unwrap());
    assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 200);
    rows.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn query_setup_error_preserves_driver_error_and_releases_actor() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let error = match statement
        .execute_query("SELECT * FROM absent_task7_table")
        .await
    {
        Ok(_) => panic!("absent table must remain a database error"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("absent_task7_table"),
        "startup must preserve driver error: {error}"
    );
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_startup_error(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn owning_stream_maps_rows_and_early_drop_releases_same_physical_connection() {
    use futures::StreamExt;
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let cursor = statement.execute_query(ROWS).await.unwrap();
    let mut stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1)?));
    assert_eq!(stream.next().await.unwrap().unwrap(), 1);
    assert_eq!(stream.next().await.unwrap().unwrap(), 2);
    drop(stream);
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_stream_drop(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    connection.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_query_setup_waiting_for_connection_does_not_leave_a_worker() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut first = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut second = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    first.set_fetch_size(1).unwrap();
    second.set_fetch_size(1).unwrap();
    let mut cursor = first.execute_query(ROWS).await.unwrap();
    let mut setup = second.execute_query("SELECT 777");
    let first_poll =
        futures::future::poll_fn(|context| std::task::Poll::Ready(setup.as_mut().poll(context)))
            .await;
    assert!(first_poll.is_pending());
    drop(setup);
    cursor.close().await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_cancelled_setup(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    let mut cursor = second.execute_query("SELECT 888 AS value").await.unwrap();
    assert!(cursor.next().await.unwrap());
    assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), 888);
    cursor.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_cursor_with_pending_next_releases_connection() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    let mut next = cursor.next();
    let first_poll =
        futures::future::poll_fn(|context| std::task::Poll::Ready(next.as_mut().poll(context)))
            .await;
    assert!(first_poll.is_pending());
    drop(next);
    drop(cursor);
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_pending_drop(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn generic_prepared_execute_with_fetch_hint_preserves_known_buffered_count() {
    use druid_rdbc::statement::ExecutionResult;
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement("SELECT ? AS value", StatementOptions::default())
        .await
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(73),
        })
        .unwrap();
    prepared.set_fetch_size(1).unwrap();
    let mut results = prepared.execute_prepared().await.unwrap();
    match results.first_mut().unwrap() {
        ExecutionResult::ResultSet(rows) => {
            assert_eq!(rows.known_row_count(), Some(1));
            assert!(rows.next().await.unwrap());
            assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 73);
            rows.close().await.unwrap();
        }
        ExecutionResult::UpdateCount(_) => panic!("prepared SELECT must remain a result set"),
    }
    connection.close().await.unwrap();
}

#[tokio::test]
async fn closing_statement_invalidates_live_stream_and_keeps_same_connection_usable() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    connection
        .exec(
            "CREATE TEMP TABLE task7_statement_owner(id INTEGER)",
            vec![],
        )
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    statement.close().await.unwrap();
    assert!(
        cursor.is_closed().unwrap(),
        "statement close must invalidate its live result set"
    );
    assert!(cursor.next().await.is_err());
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("INSERT INTO task7_statement_owner VALUES(1)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    cursor.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn dropping_prepared_statement_invalidates_its_live_stream() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement("SELECT ? AS value", StatementOptions::default())
        .await
        .unwrap();
    statement
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(7),
        })
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_prepared_query().await.unwrap();
    drop(statement);
    assert!(
        cursor.is_closed().unwrap(),
        "dropping the statement must invalidate its owned stream"
    );
    assert!(cursor.next().await.is_err());
    tokio::time::timeout(
        Duration::from_secs(5),
        connection.exec("CREATE TABLE after_statement_drop(id INTEGER)", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    cursor.close().await.unwrap();
    connection.close().await.unwrap();
}
