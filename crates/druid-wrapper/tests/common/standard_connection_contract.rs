//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! 外部池与直连 `RDBC` 连接的共同可观察合同。

use druid::core::{
    DruidPooledConnection, ExceptionSorter, ExceptionSorterProperties, Connection,
    RdbcError, SqlException, Value,
};
use std::future::Future;
use std::sync::Arc;

/// 编译期要求 wrapper adapter 同时暴露 transitional Physical 与 canonical standard。
#[allow(
    dead_code,
    reason = "shared common module is compiled by pool tests that do not own raw adapters"
)]
pub fn assert_parallel_standard_contract<C, F>()
where
    C: druid::core::Connection + druid_rdbc::connection::Connection,
    F: druid::core::ConnectionFactory + druid_rdbc::driver::ConnectionFactory,
{
}

/// 将真实 SQLx SQLite 数据库错误归类为 fatal，验证租约必须立即 discard。
#[derive(Debug)]
struct SqliteDatabaseErrorSorter;

impl ExceptionSorter for SqliteDatabaseErrorSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        exception.class_name() == "sqlx::error::DatabaseError"
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

/// 断言获取的连接具备直连 SQLx 已声明的基础 `RDBC` 能力。
///
/// 该合同通过真实 SQLite 验证 PreparedStatement、Metadata、幂等关闭，以及关闭后
/// 再次获取的可用性。它还分别验证结构化数据库错误、fatal discard、显式 close 的
/// dirty transaction rollback，以及 Drop 后租约绝不以 dirty 状态泄漏。参数 `acquire`
/// 每次调用都必须从同一个连接拥有者获取连接。
pub async fn assert_rdbc_connection_contract<F, Fut>(acquire: F) -> Result<(), RdbcError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<DruidPooledConnection, RdbcError>>,
{
    let mut connection = acquire().await?;
    connection
        .exec(
            "create table item(id integer primary key, name text)",
            vec![],
        )
        .await?;
    let mut prepared = connection
        .prepare_statement("insert into item(name) values (?)")
        .await?;
    let result = prepared
        .exec(&mut connection, vec![Value::String("alpha".to_owned())])
        .await?;
    assert_eq!(result.rows_affected, 1);

    let mut metadata = connection.get_meta_data()?;
    assert_eq!(
        metadata.get_database_product_name().await?.as_deref(),
        Some("SQLite")
    );
    drop(metadata);

    connection.close().await?;
    connection.close().await?;

    let mut reused = acquire().await?;
    let rows = reused
        .fetch("select name from item where id = 1", vec![])
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, vec![Value::String("alpha".to_owned())]);
    reused.close().await?;

    assert_structured_error_and_fatal_reacquire(&acquire).await?;
    assert_explicit_close_rolls_back_dirty_transaction(&acquire).await?;
    assert_dirty_drop_never_leaks_transaction_state(&acquire).await?;
    Ok(())
}

/// 断言 driver 数据库错误保留 SQL 异常结构，fatal 分类会即时丢弃当前租约，且下一次
/// 获取仍可执行真实 SQLite SQL。
async fn assert_structured_error_and_fatal_reacquire<F, Fut>(acquire: &F) -> Result<(), RdbcError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<DruidPooledConnection, RdbcError>>,
{
    let mut connection = acquire().await?;
    connection.set_exception_sorter(Arc::new(SqliteDatabaseErrorSorter));
    connection
        .exec(
            "create table fatal_error_marker(id integer primary key, value text not null)",
            vec![],
        )
        .await?;
    connection
        .exec(
            "insert into fatal_error_marker(id, value) values (?, ?)",
            vec![
                Value::Int(1),
                Value::String("driver_lease_only".to_owned()),
            ],
        )
        .await?;
    let error = connection
        .exec("select * from definitely_missing_table", vec![])
        .await
        .expect_err("invalid SQL must fail");
    assert!(error.sql_exception().is_some());
    assert!(
        connection.connection_holder().is_none(),
        "fatal SQLite database errors must discard the current lease immediately"
    );

    connection.close().await?;
    connection.close().await?;

    let mut reacquired = acquire().await?;
    assert!(reacquired.auto_commit());
    let markers = reacquired
        .fetch(
            "select name from sqlite_master where type = 'table' and name = 'fatal_error_marker'",
            vec![],
        )
        .await?;
    assert!(
        markers.is_empty(),
        "fatal SQLite error must discard the driver lease instead of reusing its marker state"
    );
    let rows = reacquired.fetch("select 1", vec![]).await?;
    assert_eq!(rows.len(), 1, "reacquired lease must remain executable");
    reacquired.close().await?;
    Ok(())
}

/// 断言显式 close 会 rollback 未提交写入，并把干净 lease 交还给同一连接拥有者。
async fn assert_explicit_close_rolls_back_dirty_transaction<F, Fut>(
    acquire: &F,
) -> Result<(), RdbcError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<DruidPooledConnection, RdbcError>>,
{
    let mut connection = acquire().await?;
    connection
        .exec(
            "create table dirty_close_item(id integer primary key, value text not null)",
            vec![],
        )
        .await?;
    connection.begin().await?;
    connection
        .exec(
            "insert into dirty_close_item(id, value) values (?, ?)",
            vec![Value::Int(1), Value::String("must_rollback".to_owned())],
        )
        .await?;
    connection.close().await?;

    let mut reacquired = acquire().await?;
    assert!(reacquired.auto_commit());
    let rows = reacquired
        .fetch("select id from dirty_close_item", vec![])
        .await?;
    assert!(rows.is_empty(), "explicit close must rollback dirty writes");
    reacquired.close().await?;
    Ok(())
}

/// 断言 Drop 不会把未提交事务直接归还。当前 `sqlite::memory:` 合同要求丢弃 dirty
/// lease 并重建物理连接，因此下一租约既要回到 auto-commit，也不能再看见脏表。
async fn assert_dirty_drop_never_leaks_transaction_state<F, Fut>(
    acquire: &F,
) -> Result<(), RdbcError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<DruidPooledConnection, RdbcError>>,
{
    let mut connection = acquire().await?;
    connection
        .exec(
            "create table dirty_drop_item(id integer primary key, value text not null)",
            vec![],
        )
        .await?;
    connection.begin().await?;
    connection
        .exec(
            "insert into dirty_drop_item(id, value) values (?, ?)",
            vec![Value::Int(1), Value::String("must_not_leak".to_owned())],
        )
        .await?;
    let dropped_lease_id = connection.id();
    drop(connection);

    let mut reacquired = acquire().await?;
    assert_ne!(reacquired.id(), dropped_lease_id);
    assert!(reacquired.auto_commit());
    let tables = reacquired
        .fetch(
            "select name from sqlite_master where type = 'table' and name = 'dirty_drop_item'",
            vec![],
        )
        .await?;
    assert!(
        tables.is_empty(),
        "dirty Drop must discard and replace the physical SQLite lease"
    );
    reacquired.close().await?;
    Ok(())
}
