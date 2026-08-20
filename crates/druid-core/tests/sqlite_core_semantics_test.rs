//! Java Druid core 主链的真实 SQLite 纵向契约。
//!
//! 本测试只验收 SQLite 真实支持的连接池、参数绑定、PreparedStatement、
//! 事务和回收语义。SQLite 没有存储过程，因此 CallableStatement 必须明确报错。

extern crate druid_core as druid;
use druid::core::{DruidError, PhysicalConnection, Value};
use druid::pool::DruidPool;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::Arc;

async fn sqlite_pool() -> DruidPool {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须初始化成功");
    assert_eq!(factory.max_connections(), Some(1));
    DruidPool::builder()
        .name("druid-core-sqlite")
        .driver_name("toasty-sqlite")
        .factory(Arc::new(factory))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(8)
        .build()
        .await
        .expect("真实 SQLite DruidPool 必须初始化成功")
}

#[tokio::test]
async fn sqlite_proves_pool_prepared_transaction_and_recycle_semantics() {
    let pool = sqlite_pool().await;
    let mut connection = pool.get().await.expect("必须取得 SQLite 池化连接");

    connection
        .exec(
            "CREATE TABLE account (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                balance INTEGER NOT NULL
            )",
            Vec::new(),
        )
        .await
        .expect("DDL 必须真实执行");

    let mut insert = connection
        .prepare_statement("INSERT INTO account(id, name, balance) VALUES (?, ?, ?)")
        .await
        .expect("必须创建真实 Toasty PreparedStatement");
    insert
        .exec(
            &mut connection,
            vec![
                Value::Int(1),
                Value::String("primary".to_string()),
                Value::Int(100),
            ],
        )
        .await
        .expect("prepared insert 必须真实执行");
    insert.close().expect("逻辑 PreparedStatement 必须关闭");
    drop(insert);

    let mut cached = connection
        .prepare_statement("INSERT INTO account(id, name, balance) VALUES (?, ?, ?)")
        .await
        .expect("相同 key 必须可从连接级缓存复用");
    assert_eq!(cached.prepared_statement_holder().hit_count(), 1);
    cached
        .exec(
            &mut connection,
            vec![
                Value::Int(2),
                Value::String("rollback".to_string()),
                Value::Int(50),
            ],
        )
        .await
        .expect("第二次 prepared insert 必须真实执行");
    cached.close().expect("缓存语句必须逻辑关闭");

    connection.begin().await.expect("事务必须开始");
    connection
        .exec(
            "UPDATE account SET balance = balance - ? WHERE id = ?",
            vec![Value::Int(20), Value::Int(1)],
        )
        .await
        .expect("事务内更新必须成功");
    connection.rollback().await.expect("事务必须回滚");

    let rows = connection
        .fetch(
            "SELECT id, name, balance FROM account ORDER BY id",
            Vec::new(),
        )
        .await
        .expect("查询必须真实返回行");
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![
                Value::Int(1),
                Value::String("primary".to_string()),
                Value::Int(100),
            ],
            vec![
                Value::Int(2),
                Value::String("rollback".to_string()),
                Value::Int(50),
            ],
        ]
    );

    let callable_error = connection
        .prepare_call("{call unsupported_on_sqlite(?)}")
        .await
        .expect_err("SQLite 不得伪造 CallableStatement");
    assert_eq!(
        callable_error,
        DruidError::UnsupportedOperation {
            operation: "prepare_physical_call"
        }
    );

    connection.close().await.expect("显式关闭必须归还连接");
    let mut reused = pool.get().await.expect("同一物理 SQLite 连接必须可复用");
    let rows = reused
        .fetch("SELECT COUNT(*) FROM account", Vec::new())
        .await
        .expect("复用后数据库状态必须保持");
    assert_eq!(rows[0].values, vec![Value::Int(2)]);
    reused.close().await.expect("复用连接必须归还");

    let state = pool.state();
    assert_eq!(state.active_count, 0);
    assert_eq!(state.idle_count, 1);
    assert_eq!(state.prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    pool.close().await;
}
