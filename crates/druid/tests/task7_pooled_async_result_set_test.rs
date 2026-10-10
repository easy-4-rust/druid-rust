//! Task7：真实 SQLite 流式游标经过 Druid 池化代理时的生命周期合同。

use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use std::sync::Arc;
use std::time::Duration;

const ROWS: &str = "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200) SELECT x AS value FROM n";
const LATE_ERROR: &str = "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200) SELECT CASE WHEN x=200 THEN abs(-9223372036854775808) ELSE x END AS value FROM n";

async fn pool() -> DruidPool {
    DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(2)
        .build()
        .await
        .unwrap()
}

async fn assert_same_connection_reborrow(pool: &DruidPool) {
    assert_eq!(pool.state().active_count, 0);
    let mut connection = tokio::time::timeout(Duration::from_secs(5), pool.get())
        .await
        .unwrap()
        .unwrap();
    let rows = connection
        .fetch("SELECT id FROM lease_marker", vec![])
        .await
        .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(73)]);
    assert_eq!(
        pool.state().create_count,
        1,
        "临时表与创建计数共同证明物理连接复用"
    );
    connection.close().await.unwrap();
}

#[tokio::test]
async fn pooled_parent_close_stops_live_cursor_and_returns_same_physical_connection() {
    let pool = pool().await;
    let mut connection = pool.get().await.unwrap();
    connection
        .exec("CREATE TEMP TABLE lease_marker(id INTEGER)", vec![])
        .await
        .unwrap();
    connection
        .exec("INSERT INTO lease_marker VALUES (73)", vec![])
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    assert!(cursor.next().await.unwrap());
    assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    tokio::time::timeout(Duration::from_secs(5), connection.close())
        .await
        .unwrap()
        .unwrap();
    assert!(cursor.next().await.is_err(), "归还后的旧租约游标必须失效");
    cursor.close().await.unwrap();
    assert_same_connection_reborrow(&pool).await;
    pool.close().await;
}

#[tokio::test]
async fn pooled_statement_close_stops_live_cursor_without_discarding_connection() {
    let pool = pool().await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    assert!(cursor.next().await.unwrap());
    tokio::time::timeout(Duration::from_secs(5), statement.close())
        .await
        .unwrap()
        .unwrap();
    assert!(cursor.next().await.is_err());
    cursor.close().await.unwrap();
    let rows = tokio::time::timeout(
        Duration::from_secs(5),
        connection.fetch("SELECT 73", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(73)]);
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 0);
    pool.close().await;
}

#[tokio::test]
async fn pooled_late_driver_error_preserves_prior_rows_and_releases_cursor() {
    let pool = pool().await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement
        .execute_query(LATE_ERROR)
        .await
        .expect("后段错误不得阻止前199行消费");
    assert_eq!(cursor.known_row_count(), None);
    for expected in 1..200 {
        assert!(cursor.next().await.unwrap());
        assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), expected);
    }
    assert!(cursor.next().await.is_err());
    cursor.close().await.unwrap();
    statement.close().await.unwrap();
    let rows = tokio::time::timeout(
        Duration::from_secs(5),
        connection.fetch("SELECT 73", vec![]),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(73)]);
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn cached_prepared_statement_retains_streaming_after_fresh_logical_lease() {
    let pool = pool().await;
    let sql = "SELECT ? AS value";
    for parameter in [42, 73] {
        let mut connection = pool.get().await.unwrap();
        let mut prepared = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .unwrap();
        prepared.set_fetch_size(1).unwrap();
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::Int(parameter),
            })
            .unwrap();
        let mut cursor = prepared.execute_prepared_query().await.unwrap();
        assert_eq!(cursor.known_row_count(), None);
        assert!(cursor.next().await.unwrap());
        assert_eq!(
            cursor.int(ColumnIndex::new(1).unwrap()).unwrap(),
            parameter as i32
        );
        cursor.close().await.unwrap();
        prepared.close().await.unwrap();
        connection.close().await.unwrap();
    }
    assert!(
        pool.state().cached_prepared_statement_hit_count >= 1,
        "第二次必须真实命中缓存"
    );
    assert_eq!(pool.state().create_count, 1);
    pool.close().await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_pooled_next_keeps_first_row_and_parent_return_finishes() {
    use std::future::Future;
    use std::task::Poll;

    let pool = pool().await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut cursor = statement.execute_query(ROWS).await.unwrap();
    assert_eq!(cursor.known_row_count(), None);
    {
        let mut next = Box::pin(cursor.next());
        std::future::poll_fn(|context| {
            assert!(next.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
    }
    assert!(cursor.next().await.unwrap());
    assert_eq!(cursor.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    tokio::time::timeout(Duration::from_secs(5), connection.close())
        .await
        .unwrap()
        .unwrap();
    assert!(cursor.next().await.is_err());
    cursor.close().await.unwrap();
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().discard_count, 0);
    pool.close().await;
}
