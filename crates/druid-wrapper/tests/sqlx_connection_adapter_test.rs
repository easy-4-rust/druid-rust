//! SQLx Adapter 真实 SQLite 驱动合同测试。

use druid::core::{PhysicalConnection, PreparedStatementKey, PreparedStatementMethodType, Value};
use druid::pool::DruidPool;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};
use std::sync::Arc;

async fn sqlite_pool() -> DruidPool {
    DruidPool::builder()
        .name("sqlite-contract")
        .driver_name("sqlx-sqlite")
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("SQLite pool must build")
}

#[tokio::test]
async fn sqlx_adapter_exec_fetch_and_type_mapping() {
    let pool = sqlite_pool().await;
    let mut connection = pool.get().await.expect("SQLite connection must open");
    assert_eq!(connection.driver_name(), "SQLite");

    connection
        .exec(
            "CREATE TABLE item (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                payload BLOB,
                score REAL,
                enabled BOOLEAN
            )",
            vec![],
        )
        .await
        .expect("table creation must succeed");
    let result = connection
        .exec(
            "INSERT INTO item(name, payload, score, enabled) VALUES (?, ?, ?, ?)",
            vec![
                Value::String("alpha".to_string()),
                Value::Bytes(vec![1, 2, 3]),
                Value::Float(9.5),
                Value::Bool(true),
            ],
        )
        .await
        .expect("insert must succeed");
    assert_eq!(result.rows_affected, 1);
    assert_eq!(result.last_insert_id, Some(1));

    let rows = connection
        .fetch(
            "SELECT id, name, payload, score, enabled FROM item WHERE name = ?",
            vec![Value::String("alpha".to_string())],
        )
        .await
        .expect("query must succeed");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].values,
        vec![
            Value::Int(1),
            Value::String("alpha".to_string()),
            Value::Bytes(vec![1, 2, 3]),
            Value::Float(9.5),
            Value::Bool(true),
        ]
    );

    let aggregate = connection
        .fetch("SELECT COUNT(*) FROM item", vec![])
        .await
        .expect("SQLite expression columns must use their runtime value type");
    assert_eq!(aggregate[0].values, vec![Value::Int(1)]);
}

#[tokio::test]
async fn sqlx_adapter_transaction_and_savepoint_semantics() {
    let pool = sqlite_pool().await;
    let mut connection = pool.get().await.expect("SQLite connection must open");
    connection
        .exec(
            "CREATE TABLE account(id INTEGER PRIMARY KEY, balance INTEGER)",
            vec![],
        )
        .await
        .expect("table creation must succeed");

    connection.begin().await.expect("transaction must begin");
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(1), Value::Int(10)],
        )
        .await
        .expect("first insert must succeed");
    let savepoint = connection
        .set_savepoint_named("after_first_insert")
        .await
        .expect("savepoint must be created");
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(2), Value::Int(20)],
        )
        .await
        .expect("second insert must succeed");
    connection
        .rollback_to(&savepoint)
        .await
        .expect("rollback to savepoint must succeed");
    connection
        .release_savepoint(&savepoint)
        .await
        .expect("savepoint release must succeed");
    connection.commit().await.expect("transaction must commit");

    let rows = connection
        .fetch("SELECT id FROM account ORDER BY id", vec![])
        .await
        .expect("verification query must succeed");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, vec![Value::Int(1)]);

    connection
        .begin()
        .await
        .expect("second transaction must begin");
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(3), Value::Int(30)],
        )
        .await
        .expect("third insert must succeed");
    connection
        .rollback()
        .await
        .expect("transaction must rollback");
    let rows = connection
        .fetch("SELECT id FROM account ORDER BY id", vec![])
        .await
        .expect("rollback verification query must succeed");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, vec![Value::Int(1)]);
}

#[tokio::test]
async fn sqlx_adapter_rejects_unsafe_savepoint_names() {
    let pool = sqlite_pool().await;
    let mut connection = pool.get().await.expect("SQLite connection must open");
    connection.begin().await.expect("transaction must begin");
    let result = connection.set_savepoint_named("bad;DROP_TABLE").await;
    assert!(result.is_err());
    connection
        .rollback()
        .await
        .expect("transaction must rollback");
}

#[tokio::test]
async fn sqlx_adapter_executes_and_reuses_real_prepared_statements() {
    let pool = DruidPool::builder()
        .name("sqlite-prepared-contract")
        .driver_name("sqlx-sqlite")
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(3)
        .build()
        .await
        .expect("SQLite prepared pool must build");
    let mut connection = pool.get().await.expect("SQLite connection must open");
    connection
        .exec(
            "CREATE TABLE prepared_item(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            vec![],
        )
        .await
        .expect("table creation must succeed");

    let insert_sql = "INSERT INTO prepared_item(id, name) VALUES (?, ?)";
    let mut first_insert = connection
        .prepare_statement(insert_sql)
        .await
        .expect("first prepare must succeed");
    first_insert
        .exec(
            &mut connection,
            vec![Value::Int(1), Value::String("first".to_string())],
        )
        .await
        .expect("first prepared insert must succeed");
    first_insert.close().expect("first statement must close");

    let mut second_insert = connection
        .prepare_statement(insert_sql)
        .await
        .expect("cached prepare must succeed");
    second_insert
        .exec(
            &mut connection,
            vec![Value::Int(2), Value::String("second".to_string())],
        )
        .await
        .expect("cached prepared insert must succeed");
    second_insert.close().expect("second statement must close");

    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    assert_eq!(state.cached_prepared_statement_miss_count, 1);
    assert_eq!(state.cached_prepared_statement_count, 1);

    let mut select = connection
        .prepare_statement("SELECT id, name FROM prepared_item ORDER BY id")
        .await
        .expect("select prepare must succeed");
    let rows = select
        .fetch(&mut connection, vec![])
        .await
        .expect("prepared select must succeed");
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].values,
        vec![Value::Int(1), Value::String("first".to_string())]
    );
    assert_eq!(
        rows[1].values,
        vec![Value::Int(2), Value::String("second".to_string())]
    );
    select.close().expect("select statement must close");

    connection
        .close()
        .await
        .expect("pooled connection must close");
    pool.close().await;
}

#[tokio::test]
async fn sqlx_adapter_rejects_a_closed_prepared_statement_handle() {
    let mut adapter = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("SQLite adapter must connect");
    let key = PreparedStatementKey::new(
        Some("SELECT 1".to_string()),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("prepared key must build");
    let statement = adapter
        .prepare_physical_statement(&key)
        .await
        .expect("physical prepare must succeed");
    adapter
        .close_prepared_statement(statement.clone())
        .await
        .expect("physical statement close must succeed");

    assert!(adapter
        .fetch_prepared(statement.as_ref(), vec![])
        .await
        .is_err());
}
