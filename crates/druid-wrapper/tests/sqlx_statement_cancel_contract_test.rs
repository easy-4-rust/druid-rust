use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionAdapter;

/// 原实库合同要求取消请求先于执行；只拒绝该次 admission，保留语句和连接复用。
#[tokio::test]
async fn pending_cancel_prevents_update_once_without_closing_statement_or_connection() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    connection
        .exec("CREATE TABLE cancelled_insert (id INTEGER)", Vec::new())
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement(
            "INSERT INTO cancelled_insert VALUES (?)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    statement
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(7),
        })
        .unwrap();
    statement.cancel().expect("cancel 请求必须被接受");
    let error = statement
        .execute_prepared_update()
        .await
        .expect_err("被取消的执行不得写入数据库");
    assert!(matches!(error, RdbcError::SqlException(ref e) if e.sql_state()==Some("HY008")));
    assert!(!statement.is_closed().unwrap());
    assert_eq!(statement.query_timeout().unwrap(), 0);
    let rows = connection
        .fetch("SELECT COUNT(*) FROM cancelled_insert", Vec::new())
        .await
        .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(0)]);
    assert_eq!(statement.execute_prepared_update().await.unwrap(), 1);
    let rows = connection
        .fetch("SELECT id FROM cancelled_insert", Vec::new())
        .await
        .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(7)]);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 活动流游标收到取消后退出，之后同一语句及物理连接仍可执行新的查询。
#[tokio::test]
async fn cancel_active_cursor_releases_actor_and_allows_a_fresh_execution() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement(
            "SELECT 1 AS n UNION ALL SELECT 2 UNION ALL SELECT 3",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut rows = statement.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(1)
    );
    statement.cancel().expect("活动流游标必须接受取消请求");
    let error = tokio::time::timeout(std::time::Duration::from_secs(5), rows.next())
        .await
        .expect("取消不能留下永久等待的游标")
        .expect_err("已取消的游标不得继续返回行");
    assert!(
        matches!(error, RdbcError::ConnectionDiscarded)
            || matches!(error, RdbcError::SqlException(ref e) if e.sql_state()==Some("HY008"))
    );
    rows.close().await.unwrap();
    assert!(!statement.is_closed().unwrap());
    let mut fresh = statement
        .execute_prepared_query()
        .await
        .expect("活动取消不能使后续执行永久中毒");
    for expected in [1, 2, 3] {
        assert!(fresh.next().await.unwrap());
        assert_eq!(
            fresh.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(expected)
        );
    }
    assert!(!fresh.next().await.unwrap());
    fresh.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 取消活动游标本身必须释放 actor，不能依赖调用方随后 close 或 Drop 旧结果集。
#[tokio::test]
async fn cancel_releases_connection_while_old_cursor_is_retained() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement("SELECT 1 UNION ALL SELECT 2", StatementOptions::default())
        .await
        .unwrap();
    statement.set_fetch_size(1).unwrap();
    let mut old = statement.execute_prepared_query().await.unwrap();
    assert!(old.next().await.unwrap());
    statement.cancel().unwrap();
    let probe = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        connection.fetch("SELECT 9", Vec::new()),
    )
    .await
    .expect("cancel 本身必须释放连接锁")
    .unwrap();
    assert_eq!(probe[0].values, vec![Value::Int(9)]);
    let mut fresh = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        statement.execute_prepared_query(),
    )
    .await
    .expect("新执行不能等待旧游标 close")
    .unwrap();
    assert!(fresh.next().await.unwrap());
    assert_eq!(
        fresh.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(1)
    );
    assert!(old.next().await.is_err(), "新代次不得复活旧游标");
    old.close().await.unwrap();
    assert!(fresh.next().await.unwrap(), "旧游标关闭不得取消新代次");
    assert_eq!(
        fresh.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(2)
    );
    fresh.close().await.unwrap();
    statement.close().await.unwrap();
    assert!(matches!(
        statement.cancel(),
        Err(RdbcError::ConnectionDiscarded)
    ));
    connection.close().await.unwrap();
}

/// 普通语句取消的批次准入没有写入副作用，取消请求只消费一次，批次内容保留。
#[tokio::test]
async fn pending_cancel_preserves_ordinary_batch_for_a_single_retry() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    connection
        .exec("CREATE TABLE batch_cancel (id INTEGER)", Vec::new())
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .add_batch("INSERT INTO batch_cancel VALUES (1)".to_owned())
        .unwrap();
    statement
        .add_batch("INSERT INTO batch_cancel VALUES (2)".to_owned())
        .unwrap();
    statement.cancel().unwrap();
    statement.cancel().unwrap();
    let error = statement.execute_batch().await.unwrap_err();
    assert!(matches!(error, RdbcError::SqlException(ref e) if e.sql_state()==Some("HY008")));
    assert_eq!(statement.batch().unwrap().len(), 2);
    assert_eq!(
        connection
            .fetch("SELECT COUNT(*) FROM batch_cancel", Vec::new())
            .await
            .unwrap()[0]
            .values,
        vec![Value::Int(0)]
    );
    assert_eq!(statement.execute_batch().await.unwrap(), vec![1, 1]);
    assert_eq!(
        connection
            .fetch("SELECT COUNT(*) FROM batch_cancel", Vec::new())
            .await
            .unwrap()[0]
            .values,
        vec![Value::Int(2)]
    );
    statement.cancel().unwrap();
    let error = match statement.execute_query("SELECT 3").await {
        Ok(_) => panic!("预取消必须拒绝本次查询"),
        Err(error) => error,
    };
    assert!(matches!(error, RdbcError::SqlException(ref e) if e.sql_state()==Some("HY008")));
    let mut result = statement.execute_query("SELECT 3").await.unwrap();
    assert!(result.next().await.unwrap());
    assert_eq!(
        result.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(3)
    );
    result.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
