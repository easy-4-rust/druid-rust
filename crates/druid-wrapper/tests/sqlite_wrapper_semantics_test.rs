//! `druid-wrapper` 的真实 `SQLite` 适配边界契约。

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    ConnectionRecycleDisposition, DruidError, DruidPooledConnection, ExceptionSorter,
    ExceptionSorterProperties, PhysicalConnection, PhysicalConnectionFactory, PhysicalResultSet,
    Pool, RowSetResultSet, SqlException, Value, Wrapper, WrapperExt,
};
use druid_wrapper::sqlx::bb8::SqlxBb8Pool;
use druid_wrapper::sqlx::deadpool::SqlxDeadpoolPool;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};
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
async fn direct_sqlx_wrapper_uses_real_sqlite_and_reports_callable_capability() {
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
            vec![Value::Int(1), Value::String("direct".to_string())],
        )
        .await
        .expect("参数绑定必须执行");
    let rows = connection
        .fetch("SELECT value FROM item WHERE id = ?", vec![Value::Int(1)])
        .await
        .expect("查询必须执行");
    assert_eq!(rows[0].values, vec![Value::String("direct".to_string())]);

    let callable = connection
        .prepare_physical_call(
            &druid::core::PreparedStatementKey::new(
                Some("{call sqlite_has_no_procedure()}".to_string()),
                None,
                druid::core::PreparedStatementMethodType::Precall1,
            )
            .unwrap(),
        )
        .await;
    assert!(matches!(
        callable,
        Err(DruidError::UnsupportedOperation {
            operation: "prepare_physical_call"
        })
    ));
    assert!(connection.capabilities().clear_warnings);
    assert_eq!(
        connection
            .warnings()
            .await
            .expect("存活的 SQLx SQLite 连接必须可读取 warning"),
        None
    );
    connection
        .clear_warnings()
        .await
        .expect("存活的 SQLx SQLite 连接必须可清理 warning");
    connection.close().await.expect("SQLx 物理连接必须显式关闭");
    assert!(connection.is_closed());
    assert!(matches!(
        connection.warnings().await,
        Err(DruidError::ConnectionDiscarded)
    ));
    assert!(matches!(
        connection.clear_warnings().await,
        Err(DruidError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn direct_sqlx_wrapper_preserves_sqlite_temporal_and_decimal_getter_semantics() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("必须创建真实 SQLite 连接");
    connection
        .exec(
            "CREATE TABLE strong_value (
                id INTEGER PRIMARY KEY,
                amount NUMERIC NOT NULL,
                event_date DATE NOT NULL,
                event_time TIME NOT NULL,
                event_at DATETIME NOT NULL
            )",
            Vec::new(),
        )
        .await
        .expect("强类型表必须创建");

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
        .expect("SQLx SQLite 必须绑定强类型值");
    let rows = connection
        .fetch(
            "SELECT amount, event_date, event_time, event_at
             FROM strong_value WHERE id = ?",
            vec![Value::Int(1)],
        )
        .await
        .expect("SQLx SQLite 必须读取强类型值");
    // SQLite NUMERIC affinity 将可表示的 Decimal 存为 REAL，SQLx 动态元数据
    // 只能诚实返回 Float。JDBC getBigDecimal 语义由 PhysicalResultSet 转换，
    // 不根据列名伪造 Value::Decimal。
    assert_eq!(rows[0].values[0], Value::Float(987654321.125));
    assert_eq!(rows[0].values[1], Value::Date(date));
    assert_eq!(rows[0].values[2], Value::Time(time));
    assert_eq!(rows[0].values[3], Value::Timestamp(timestamp));
    let result_set = RowSetResultSet::new(rows);
    assert!(result_set.next().unwrap());
    assert_eq!(result_set.big_decimal(1, None).unwrap(), Some(decimal));
}

#[tokio::test]
async fn external_pool_wrappers_return_real_sqlite_leases_to_their_owner() {
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
async fn pooled_wrapper_unwraps_the_real_sqlite_adapter_and_itself() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let physical = factory.create().await.expect("必须创建真实 SQLite 连接");
    let mut connection = DruidPooledConnection::new(physical, 7, Box::new(|_, _| {}));

    assert!(!connection.is_wrapper_for(None));
    assert!(connection.unwrap(None).is_none());
    assert!(connection.is_wrapper_for_type::<DruidPooledConnection>());
    assert!(connection.unwrap_ref::<DruidPooledConnection>().is_some());
    let pooled_unwrapped = connection
        .unwrap(Some(std::any::TypeId::of::<DruidPooledConnection>()))
        .expect("必须按池化连接具体类型解包");
    assert_eq!(format!("{pooled_unwrapped:?}"), "Unwrapped::Object");
    assert!(pooled_unwrapped.physical_connection().is_none());
    assert!(pooled_unwrapped.prepared_statement().is_none());
    assert!(pooled_unwrapped.callable_statement().is_none());
    assert!(connection.is_wrapper_for_type::<SqlxConnectionAdapter>());
    assert!(connection.unwrap_ref::<SqlxConnectionAdapter>().is_some());
    assert!(connection.is_wrapper_for_type::<dyn PhysicalConnection>());
    let physical_unwrapped = connection
        .unwrap(Some(std::any::TypeId::of::<dyn PhysicalConnection>()))
        .expect("必须按 Connection 接口解包");
    assert_eq!(
        format!("{physical_unwrapped:?}"),
        "Unwrapped::PhysicalConnection"
    );
    assert!(physical_unwrapped.physical_connection().is_some());
    assert!(physical_unwrapped.prepared_statement().is_none());
    assert!(physical_unwrapped.callable_statement().is_none());
    assert!(physical_unwrapped
        .downcast_ref::<SqlxConnectionAdapter>()
        .is_none());

    connection
        .exec(
            "CREATE TABLE wrapped_item(id INTEGER PRIMARY KEY)",
            Vec::new(),
        )
        .await
        .expect("解包验证后连接仍须真实执行 SQLite");
    connection.close().await.expect("池化连接必须可正常关闭");

    assert!(connection.is_wrapper_for_type::<DruidPooledConnection>());
    assert!(!connection.is_wrapper_for_type::<SqlxConnectionAdapter>());
    assert!(!connection.is_wrapper_for_type::<dyn PhysicalConnection>());
}

#[tokio::test]
async fn real_sqlite_database_error_flows_through_sorter_and_discards_physical_connection() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let physical = factory.create().await.expect("必须创建真实 SQLite 连接");
    let observed_disposition = Arc::new(Mutex::new(None));
    let disposition_for_callback = observed_disposition.clone();
    let mut connection = DruidPooledConnection::with_recycle_policy(
        physical,
        8,
        "sqlite-fatal-test".to_string(),
        None,
        false,
        None,
        Box::new(move |_physical, _id, disposition| {
            *disposition_for_callback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(disposition);
            false
        }),
    )
    .with_exception_sorter(Arc::new(SqliteDatabaseErrorSorter));

    let error = connection
        .exec("SELECT * FROM table_that_does_not_exist", Vec::new())
        .await
        .expect_err("真实 SQLite 必须返回数据库错误");
    let DruidError::SqlException(exception) = &error else {
        panic!("SQLx DatabaseError 必须保留为 SqlException，实际为 {error:?}");
    };
    assert_eq!(exception.class_name(), "sqlx::error::DatabaseError");
    assert!(exception.message().is_some_and(|message| {
        message.contains("table_that_does_not_exist") || message.contains("no such table")
    }));
    // Java handleFatalError 当场 discard；回调已经取得物理连接，逻辑对象不再
    // 保留一个仅标记 discard 的 holder。
    assert!(connection.connection_holder().is_none());
    assert!(matches!(
        *observed_disposition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        Some(ConnectionRecycleDisposition::Discard {
            recycle_error: None
        })
    ));

    connection
        .close()
        .await
        .expect("fatal 异常后的 close 不得吞吐错误");
    assert!(matches!(
        *observed_disposition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        Some(ConnectionRecycleDisposition::Discard {
            recycle_error: None
        })
    ));
}
