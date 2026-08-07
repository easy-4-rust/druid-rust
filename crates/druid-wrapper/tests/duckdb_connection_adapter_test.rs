#![cfg(feature = "duckdb-native")]

//! DuckDB 原生 Adapter 的真实内存数据库合同。

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    DruidError, PhysicalConnection, PreparedStatementKey, PreparedStatementMethodType, Value,
};
use druid_wrapper::driver::{DatabaseConnectionConfig, DruidDriverRegistry};
use druid_wrapper::duckdb::DuckDbConnectionAdapter;
use std::str::FromStr;

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
