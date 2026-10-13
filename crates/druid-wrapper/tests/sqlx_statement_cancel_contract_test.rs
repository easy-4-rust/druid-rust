use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};
use std::sync::Arc;
use std::time::Duration;

/// 通过真实 DruidPool 借还连接，验证普通 Statement 的旧取消句柄在租约结束后失效。
#[tokio::test]
async fn pooled_statement_cancellation_handle_expires_after_real_pool_lease_return() {
    let pool = DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();

    let mut first_lease = pool.get_connection().await.unwrap();
    let old_statement = first_lease
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let old_control = old_statement
        .cancellation_control()
        .expect("SQLite ordinary statement exposes cancellation");
    first_lease.close().await.unwrap();

    let mut second_lease = pool.get_connection().await.unwrap();
    let mut current_statement = second_lease
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    assert!(matches!(
        old_control.cancel().await,
        Err(RdbcError::ConnectionDiscarded)
    ));

    let current_control = current_statement
        .cancellation_control()
        .expect("current ordinary statement exposes cancellation");
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        current_control.cancel().await
    });
    let execution = tokio::time::timeout(
        Duration::from_secs(5),
        current_statement.execute_query(
            "WITH RECURSIVE counter(value) AS (VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000) SELECT sum(value) FROM counter",
        ),
    )
    .await
    .expect("ordinary SQLite VM cancellation must finish within the bounded test deadline");
    let error = match execution {
        Ok(_) => panic!("current ordinary statement cancellation must interrupt the query"),
        Err(error) => error,
    };
    assert!(
        matches!(error, RdbcError::SqlException(ref error) if error.sql_state() == Some("HY008"))
    );
    cancel_task
        .await
        .expect("cancel task must finish")
        .expect("current ordinary statement cancellation is accepted");

    let mut rows = current_statement.execute_query("SELECT 17").await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(17)
    );
    current_statement.close().await.unwrap();
    second_lease.close().await.unwrap();
    pool.close().await;
}

/// 通过真实语句缓存和 DruidPool 租约复用，旧 PreparedStatement 控制句柄不能触及新 owner。
#[tokio::test]
async fn pooled_prepared_cancellation_handle_cannot_cross_cached_checkout() {
    let pool = DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let sql = "WITH RECURSIVE counter(value) AS (VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000) SELECT sum(value) FROM counter";

    let mut first_lease = pool.get_connection().await.unwrap();
    let old_statement = first_lease
        .prepare_statement(sql, StatementOptions::default())
        .await
        .unwrap();
    let old_control = old_statement
        .cancellation_control()
        .expect("SQLite prepared statement exposes cancellation");
    first_lease.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 1);

    let mut second_lease = pool.get_connection().await.unwrap();
    let mut current_statement = second_lease
        .prepare_statement(sql, StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
    assert!(matches!(
        old_control.cancel().await,
        Err(RdbcError::ConnectionDiscarded)
    ));

    let current_control = current_statement
        .cancellation_control()
        .expect("new cache owner must retain its own control handle");
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        current_control.cancel().await
    });
    let execution = tokio::time::timeout(
        Duration::from_secs(5),
        current_statement.execute_prepared_query(),
    )
    .await
    .expect("SQLite VM cancellation must finish within the bounded test deadline");
    let error = match execution {
        Ok(_) => panic!("current owner cancellation must interrupt the recursive query"),
        Err(error) => error,
    };
    assert!(
        matches!(error, RdbcError::SqlException(ref error) if error.sql_state() == Some("HY008"))
    );
    cancel_task
        .await
        .expect("cancel task must finish")
        .expect("current owner cancellation is accepted");

    let mut probe = second_lease
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = probe.execute_query("SELECT 23").await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(23)
    );
    probe.close().await.unwrap();
    current_statement.close().await.unwrap();
    second_lease.close().await.unwrap();
    pool.close().await;
}

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

/// 执行 Future 被 drop 时不宣称数据库已取消，并确保物理 owner 不会重新入池。
#[tokio::test]
async fn dropped_execution_future_discards_the_native_pool_owner_once() {
    let pool = Arc::new(
        DruidPool::builder()
            .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
            .max_open(1)
            .max_idle(1)
            .build()
            .await
            .unwrap(),
    );
    let before = pool.state();
    let task_pool = Arc::clone(&pool);
    let execution = tokio::spawn(async move {
        let mut connection = task_pool.get_connection().await.unwrap();
        let mut statement = connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap();
        statement
            .execute_query(
                "WITH RECURSIVE counter(value) AS (VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 100000000) SELECT sum(value) FROM counter",
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    execution.abort();
    let join_error = match execution.await {
        Ok(_) => panic!("execution task must be cancelled"),
        Err(error) => error,
    };
    assert!(
        join_error.is_cancelled(),
        "dropping the execution future must not be reported as a database cancellation result"
    );

    let discarded = pool.state();
    assert_eq!(discarded.active_count, 0, "aborted lease must return once");
    assert_eq!(
        discarded.idle_count, 0,
        "unconfirmed SQLite owner must not be reusable"
    );
    assert_eq!(discarded.discard_count, before.discard_count + 1);
    tokio::time::timeout(Duration::from_secs(2), async {
        while pool.state().destroy_count < before.destroy_count + 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("discarded physical owner must be closed exactly once");
    assert_eq!(pool.state().destroy_count, before.destroy_count + 1);

    let _replacement = pool.get_connection().await.unwrap();
    assert_eq!(pool.state().create_count, before.create_count + 2);
    pool.close().await;
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

/// 独立取消句柄绑定获取时的执行代次，旧句柄不得取消后续查询。
#[tokio::test]
async fn independent_cancel_control_is_generation_bound() {
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

    let cancellation = statement
        .cancellation_control()
        .expect("SQLx SQLite 应提供独立取消句柄");
    let mut first = statement.execute_prepared_query().await.unwrap();
    assert!(first.next().await.unwrap());
    cancellation.cancel().await.unwrap();
    let cancelled = tokio::time::timeout(Duration::from_secs(5), first.next())
        .await
        .expect("取消应在期限内停止旧游标")
        .expect_err("旧执行应被取消");
    assert!(
        matches!(cancelled, RdbcError::ConnectionDiscarded)
            || matches!(cancelled, RdbcError::SqlException(ref error) if error.sql_state() == Some("HY008"))
    );
    cancellation
        .cancel()
        .await
        .expect("重复取消同一代次必须安全");
    first.close().await.unwrap();

    let mut other_statement = connection
        .prepare_statement("SELECT 9", StatementOptions::default())
        .await
        .unwrap();
    let mut unaffected = other_statement.execute_prepared_query().await.unwrap();
    assert!(
        unaffected.next().await.unwrap(),
        "取消不能影响同连接的其他语句"
    );
    assert_eq!(
        unaffected.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(9)
    );
    unaffected.close().await.unwrap();
    other_statement.close().await.unwrap();

    let mut second = statement.execute_prepared_query().await.unwrap();
    assert!(second.next().await.unwrap(), "旧句柄不能取消新代次");
    assert_eq!(
        second.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(1)
    );
    second.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 取得句柄后先完成一次查询，再调用旧句柄不能预取消下一次执行。
#[tokio::test]
async fn late_cancel_control_does_not_become_pending_for_next_generation() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement("SELECT 7", StatementOptions::default())
        .await
        .unwrap();
    let cancellation = statement
        .cancellation_control()
        .expect("SQLx SQLite 应提供独立取消句柄");
    let mut first = statement.execute_prepared_query().await.unwrap();
    assert!(first.next().await.unwrap());
    first.close().await.unwrap();

    cancellation.cancel().await.unwrap();
    let mut second = statement
        .execute_prepared_query()
        .await
        .expect("已完成代次上的迟到取消不得污染下一执行");
    assert!(second.next().await.unwrap());
    assert_eq!(
        second.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(7)
    );
    second.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// SQLite VM 正在执行的缓冲查询必须被独立取消句柄中断，而非仅终止流式游标任务。
#[tokio::test]
async fn independent_cancel_interrupts_running_buffered_sqlite_query() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let cancellation = statement
        .cancellation_control()
        .expect("SQLite 应提供真正的执行中取消控制");
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation.cancel().await
    });

    let query = statement.execute_query(
        "WITH RECURSIVE counter(value) AS (\
            VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000\
         ) SELECT sum(value) FROM counter",
    );
    let outcome = tokio::time::timeout(Duration::from_secs(2), query)
        .await
        .expect("取消必须在预算内让 SQLite 查询结束");
    cancel_task.await.unwrap().unwrap();
    let error = match outcome {
        Ok(_) => panic!("执行中的缓冲查询必须报告取消"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        RdbcError::SqlException(ref error) if error.sql_state() == Some("HY008")
    ));

    let rows = connection.fetch("SELECT 9", Vec::new()).await.unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(9)]);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// SQLite 更新语句的 VM 运行时也必须中断，取消后连接应通过清理屏障恢复可用。
#[tokio::test]
async fn independent_cancel_interrupts_running_sqlite_update() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    connection
        .exec("CREATE TABLE cancel_update (value INTEGER)", Vec::new())
        .await
        .unwrap();
    connection
        .exec("INSERT INTO cancel_update VALUES (0)", Vec::new())
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let cancellation = statement.cancellation_control().unwrap();
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation.cancel().await
    });

    let update = statement.execute_update(
        "UPDATE cancel_update SET value = (\
            WITH RECURSIVE counter(value) AS (\
                VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000\
            ) SELECT sum(value) FROM counter\
         )",
        GeneratedKeys::None,
    );
    let outcome = tokio::time::timeout(Duration::from_secs(2), update)
        .await
        .expect("取消必须在预算内让 SQLite 更新结束");
    cancel_task.await.unwrap().unwrap();
    assert!(matches!(
        outcome,
        Err(RdbcError::SqlException(ref error)) if error.sql_state() == Some("HY008")
    ));

    let rows = connection
        .fetch("SELECT value FROM cancel_update", Vec::new())
        .await
        .unwrap();
    assert_eq!(rows[0].values, vec![Value::Int(0)]);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 普通 Statement#execute 的结果分支必须共享同一 SQLite VM 取消通道。
#[tokio::test]
async fn independent_cancel_interrupts_running_sqlite_execute() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let cancellation = statement.cancellation_control().unwrap();
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation.cancel().await
    });
    let started = std::time::Instant::now();
    let execute = statement.execute(
        "WITH RECURSIVE counter(value) AS (\
            VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000\
         ) SELECT sum(value) FROM counter",
        GeneratedKeys::None,
    );
    let outcome = tokio::time::timeout(Duration::from_secs(2), execute)
        .await
        .expect("取消必须在预算内让 SQLite execute 结束");
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "取消应中断描述/执行阶段，不能等待 500ms 长查询自然结束"
    );
    cancel_task.await.unwrap().unwrap();
    assert!(matches!(
        outcome,
        Err(RdbcError::SqlException(ref error)) if error.sql_state() == Some("HY008")
    ));
    assert_eq!(
        connection.fetch("SELECT 11", Vec::new()).await.unwrap()[0].values,
        vec![Value::Int(11)]
    );
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// PreparedStatement 缓冲查询在参数化执行入口也必须接入 SQLite VM 中断。
#[tokio::test]
async fn independent_cancel_interrupts_running_prepared_sqlite_query() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement(
            "WITH RECURSIVE counter(value) AS (\
                VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000\
             ) SELECT sum(value) FROM counter",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    let cancellation = statement.cancellation_control().unwrap();
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation.cancel().await
    });
    let started = std::time::Instant::now();
    let query = statement.execute_prepared_query();
    let outcome = tokio::time::timeout(Duration::from_secs(2), query)
        .await
        .expect("取消必须在预算内让预编译查询结束");
    assert!(started.elapsed() < Duration::from_millis(500));
    cancel_task.await.unwrap().unwrap();
    assert!(matches!(
        outcome,
        Err(RdbcError::SqlException(ref error)) if error.sql_state() == Some("HY008")
    ));
    assert_eq!(
        connection.fetch("SELECT 12", Vec::new()).await.unwrap()[0].values,
        vec![Value::Int(12)]
    );
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// PreparedStatement 更新的参数执行路径也需要真实中断且不能提交部分更新。
#[tokio::test]
async fn independent_cancel_interrupts_running_prepared_sqlite_update() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    connection
        .exec(
            "CREATE TABLE prepared_cancel_update (value INTEGER)",
            Vec::new(),
        )
        .await
        .unwrap();
    connection
        .exec("INSERT INTO prepared_cancel_update VALUES (0)", Vec::new())
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement(
            "UPDATE prepared_cancel_update SET value = (\
                WITH RECURSIVE counter(value) AS (\
                    VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000\
                ) SELECT sum(value) FROM counter\
             ) WHERE value = ?",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    statement
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(0),
        })
        .unwrap();
    let cancellation = statement.cancellation_control().unwrap();
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation.cancel().await
    });
    let update = statement.execute_prepared_update();
    let outcome = tokio::time::timeout(Duration::from_secs(2), update)
        .await
        .expect("取消必须在预算内让预编译更新结束");
    cancel_task.await.unwrap().unwrap();
    assert!(matches!(
        outcome,
        Err(RdbcError::SqlException(ref error)) if error.sql_state() == Some("HY008")
    ));
    assert_eq!(
        connection
            .fetch("SELECT value FROM prepared_cancel_update", Vec::new())
            .await
            .unwrap()[0]
            .values,
        vec![Value::Int(0)]
    );
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// SQLite 请求 generated keys 的预编译查询仍必须把取消传到其专用 actor 路径。
#[tokio::test]
async fn independent_cancel_interrupts_running_sqlite_generated_key_query() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement_with_generated_keys(
            "WITH RECURSIVE counter(value) AS (\
                VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000\
             ) SELECT sum(value) FROM counter",
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    let cancellation = statement.cancellation_control().unwrap();
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation.cancel().await
    });
    let started = std::time::Instant::now();
    let query = statement.execute_prepared_query();
    let outcome = tokio::time::timeout(Duration::from_secs(2), query)
        .await
        .expect("取消必须在预算内让生成键查询结束");
    assert!(started.elapsed() < Duration::from_millis(500));
    cancel_task.await.unwrap().unwrap();
    assert!(matches!(
        outcome,
        Err(RdbcError::SqlException(ref error)) if error.sql_state() == Some("HY008")
    ));
    assert_eq!(
        connection.fetch("SELECT 13", Vec::new()).await.unwrap()[0].values,
        vec![Value::Int(13)]
    );
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 请求生成键的 PreparedStatement 更新必须在读取/发布键之前响应执行中取消。
#[tokio::test]
async fn independent_cancel_interrupts_running_sqlite_generated_key_update() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    connection
        .exec(
            "CREATE TABLE generated_key_cancel_update (id INTEGER PRIMARY KEY AUTOINCREMENT, value INTEGER)",
            Vec::new(),
        )
        .await
        .unwrap();
    let mut statement = connection
        .prepare_statement_with_generated_keys(
            r#"INSERT INTO generated_key_cancel_update(value)
               WITH RECURSIVE counter(value) AS (
                   VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000
               ) SELECT sum(value) FROM counter"#,
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    let cancellation = statement.cancellation_control().unwrap();
    let cancel_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation.cancel().await
    });

    let update = statement.execute_prepared_update();
    let outcome = tokio::time::timeout(Duration::from_secs(2), update)
        .await
        .expect("取消必须在预算内中断生成键更新");
    cancel_task.await.unwrap().unwrap();
    assert!(matches!(
        outcome,
        Err(RdbcError::SqlException(ref error)) if error.sql_state() == Some("HY008")
    ));

    let mut keys = statement.generated_keys().await.unwrap();
    assert!(
        !keys.next().await.unwrap(),
        "取消失败的更新不能发布部分或上一轮生成键"
    );
    keys.close().await.unwrap();
    assert_eq!(
        connection
            .fetch(
                "SELECT COUNT(*) FROM generated_key_cancel_update",
                Vec::new()
            )
            .await
            .unwrap()[0]
            .values,
        vec![Value::Int(0)],
        "被取消的生成键更新不得提交行"
    );
    assert_eq!(
        connection.fetch("SELECT 15", Vec::new()).await.unwrap()[0].values,
        vec![Value::Int(15)],
        "清理完成后连接仍应可复用"
    );
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 丢弃 Rust 执行 Future 不应伪装成数据库取消；未确认清理的连接必须丢弃。
#[tokio::test]
async fn dropping_running_sqlite_execution_future_discards_uncertain_connection() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    {
        let execution = statement.execute_query(
            "WITH RECURSIVE counter(value) AS (\
                VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000\
             ) SELECT sum(value) FROM counter",
        );
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => { let _ = result; panic!("long query unexpectedly completed early"); },
            _ = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
    }
    assert_eq!(
        connection.fetch("SELECT 14", Vec::new()).await,
        Err(RdbcError::ConnectionDiscarded),
        "unconfirmed execution cleanup must prevent reuse instead of claiming cancellation"
    );
    assert!(connection.is_closed().unwrap());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// PostgreSQL 当前没有接入独立取消通道；真实后端必须精确拒绝，而不能返回空操作句柄。
#[tokio::test]
#[ignore = "requires DRUID_DATABASE_URL pointing to a live PostgreSQL service"]
async fn live_postgres_cancellation_control_reports_feature_not_supported() {
    let url = std::env::var("DRUID_DATABASE_URL")
        .expect("live PostgreSQL cancellation test requires DRUID_DATABASE_URL");
    let mut connection = SqlxConnectionAdapter::connect(&url).await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let error = match statement.cancellation_control() {
        Ok(_) => {
            panic!("PostgreSQL cancellation must not be advertised before it reaches the server")
        }
        Err(error) => error,
    };
    assert!(matches!(
        error,
        RdbcError::SqlException(ref error) if error.sql_state() == Some("0A000")
    ));
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// MySQL 当前没有接入独立取消通道；真实后端必须精确拒绝，而不能返回空操作句柄。
#[tokio::test]
#[ignore = "requires DRUID_MYSQL_URL pointing to a live MySQL service"]
async fn live_mysql_cancellation_control_reports_feature_not_supported() {
    let url = std::env::var("DRUID_MYSQL_URL")
        .expect("live MySQL cancellation test requires DRUID_MYSQL_URL");
    let mut connection = SqlxConnectionAdapter::connect(&url).await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let error = match statement.cancellation_control() {
        Ok(_) => panic!(
            "MySQL cancellation must not be advertised before server interruption is verified"
        ),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        RdbcError::SqlException(ref error) if error.sql_state() == Some("0A000")
    ));
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
