#![cfg(feature = "duckdb-native")]

//! DuckDB 原生 Adapter 的真实内存数据库合同。

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    DruidError, PhysicalConnection, PhysicalPreparedStatement, PreparedStatementKey,
    PreparedStatementMethodType, Value,
};
use druid_wrapper::driver::{DatabaseConnectionConfig, DruidDriverRegistry};
use druid_wrapper::duckdb::DuckDbConnectionAdapter;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[tokio::test]
async fn duckdb_registry_resolves_only_explicit_native_urls() {
    let registry = DruidDriverRegistry::builtin().expect("内置目录必须可解析");
    let config = DatabaseConnectionConfig::new("duckdb", "duckdb::memory:")
        .expect("DuckDB profile id 必须合法");
    let resolved = registry
        .resolve(&config)
        .expect("启用 feature 后 DuckDB native profile 必须可解析");

    assert_eq!(resolved.profile().id().as_str(), "duckdb");
    assert_eq!(resolved.factory().connection_url(), Some("duckdb::memory:"));

    let invalid = DuckDbConnectionAdapter::connect("sqlite::memory:").await;
    assert!(matches!(invalid, Err(DruidError::InvalidArgument(_))));
}

#[tokio::test]
async fn duckdb_adapter_preserves_scalar_types_and_native_prepare() {
    let mut connection = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("DuckDB 内存连接必须打开");
    connection
        .exec(
            "CREATE TABLE scalar_item (
                id BIGINT PRIMARY KEY,
                enabled BOOLEAN,
                ratio DOUBLE,
                amount DECIMAL(18, 4),
                business_date DATE,
                business_time TIME,
                created_at TIMESTAMP,
                label VARCHAR,
                payload BLOB
            )",
            Vec::new(),
        )
        .await
        .expect("DuckDB DDL 必须成功");

    let insert_key = PreparedStatementKey::new(
        Some("INSERT INTO scalar_item VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)".to_string()),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("prepare key 必须合法");
    let insert = connection
        .prepare_physical_statement(&insert_key)
        .await
        .expect("DuckDB 必须执行原生 prepare 校验");
    let date = NaiveDate::from_ymd_opt(2026, 8, 7).expect("测试日期必须合法");
    let time = NaiveTime::from_hms_micro_opt(9, 8, 7, 654_321).expect("测试时间必须合法");
    let timestamp = NaiveDateTime::new(date, time);
    let amount = BigDecimal::from_str("123456789.0123").expect("测试 Decimal 必须合法");
    let result = connection
        .exec_prepared(
            insert.as_ref(),
            vec![
                Value::Int(7),
                Value::Bool(true),
                Value::Float(3.25),
                Value::Decimal(amount.clone()),
                Value::Date(date),
                Value::Time(time),
                Value::Timestamp(timestamp),
                Value::String("druid-duckdb".to_string()),
                Value::Bytes(vec![0, 1, 2, 255]),
            ],
        )
        .await
        .expect("prepared insert 必须成功");
    assert_eq!(result.rows_affected, 1);

    let query_key = PreparedStatementKey::new(
        Some(
            "SELECT id, enabled, ratio, amount, business_date, business_time, created_at, label, payload FROM scalar_item WHERE id = ?"
                .to_string(),
        ),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("query key 必须合法");
    let query = connection
        .prepare_physical_statement(&query_key)
        .await
        .expect("prepared query 必须成功");
    let rows = connection
        .fetch_prepared(query.as_ref(), vec![Value::Int(7)])
        .await
        .expect("prepared fetch 必须成功");

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].values,
        vec![
            Value::Int(7),
            Value::Bool(true),
            Value::Float(3.25),
            Value::Decimal(amount),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(timestamp),
            Value::String("druid-duckdb".to_string()),
            Value::Bytes(vec![0, 1, 2, 255]),
        ]
    );
}

#[tokio::test]
async fn duckdb_adapter_transactions_metadata_and_lifecycle_are_explicit() {
    let mut connection = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("DuckDB 内存连接必须打开");
    assert_eq!(connection.driver_name(), "duckdb-rs");
    assert!(connection.capabilities().transactions);
    assert!(!connection.capabilities().savepoints);
    connection
        .exec("CREATE TABLE account(id BIGINT PRIMARY KEY)", Vec::new())
        .await
        .expect("建表必须成功");

    connection.begin().await.expect("事务必须开始");
    assert!(!connection.auto_commit());
    connection
        .exec("INSERT INTO account VALUES (?)", vec![Value::Int(1)])
        .await
        .expect("事务内插入必须成功");
    connection.rollback().await.expect("事务必须回滚");
    assert!(connection.auto_commit());
    let rows = connection
        .fetch("SELECT COUNT(*) FROM account", Vec::new())
        .await
        .expect("回滚后查询必须成功");
    assert_eq!(rows[0].values, vec![Value::Int(0)]);

    {
        let mut metadata = connection
            .database_meta_data()
            .expect("DuckDB metadata 必须可创建");
        assert_eq!(
            metadata
                .get_database_product_name()
                .await
                .unwrap()
                .as_deref(),
            Some("DuckDB")
        );
        assert_eq!(
            metadata.get_driver_name().await.unwrap().as_deref(),
            Some("duckdb-rs")
        );
        assert!(metadata
            .get_database_product_version()
            .await
            .expect("DuckDB 版本必须可查询")
            .is_some());
    }

    connection.ping().await.expect("存活检查必须成功");
    connection.close().await.expect("关闭必须成功");
    assert!(connection.is_closed());
    assert!(matches!(
        connection.ping().await,
        Err(DruidError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn duckdb_prepared_statement_is_bound_to_its_physical_connection() {
    let mut first = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("第一个连接必须打开");
    let mut second = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("第二个连接必须打开");
    let key = PreparedStatementKey::new(
        Some("SELECT ?".to_string()),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("prepare key 必须合法");
    let statement = first
        .prepare_physical_statement(&key)
        .await
        .expect("第一个连接必须可 prepare");

    let result = second
        .fetch_prepared(statement.as_ref(), vec![Value::Int(1)])
        .await;
    assert!(
        matches!(result, Err(DruidError::DriverError(message)) if message.contains("another physical connection"))
    );
}

#[tokio::test]
async fn duckdb_errors_preserve_sql_classification_fields() {
    let mut connection = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("DuckDB 内存连接必须打开");
    let error = connection
        .fetch("SELECT * FROM druid_missing_contract_table", Vec::new())
        .await
        .expect_err("不存在的表必须返回驱动错误");

    match error {
        DruidError::SqlException(exception) => {
            assert_ne!(exception.error_code(), 0);
            assert_eq!(exception.sql_state(), Some("HY000"));
            assert!(exception.class_name().starts_with("duckdb::Error"));
            assert!(exception
                .message()
                .is_some_and(|message| !message.is_empty()));
        }
        other => panic!("DuckDB 错误必须保留为 SqlException，实际为 {other:?}"),
    }
}

#[tokio::test]
async fn duckdb_prepared_statement_timeout_interrupts_the_native_query() {
    let mut connection = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("DuckDB 内存连接必须打开");
    let statement = prepare_long_running_statement(&mut connection).await;
    statement
        .set_query_timeout(1)
        .expect("DuckDB PreparedStatement 必须接受正查询超时");

    let error = tokio::time::timeout(
        Duration::from_secs(10),
        connection.fetch_prepared(statement.as_ref(), Vec::new()),
    )
    .await
    .expect("原生超时必须在上层测试截止时间内完成")
    .expect_err("长查询必须被查询超时中断");
    match error {
        DruidError::SqlException(exception) => {
            assert_eq!(exception.sql_state(), Some("HYT00"));
            assert_eq!(exception.class_name(), "java.sql.SQLTimeoutException");
        }
        other => panic!("查询超时必须保留结构化 SqlException，实际为 {other:?}"),
    }
    connection.ping().await.expect("查询超时后连接仍应可复用");
}

#[tokio::test]
async fn duckdb_prepared_statement_cancel_interrupts_the_native_query() {
    let mut connection = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("DuckDB 内存连接必须打开");
    let statement = prepare_long_running_statement(&mut connection).await;
    let executing_statement = Arc::clone(&statement);
    let cancel_statement = Arc::clone(&statement);
    let execution = tokio::spawn(async move {
        let result = connection
            .fetch_prepared(executing_statement.as_ref(), Vec::new())
            .await;
        (connection, result)
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel_statement
        .cancel()
        .expect("DuckDB PreparedStatement cancel 必须调用原生 interrupt");

    let (mut connection, result) = tokio::time::timeout(Duration::from_secs(10), execution)
        .await
        .expect("取消必须在上层测试截止时间内完成")
        .expect("取消任务不得 panic");
    match result.expect_err("长查询必须被显式 cancel 中断") {
        DruidError::SqlException(exception) => {
            assert_eq!(exception.sql_state(), Some("HY008"));
            assert_eq!(
                exception.class_name(),
                "duckdb::Error::OperationInterrupted"
            );
        }
        other => panic!("取消必须保留结构化 SqlException，实际为 {other:?}"),
    }
    connection.ping().await.expect("显式取消后连接仍应可复用");
}

#[tokio::test]
async fn duckdb_file_database_survives_physical_database_restart() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = format!(
        "druid_duckdb_restart_{}_{}.duckdb",
        std::process::id(),
        nanos
    );
    let path = std::env::current_dir()
        .expect("测试工作目录必须可读取")
        .join("target")
        .join(&file_name);
    let url = format!("duckdb:target/{file_name}");

    let mut first = DuckDbConnectionAdapter::connect(&url)
        .await
        .expect("DuckDB 文件数据库必须打开");
    first
        .exec(
            "CREATE TABLE restart_item(id BIGINT PRIMARY KEY)",
            Vec::new(),
        )
        .await
        .expect("重启契约表必须创建");
    first
        .exec("INSERT INTO restart_item VALUES (7)", Vec::new())
        .await
        .expect("重启前数据必须持久化");
    first.close().await.expect("第一个物理数据库必须关闭");

    let mut reopened = DuckDbConnectionAdapter::connect(&url)
        .await
        .expect("DuckDB 文件数据库必须重启");
    let rows = reopened
        .fetch("SELECT id FROM restart_item", Vec::new())
        .await
        .expect("重启后必须能读取持久化数据");
    assert_eq!(rows[0].values, vec![Value::Int(7)]);
    reopened
        .exec("DROP TABLE restart_item", Vec::new())
        .await
        .expect("重启契约表必须清理");
    reopened.close().await.expect("重启数据库必须关闭");
    std::fs::remove_file(path).expect("DuckDB 测试数据库文件必须可清理");
}

async fn prepare_long_running_statement(
    connection: &mut DuckDbConnectionAdapter,
) -> Arc<dyn PhysicalPreparedStatement> {
    let key = PreparedStatementKey::new(
        Some(
            "SELECT SUM(a.i * b.i) FROM range(1000000) AS a(i), range(1000000) AS b(i)".to_owned(),
        ),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("长查询 prepare key 必须合法");
    connection
        .prepare_physical_statement(&key)
        .await
        .expect("长查询必须可 prepare")
}
