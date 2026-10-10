//! Task7：池化结果集转换成 owning Stream 后的提前丢弃合同。

use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn dropping_pooled_owning_stream_releases_cursor_while_statement_stays_alive() {
    let pool = DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let cursor = statement.execute_query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200) SELECT x FROM n").await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    let mut stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1).unwrap()));
    assert_eq!(
        std::future::poll_fn(|context| stream.as_mut().poll_next(context))
            .await
            .unwrap()
            .unwrap(),
        1
    );
    drop(stream);
    let rows = tokio::time::timeout(
        Duration::from_secs(5),
        connection.fetch("SELECT 73", vec![]),
    )
    .await
    .expect("owning Stream丢弃必须释放数据库游标，不应等待仍存活的Statement关闭")
    .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(73)]);
    assert!(!statement.is_closed().unwrap());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 0);
    pool.close().await;
}

#[tokio::test]
async fn dropping_pooled_owning_stream_on_plain_thread_uses_owner_runtime() {
    let pool = DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let cursor = statement.execute_query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200) SELECT x FROM n").await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    let mut stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1).unwrap()));
    assert_eq!(
        std::future::poll_fn(|context| stream.as_mut().poll_next(context))
            .await
            .unwrap()
            .unwrap(),
        1
    );
    std::thread::spawn(move || drop(stream)).join().unwrap();
    let rows = tokio::time::timeout(
        Duration::from_secs(5),
        connection.fetch("SELECT 73", vec![]),
    )
    .await
    .expect("owning Stream丢弃必须释放数据库游标，不应等待仍存活的Statement关闭")
    .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(73)]);
    assert!(!statement.is_closed().unwrap());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 0);
    pool.close().await;
}

#[tokio::test]
async fn dropping_unpolled_pooled_owning_stream_releases_cursor() {
    let pool = DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let cursor = statement.execute_query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200) SELECT x FROM n").await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    let stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1).unwrap()));
    // 从未轮询也已经拥有驱动游标，不能把释放责任留给仍然存活的语句。
    drop(stream);
    let rows = tokio::time::timeout(
        Duration::from_secs(5),
        connection.fetch("SELECT 73", vec![]),
    )
    .await
    .expect("未轮询的owning Stream也必须交接游标清理")
    .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(73)]);
    assert!(!statement.is_closed().unwrap());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 0);
    pool.close().await;
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_pooled_stream_with_pending_next_releases_cursor() {
    let pool = DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let cursor = statement.execute_query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200) SELECT x FROM n").await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    let mut stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1).unwrap()));
    // 当前线程尚未调度worker，保证实际停在等待首行的future内部。
    std::future::poll_fn(|context| {
        assert!(stream.as_mut().poll_next(context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(stream);
    let rows = tokio::time::timeout(
        Duration::from_secs(5),
        connection.fetch("SELECT 73", vec![]),
    )
    .await
    .expect("等待首行期间丢弃owning Stream必须交接游标清理")
    .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(73)]);
    assert!(!statement.is_closed().unwrap());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 0);
    pool.close().await;
}
