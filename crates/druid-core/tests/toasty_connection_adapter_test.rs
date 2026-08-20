//! Toasty 内置数据源的真实 `SQLite` 契约测试。

extern crate druid_core as druid;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    DruidError, PhysicalConnectionFactory, PreparedStatementKey, PreparedStatementMethodType, Value,
};
use druid::toasty::ToastyConnectionFactory;
use std::str::FromStr;

async fn sqlite_connection() -> Box<dyn druid::core::PhysicalConnection> {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    factory
        .create()
        .await
        .expect("Toasty 必须创建未池化 SQLite 物理连接")
}

#[tokio::test]
async fn sqlite_factory_and_raw_sql_cover_druid_value_semantics() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    assert_eq!(factory.url(), "sqlite::memory:");
    assert_eq!(factory.driver_name(), "SQLite");
    assert_eq!(factory.max_connections(), Some(1));

    let mut connection = factory.create().await.expect("必须创建真实 SQLite 连接");
    assert_eq!(connection.driver_name(), "SQLite");
    assert!(connection.capabilities().transactions);
    assert!(connection.capabilities().savepoints);
    assert!(connection.capabilities().auto_commit);
    assert!(!connection.capabilities().read_only);
    factory
        .validate(&mut connection)
        .await
        .expect("新连接必须通过真实 ping");

    connection
        .exec(
            "CREATE TABLE typed_value (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                enabled BOOLEAN NOT NULL,
                amount REAL NOT NULL,
                name TEXT NOT NULL,
                payload BLOB NOT NULL,
                optional TEXT
            )",
            Vec::new(),
        )
        .await
        .expect("DDL 必须在真实 SQLite 执行");

    let inserted = connection
        .exec(
            "INSERT INTO typed_value(enabled, amount, name, payload, optional)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                Value::Bool(true),
                Value::Float(12.5),
                Value::String("toasty".to_string()),
                Value::Bytes(vec![1, 2, 3]),
                Value::Null,
            ],
        )
        .await
        .expect("全部 Druid Value 参数必须真实绑定");
    assert_eq!(inserted.rows_affected, 1);
    assert_eq!(inserted.last_insert_id, Some(1));

    let rows = connection
        .fetch(
            "SELECT id, enabled, amount, name, payload, optional
             FROM typed_value WHERE id = ?1",
            vec![Value::Int(1)],
        )
        .await
        .expect("真实 SQLite 查询必须成功");
    assert_eq!(rows.len(), 1);
    // Toasty RawSql::Infer 遵循 SQLite runtime storage class；BOOLEAN 在 SQLite
    // 中以 INTEGER 返回。这不是伪造 Rust bool，调用方需要按元数据解释。
    assert_eq!(
        rows[0].values,
        vec![
            Value::Int(1),
            Value::Int(1),
            Value::Float(12.5),
            Value::String("toasty".to_string()),
            Value::Bytes(vec![1, 2, 3]),
            Value::Null,
        ]
    );

    let key = PreparedStatementKey::new(
        Some("UPDATE typed_value SET name = ?1 WHERE id = ?2".to_string()),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("PreparedStatement key 必须合法");
    let statement = connection
        .prepare_physical_statement(&key)
        .await
        .expect("Toasty 必须创建物理预编译句柄");
    let updated = connection
        .exec_prepared(
            statement.as_ref(),
            vec![Value::String("prepared".to_string()), Value::Int(1)],
        )
        .await
        .expect("预编译更新必须真实执行");
    assert_eq!(updated.rows_affected, 1);
    connection
        .close_prepared_statement(statement.clone())
        .await
        .expect("预编译句柄必须可关闭");
    assert!(statement.is_closed());

    factory
        .close(&mut connection)
        .await
        .expect("工厂必须关闭物理连接");
    assert!(connection.is_closed());
    assert!(matches!(
        connection.ping().await,
        Err(DruidError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn sqlite_raw_sql_preserves_decimal_date_time_and_timestamp_types() {
    let mut connection = sqlite_connection().await;
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
        .expect("强类型 SQLite 表必须创建");

    let decimal = BigDecimal::from_str("1234567890.123456").unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_micro_opt(9, 8, 7, 654_321).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    connection
        .exec(
            "INSERT INTO strong_value VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                Value::Int(1),
                Value::Decimal(decimal.clone()),
                Value::Date(date),
                Value::Time(time),
                Value::Timestamp(timestamp),
            ],
        )
        .await
        .expect("Toasty 必须绑定全部强类型值");

    let rows = connection
        .fetch(
            "SELECT amount, event_date, event_time, event_at
             FROM strong_value WHERE id = ?1",
            vec![Value::Int(1)],
        )
        .await
        .expect("Toasty 必须读取全部强类型值");
    assert_eq!(
        rows[0].values,
        vec![
            Value::Decimal(decimal),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(timestamp),
        ]
    );
}

#[tokio::test]
async fn sqlite_transactions_savepoints_and_discard_are_real() {
    let mut connection = sqlite_connection().await;
    connection
        .exec(
            "CREATE TABLE tx_event (
                id INTEGER PRIMARY KEY,
                label TEXT NOT NULL
            )",
            Vec::new(),
        )
        .await
        .expect("事务测试表必须创建");

    connection
        .set_auto_commit(false)
        .await
        .expect("setAutoCommit(false) 必须真实 BEGIN");
    assert!(!connection.auto_commit());
    connection
        .exec(
            "INSERT INTO tx_event(id, label) VALUES (?1, ?2)",
            vec![Value::Int(1), Value::String("keep".to_string())],
        )
        .await
        .expect("事务内第一条数据必须写入");
    let savepoint = connection
        .set_savepoint_named("before_discard")
        .await
        .expect("必须创建真实命名保存点");
    connection
        .exec(
            "INSERT INTO tx_event(id, label) VALUES (?1, ?2)",
            vec![Value::Int(2), Value::String("discard".to_string())],
        )
        .await
        .expect("保存点后的数据必须写入");
    connection
        .rollback_to(&savepoint)
        .await
        .expect("必须真实回滚到保存点");
    connection
        .release_savepoint(&savepoint)
        .await
        .expect("必须真实释放保存点");
    connection
        .set_auto_commit(true)
        .await
        .expect("setAutoCommit(true) 必须真实 COMMIT");
    assert!(connection.auto_commit());

    let rows = connection
        .fetch("SELECT id, label FROM tx_event ORDER BY id", Vec::new())
        .await
        .expect("提交后必须可查询");
    assert_eq!(
        rows[0].values,
        vec![Value::Int(1), Value::String("keep".to_string())]
    );
    assert_eq!(rows.len(), 1);

    connection.begin().await.expect("第二个事务必须开始");
    connection
        .exec(
            "INSERT INTO tx_event(id, label) VALUES (?1, ?2)",
            vec![Value::Int(3), Value::String("rollback".to_string())],
        )
        .await
        .expect("回滚事务内写入必须成功");
    connection.rollback().await.expect("事务必须真实回滚");
    let rows = connection
        .fetch("SELECT COUNT(*) FROM tx_event", Vec::new())
        .await
        .expect("回滚后必须可查询");
    assert_eq!(rows[0].values, vec![Value::Int(1)]);

    assert!(matches!(
        connection.set_read_only(true).await,
        Err(DruidError::UnsupportedOperation {
            operation: "toasty_sqlite_read_only_transaction"
        })
    ));
    connection
        .set_transaction_isolation(8)
        .await
        .expect("SQLite Serializable 必须接受");
    assert_eq!(connection.transaction_isolation(), 8);
    assert!(matches!(
        connection.set_transaction_isolation(2).await,
        Err(DruidError::InvalidArgument(_))
    ));

    connection.begin().await.expect("保存点校验事务必须开始");
    assert!(matches!(
        connection.set_savepoint_named("unsafe-name").await,
        Err(DruidError::InvalidArgument(_))
    ));
    connection.rollback().await.expect("校验事务必须回滚");

    connection.mark_discarded();
    assert!(connection.is_discarded());
    assert!(matches!(
        connection.fetch("SELECT 1", Vec::new()).await,
        Err(DruidError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn unsupported_scheme_is_reported_instead_of_falling_back() {
    let error = ToastyConnectionFactory::new("unknown://database")
        .await
        .expect_err("未知 scheme 不得回退到其他驱动");
    assert!(matches!(error, DruidError::DriverError(_)));

    let error = ToastyConnectionFactory::new("dynamodb://local")
        .await
        .expect_err("DynamoDB 不得冒充 SQL 物理连接");
    assert_eq!(
        error,
        DruidError::UnsupportedOperation {
            operation: "toasty_non_sql_physical_connection"
        }
    );
}

#[tokio::test]
async fn sqlite_driver_failure_preserves_structured_sql_exception_boundary() {
    let mut connection = sqlite_connection().await;
    let error = connection
        .exec("THIS IS NOT VALID SQLITE", Vec::new())
        .await
        .expect_err("真实 SQLite 语法错误必须返回失败");

    let DruidError::SqlException(exception) = error else {
        panic!("Toasty driver operation failure 必须映射为结构化 SqlException");
    };
    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(
        exception.class_name(),
        "toasty_core::error::DriverOperationFailed"
    );
    assert!(
        exception
            .message()
            .is_some_and(|message| !message.is_empty()),
        "上游未公开 code/SQLState 时仍必须无损保留驱动消息"
    );
}
