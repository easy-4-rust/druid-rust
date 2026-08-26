//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! `druid-wrapper` 的真实 `SQLite` 适配边界契约。

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    Connection, ConnectionFactory, ConnectionRecycleDisposition, DruidConnectionHolder,
    DruidPooledConnection, ExceptionSorter, ExceptionSorterProperties, Pool, RdbcError,
    SqlException, Value, Wrapper,
};
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::wrapper::StandardUnwrapped;
use druid_wrapper::sqlx::bb8::SqlxBb8Pool;
use druid_wrapper::sqlx::deadpool::SqlxDeadpoolPool;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug)]
struct SqliteDatabaseErrorSorter;

impl ExceptionSorter for SqliteDatabaseErrorSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        exception.class_name() == "sqlx::error::DatabaseError"
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

#[tokio::test]
async fn task6_sqlite_wrapper_semantics_test_direct_sqlx_wrapper_uses_real_sqlite_and_reports_callable_capability(
) {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = factory.create().await.expect("必须创建真实 SQLite 连接");
    connection
        .exec(
            "CREATE TABLE item(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            Vec::new(),
        )
        .await
        .expect("DDL 必须执行");
    connection
        .exec(
            "INSERT INTO item(id, value) VALUES (?, ?)",
            vec![Value::Int(1), Value::String("direct".to_owned())],
        )
        .await
        .expect("参数绑定必须执行");
    let rows = connection
        .fetch("SELECT value FROM item WHERE id = ?", vec![Value::Int(1)])
        .await
        .expect("查询必须执行");
    assert_eq!(rows[0].values, vec![Value::String("direct".to_owned())]);
    let callable = connection
        .prepare_call(
            "{call sqlite_has_no_procedure()}",
            StatementOptions::default(),
        )
        .await;
    assert!(matches!(
        callable,
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_prepare_call"
        })
    ));
    assert!(connection.capabilities().transactions);
    assert_eq!(connection.warnings().await.unwrap(), None);
    let warning_clear = connection.clear_warnings().await;
    assert!(warning_clear.is_ok());
    connection.ping().await.expect("存活连接必须可 ping");
    connection.close().await.unwrap();
    assert!(connection.is_closed().unwrap());
    assert!(matches!(
        connection.warnings().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        connection.clear_warnings().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn task6_sqlite_wrapper_semantics_test_direct_sqlx_wrapper_preserves_sqlite_temporal_and_decimal_getter_semantics(
) {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = factory.create().await.expect("必须创建真实 SQLite 连接");
    connection
        .exec(
            "CREATE TABLE strong_value (id INTEGER PRIMARY KEY, amount NUMERIC NOT NULL, event_date DATE NOT NULL, event_time TIME NOT NULL, event_at DATETIME NOT NULL)",
            Vec::new(),
        )
        .await
        .unwrap();
    let decimal = BigDecimal::from_str("987654321.125").unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_micro_opt(18, 17, 16, 456_789).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    connection
        .exec(
            "INSERT INTO strong_value VALUES (?, ?, ?, ?, ?)",
            vec![
                Value::Int(1),
                Value::Decimal(decimal.clone()),
                Value::Date(date),
                Value::Time(time),
                Value::Timestamp(timestamp),
            ],
        )
        .await
        .unwrap();
    let rows = connection
        .fetch(
            "SELECT amount, event_date, event_time, event_at FROM strong_value WHERE id = ?",
            vec![Value::Int(1)],
        )
        .await
        .unwrap();
    assert_eq!(rows[0].values[0], Value::Float(987654321.125));
    assert_eq!(rows[0].values[1], Value::Date(date));
    assert_eq!(rows[0].values[2], Value::Time(time));
    assert_eq!(rows[0].values[3], Value::Timestamp(timestamp));
    let mut result_set = BufferedRowSet::new(rows);
    assert!(result_set.next().await.unwrap());
    assert_eq!(
        result_set
            .big_decimal(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Some(decimal)
    );
}

#[tokio::test]
async fn task6_sqlite_wrapper_semantics_test_external_pool_wrappers_return_real_sqlite_leases_to_their_owner(
) {
    let bb8 = SqlxBb8Pool::connect(
        "wrapper-bb8",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .expect("bb8 SQLite bridge 必须初始化");
    let mut bb8_connection = bb8.get().await.expect("bb8 必须返回连接");
    bb8_connection
        .exec("CREATE TABLE bb8_item(id INTEGER PRIMARY KEY)", Vec::new())
        .await
        .expect("bb8 lease 必须真实执行 SQLite");
    bb8_connection.close().await.expect("bb8 lease 必须归还");
    assert_eq!(bb8.state().recycle_count, 1);

    let deadpool = SqlxDeadpoolPool::connect(
        "wrapper-deadpool",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .expect("deadpool SQLite bridge 必须初始化");
    let mut deadpool_connection = deadpool.get().await.expect("deadpool 必须返回连接");
    deadpool_connection
        .exec(
            "CREATE TABLE deadpool_item(id INTEGER PRIMARY KEY)",
            Vec::new(),
        )
        .await
        .expect("deadpool lease 必须真实执行 SQLite");
    deadpool_connection
        .close()
        .await
        .expect("deadpool lease 必须归还");
    assert_eq!(deadpool.state().recycle_count, 1);
}

#[tokio::test]
async fn task6_sqlite_wrapper_semantics_test_pooled_wrapper_unwraps_the_real_sqlite_adapter_and_itself(
) {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let connection = factory.create().await.expect("必须创建真实 SQLite 连接");
    let holder = DruidConnectionHolder::with_standard_connection(connection, 7, Duration::ZERO, 0)
        .await
        .unwrap();
    let returned = Arc::new(Mutex::new(Vec::new()));
    let returned_for_callback = Arc::clone(&returned);
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "sqlite-wrapper".to_owned(),
        None,
        false,
        false,
        Box::new(move |_, disposition| {
            returned_for_callback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(disposition);
            true
        }),
    );
    assert!(matches!(
        connection.is_wrapper_for(None),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        connection.unwrap(None),
        Err(RdbcError::InvalidArgument(_))
    ));
    let target = Some(std::any::TypeId::of::<dyn Connection>());
    assert_eq!(connection.is_wrapper_for(target), Ok(true));
    assert!(matches!(
        connection.unwrap(target).unwrap(),
        StandardUnwrapped::Connection(_)
    ));
    assert_eq!(
        connection.is_wrapper_for(Some(std::any::TypeId::of::<dyn Wrapper>())),
        Ok(true)
    );
    assert_eq!(
        connection.is_wrapper_for(Some(std::any::TypeId::of::<
            dyn druid_rdbc::statement::Statement,
        >())),
        Ok(false)
    );
    assert_eq!(
        connection.is_wrapper_for(Some(std::any::TypeId::of::<
            dyn druid_rdbc::statement::PreparedStatement,
        >())),
        Ok(false)
    );
    assert_eq!(
        connection.is_wrapper_for(Some(std::any::TypeId::of::<
            dyn druid_rdbc::statement::CallableStatement,
        >())),
        Ok(false)
    );
    assert_eq!(
        connection.is_wrapper_for(Some(std::any::TypeId::of::<
            dyn druid_rdbc::result_set::ResultSet,
        >())),
        Ok(false)
    );
    assert_eq!(connection.is_closed(), Ok(false));
    assert!(matches!(
        connection.unwrap(Some(std::any::TypeId::of::<
            dyn druid_rdbc::statement::Statement,
        >())),
        Err(RdbcError::SqlException(ref exception))
            if exception.sql_state() == Some("0A000")
                && exception.class_name() == "java.sql.SQLFeatureNotSupportedException"
    ));
    assert!(matches!(
        connection.unwrap(Some(std::any::TypeId::of::<
            dyn druid_rdbc::result_set::ResultSet,
        >())),
        Err(RdbcError::SqlException(ref exception))
            if exception.sql_state() == Some("0A000")
                && exception.class_name() == "java.sql.SQLFeatureNotSupportedException"
    ));
    assert!(!connection.driver_name().unwrap().is_empty());
    connection
        .exec(
            "CREATE TABLE wrapped_item(id INTEGER PRIMARY KEY)",
            Vec::new(),
        )
        .await
        .unwrap();
    connection.close().await.unwrap();
    assert_eq!(returned.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn task6_sqlite_wrapper_semantics_test_real_sqlite_database_error_flows_through_sorter_and_discards_standard_connection(
) {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let connection = factory.create().await.expect("必须创建真实 SQLite 连接");
    let holder = DruidConnectionHolder::with_standard_connection(connection, 8, Duration::ZERO, 0)
        .await
        .unwrap();
    let observed_dispositions = Arc::new(Mutex::new(Vec::new()));
    let dispositions_for_observer = Arc::clone(&observed_dispositions);
    let returned_dispositions = Arc::new(Mutex::new(Vec::new()));
    let dispositions_for_return = Arc::clone(&returned_dispositions);
    let mut connection = DruidPooledConnection::with_standard_holder_and_disposition_observer(
        holder,
        "sqlite-fatal-test".to_owned(),
        None,
        false,
        false,
        Box::new(move |disposition| {
            dispositions_for_observer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(disposition);
        }),
        Box::new(move |_, disposition| {
            dispositions_for_return
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(disposition);
            false
        }),
    )
    .with_exception_sorter(Arc::new(SqliteDatabaseErrorSorter));
    let error = connection
        .exec("SELECT * FROM table_that_does_not_exist", Vec::new())
        .await
        .expect_err("真实 SQLite 必须返回数据库错误");
    let RdbcError::SqlException(exception) = &error else {
        panic!("SQLx DatabaseError 必须保留为 SqlException，实际为 {error:?}");
    };
    assert_eq!(exception.class_name(), "sqlx::error::DatabaseError");
    assert!(exception.message().is_some_and(|message| {
        message.contains("table_that_does_not_exist") || message.contains("no such table")
    }));
    assert!(matches!(
        observed_dispositions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        [ConnectionRecycleDisposition::Discard {
            recycle_error: None
        }]
    ));
    assert!(returned_dispositions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_empty());
    assert!(matches!(
        connection.is_closed(),
        Ok(true) | Err(RdbcError::ConnectionDiscarded)
    ));
    connection.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(
        observed_dispositions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len(),
        1
    );
    assert!(matches!(
        returned_dispositions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        [ConnectionRecycleDisposition::Discard {
            recycle_error: None
        }]
    ));
    let mut reacquired = factory.create().await.unwrap();
    assert!(reacquired.ping().await.is_ok());
    let rows = reacquired.fetch("SELECT 1", Vec::new()).await.unwrap();
    assert_eq!(rows.len(), 1);
    reacquired.close().await.unwrap();
}
