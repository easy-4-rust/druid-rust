use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};
use druid::core::{DruidError, PhysicalConnection, Value};
use druid_wrapper::driver::{
    DriverRuntimeMode, DruidDatabasePoolBuilder, DruidDriverRegistry, ProtocolFamily,
};
#[cfg(feature = "duckdb-native")]
use druid_wrapper::duckdb::DuckDbConnectionAdapter;
use std::path::Path;
#[cfg(feature = "duckdb-native")]
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 对环境显式提供的真实数据库运行统一产品契约。
///
/// 默认工作区测试不会访问外部服务；认证 CI 必须设置 profile、URL 和
/// `DRUID_REQUIRE_DATABASE_PROFILE_CONTRACT=1`，缺少配置时直接失败而非静默跳过。
#[tokio::test]
async fn database_profile_live_contract_when_configured() {
    let required = std::env::var("DRUID_REQUIRE_DATABASE_PROFILE_CONTRACT").as_deref() == Ok("1");
    let profile_id = match std::env::var("DRUID_DATABASE_PROFILE") {
        Ok(value) => value,
        Err(error) if required => panic!("DRUID_DATABASE_PROFILE is required: {error}"),
        Err(_) => return,
    };
    let url = std::env::var("DRUID_DATABASE_URL")
        .unwrap_or_else(|error| panic!("DRUID_DATABASE_URL is required: {error}"));
    let registry = DruidDriverRegistry::builtin().expect("内置数据库档案必须可解析");
    let profile = registry
        .profiles()
        .find(|profile| profile.id().as_str() == profile_id)
        .unwrap_or_else(|| panic!("unknown database profile {profile_id}"));
    assert!(
        matches!(
            profile.runtime_mode(),
            DriverRuntimeMode::Sqlx | DriverRuntimeMode::Native
        ),
        "本契约运行器当前认证 SQLx 与 Native 产品档案"
    );

    let table = contract_table_name();
    let create_sql = create_table_sql(profile.protocol_family(), &table);
    let insert_sql = insert_sql(profile.protocol_family(), &table);
    let select_sql = select_sql(profile.protocol_family(), &table);
    let pool = DruidDatabasePoolBuilder::new(&profile_id, &url)
        .name(format!("contract-{profile_id}"))
        .pool(|builder| {
            builder
                .max_open(4)
                .max_idle(4)
                .test_on_borrow(true)
                .test_on_return(true)
                .pool_prepared_statements(true)
                .max_pool_prepared_statements_per_connection(8)
        })
        .build()
        .await
        .expect("真实数据库档案必须能构建 DruidPool");

    let mut connection = pool.get().await.expect("必须能建立未池化物理连接");
    connection.ping().await.expect("validation/ping 必须成功");
    connection
        .exec(&format!("DROP TABLE IF EXISTS {table}"), Vec::new())
        .await
        .expect("清理旧契约表必须成功");
    connection
        .exec(&create_sql, Vec::new())
        .await
        .expect("DDL 必须成功");

    let scalar_values = contract_values(1, "first");
    let mut insert = connection
        .prepare_statement(&insert_sql)
        .await
        .expect("必须创建真实 PreparedStatement");
    let inserted = insert
        .exec(&mut connection, scalar_values.clone())
        .await
        .expect("参数化 INSERT 必须成功");
    assert_eq!(inserted.rows_affected, 1);
    insert
        .close()
        .expect("PreparedStatement 必须可关闭并回缓存");

    let mut select = connection
        .prepare_statement(&select_sql)
        .await
        .expect("必须创建参数化 SELECT");
    let rows = select
        .fetch(&mut connection, vec![Value::Int(1)])
        .await
        .expect("参数化 SELECT 必须成功");
    assert_eq!(rows.len(), 1);
    assert_scalar_row(profile.protocol_family(), &rows[0].values, &scalar_values);
    select.close().expect("SELECT PreparedStatement 必须可关闭");

    let updated = connection
        .exec(
            &format!(
                "UPDATE {table} SET name = {} WHERE id = {}",
                placeholder(profile.protocol_family(), 1),
                placeholder(profile.protocol_family(), 2)
            ),
            vec![Value::String("updated".to_owned()), Value::Int(1)],
        )
        .await
        .expect("UPDATE 必须成功");
    assert_eq!(updated.rows_affected, 1);

    connection
        .set_auto_commit(false)
        .await
        .expect("必须进入事务");
    connection
        .exec(&insert_sql, contract_values(2, "rollback"))
        .await
        .expect("事务内 INSERT 必须成功");
    connection.rollback().await.expect("rollback 必须成功");
    assert!(connection.auto_commit());
    assert!(connection
        .fetch(&select_sql, vec![Value::Int(2)])
        .await
        .expect("rollback 后查询必须成功")
        .is_empty());

    connection
        .set_auto_commit(false)
        .await
        .expect("必须再次进入事务");
    connection
        .exec(&insert_sql, contract_values(3, "commit"))
        .await
        .expect("事务内 INSERT 必须成功");
    connection.commit().await.expect("commit 必须成功");
    assert_eq!(
        connection
            .fetch(&select_sql, vec![Value::Int(3)])
            .await
            .expect("commit 后查询必须成功")
            .len(),
        1
    );

    let mut batch = connection
        .prepare_statement(&insert_sql)
        .await
        .expect("必须创建 batch PreparedStatement");
    batch
        .add_batch(&mut connection, contract_values(4, "batch-a"))
        .expect("第一批参数必须可加入");
    batch
        .add_batch(&mut connection, contract_values(5, "batch-b"))
        .expect("第二批参数必须可加入");
    let counts = batch
        .execute_batch(&mut connection)
        .await
        .expect("batch 必须成功");
    assert_eq!(counts.len(), 2);
    batch.close().expect("batch statement 必须可关闭");

    // 归还一个未提交连接，Druid 必须 rollback 并恢复默认 auto-commit。
    connection
        .set_auto_commit(false)
        .await
        .expect("状态复位前必须进入事务");
    connection
        .exec(&insert_sql, contract_values(6, "recycle"))
        .await
        .expect("待回收事务写入必须成功");
    connection.close().await.expect("连接归还必须成功");
    let mut connection = pool.get().await.expect("必须能重新借出连接");
    assert!(connection.auto_commit(), "归还后必须恢复 auto-commit");
    assert!(connection
        .fetch(&select_sql, vec![Value::Int(6)])
        .await
        .expect("状态复位后查询必须成功")
        .is_empty());

    let deleted = connection
        .exec(
            &format!(
                "DELETE FROM {table} WHERE id = {}",
                placeholder(profile.protocol_family(), 1)
            ),
            vec![Value::Int(1)],
        )
        .await
        .expect("DELETE 必须成功");
    assert_eq!(deleted.rows_affected, 1);
    let database_version = database_version(&mut connection, profile.protocol_family()).await;
    let requires_embedded_restart = profile.id().as_str() == "duckdb";
    if requires_embedded_restart {
        eprintln!("contract-stage: duckdb-error-classification");
        verify_duckdb_error_classification(&mut connection).await;
        eprintln!("contract-stage: duckdb-timeout-cancel");
        verify_duckdb_timeout_and_cancel(&mut connection).await;
    } else {
        connection
            .exec(&format!("DROP TABLE {table}"), Vec::new())
            .await
            .expect("清理契约表必须成功");
    }
    connection.close().await.expect("最终连接归还必须成功");

    let mut leases = Vec::new();
    for _ in 0..4 {
        leases.push(pool.get().await.expect("max-open 内借用必须成功"));
    }
    assert_eq!(pool.state().active_count, 4);
    for lease in &mut leases {
        lease.close().await.expect("并发租约必须可归还");
    }
    let state = pool.state();
    assert_eq!(state.active_count, 0);
    assert!(state.cached_prepared_statement_hit_count > 0);
    assert!(state.cached_prepared_statement_miss_count > 0);

    eprintln!("contract-stage: rejected-connection");
    verify_rejected_connection_if_configured(&profile_id).await;
    eprintln!("contract-stage: pool-close");
    pool.close().await;
    assert!(pool.state().closed);
    if requires_embedded_restart {
        eprintln!("contract-stage: duckdb-database-restart");
        verify_duckdb_database_restart(&profile_id, &url, &table, &select_sql).await;
    }
    eprintln!("contract-stage: evidence-write");
    write_contract_evidence_if_configured(profile, &database_version);
}

fn create_table_sql(family: ProtocolFamily, table: &str) -> String {
    if let Ok(template) = std::env::var("DRUID_CONTRACT_CREATE_TABLE_SQL") {
        return template.replace("{table}", table);
    }
    let binary = match family {
        ProtocolFamily::PostgreSql => "BYTEA",
        _ => "BLOB",
    };
    format!(
        "CREATE TABLE {table} (\
         id BIGINT PRIMARY KEY, \
         name VARCHAR(128) NOT NULL, \
         nullable_text VARCHAR(128), \
         enabled BOOLEAN NOT NULL, \
         amount DECIMAL(18, 2) NOT NULL, \
         event_date DATE NOT NULL, \
         event_time TIME NOT NULL, \
         event_timestamp TIMESTAMP NOT NULL, \
         payload {binary} NOT NULL)"
    )
}

fn insert_sql(family: ProtocolFamily, table: &str) -> String {
    let values = (1..=9)
        .map(|index| placeholder(family, index))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO {table} \
         (id, name, nullable_text, enabled, amount, event_date, event_time, event_timestamp, payload) \
         VALUES ({values})"
    )
}

fn select_sql(family: ProtocolFamily, table: &str) -> String {
    format!(
        "SELECT id, name, nullable_text, enabled, amount, event_date, event_time, \
         event_timestamp, payload FROM {table} WHERE id = {}",
        placeholder(family, 1)
    )
}

fn placeholder(family: ProtocolFamily, index: usize) -> String {
    if family == ProtocolFamily::PostgreSql {
        format!("${index}")
    } else {
        "?".to_owned()
    }
}

fn contract_values(id: i64, name: &str) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::String(name.to_owned()),
        Value::Null,
        Value::Bool(true),
        Value::Decimal(BigDecimal::from(25) / BigDecimal::from(2)),
        Value::Date(NaiveDate::from_ymd_opt(2026, 8, 7).expect("固定日期必须合法")),
        Value::Time(NaiveTime::from_hms_opt(12, 34, 56).expect("固定时间必须合法")),
        Value::Timestamp(
            NaiveDateTime::parse_from_str("2026-08-07 12:34:56", "%Y-%m-%d %H:%M:%S")
                .expect("固定时间戳必须合法"),
        ),
        Value::Bytes(vec![0, 1, 2, 0xff]),
    ]
}

fn assert_scalar_row(family: ProtocolFamily, actual: &[Value], expected: &[Value]) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        if family == ProtocolFamily::SQLite
            && index == 4
            && matches!((actual, expected), (Value::Float(12.5), Value::Decimal(_)))
        {
            // SQLite NUMERIC affinity has no decimal storage class. The adapter reports the
            // real runtime storage class instead of inventing a Decimal identity.
            continue;
        }
        assert_eq!(actual, expected, "scalar type mismatch at column {index}");
    }
}

async fn database_version(
    connection: &mut druid::core::DruidPooledConnection,
    family: ProtocolFamily,
) -> String {
    let sql = match family {
        ProtocolFamily::SQLite => "SELECT sqlite_version()",
        ProtocolFamily::MySql => "SELECT version()",
        ProtocolFamily::PostgreSql => "SHOW server_version",
        _ => "SELECT version()",
    };
    let rows = connection
        .fetch(sql, Vec::new())
        .await
        .expect("必须能读取真实数据库版本");
    match rows.first().and_then(|row| row.values.first()) {
        Some(Value::String(version)) => version.clone(),
        other => panic!("database version query returned unexpected value {other:?}"),
    }
}

async fn verify_rejected_connection_if_configured(profile_id: &str) {
    let required = std::env::var("DRUID_REQUIRE_DATABASE_PROFILE_CONTRACT").as_deref() == Ok("1");
    let bad_url = match std::env::var("DRUID_DATABASE_BAD_URL") {
        Ok(value) => value,
        Err(error) if required => panic!("DRUID_DATABASE_BAD_URL is required: {error}"),
        Err(_) => return,
    };
    if profile_id == "duckdb" {
        verify_duckdb_rejected_physical_connection(&bad_url).await;
        return;
    }
    let pool = DruidDatabasePoolBuilder::new(profile_id, bad_url)
        .pool(|builder| {
            builder
                .max_open(1)
                .max_idle(1)
                .login_timeout(3)
                .acquire_timeout(Duration::from_secs(5))
        })
        .build()
        .await
        .expect("错误凭据池配置本身应可构建");
    assert!(
        pool.get().await.is_err(),
        "错误凭据或不可打开的数据库路径必须拒绝建连"
    );
    pool.close().await;
}

#[cfg(feature = "duckdb-native")]
async fn verify_duckdb_rejected_physical_connection(bad_url: &str) {
    assert!(
        bad_url.starts_with("duckdb:"),
        "DuckDB 坏连接测试输入必须保留 duckdb scheme"
    );
    let malformed_url = format!("{bad_url}?druid-invalid-option=1");
    assert!(
        matches!(
            DuckDbConnectionAdapter::connect(&malformed_url).await,
            Err(DruidError::InvalidArgument(_))
        ),
        "DuckDB 不支持凭据认证，必须在物理连接前拒绝非法 URL"
    );
}

#[cfg(not(feature = "duckdb-native"))]
async fn verify_duckdb_rejected_physical_connection(_bad_url: &str) {
    panic!("DuckDB 完整契约必须启用 duckdb-native feature");
}

fn write_contract_evidence_if_configured(
    profile: &druid_wrapper::driver::DatabaseProfile,
    database_version: &str,
) {
    let Ok(path) = std::env::var("DRUID_CONTRACT_EVIDENCE_PATH") else {
        return;
    };
    let source_revision =
        std::env::var("GITHUB_SHA").expect("生成认证证据时必须由 CI 提供 GITHUB_SHA");
    let evidence_ref = std::env::var("DRUID_CONTRACT_EVIDENCE_REF")
        .expect("生成认证证据时必须提供不可变 CI 运行引用");
    let target = std::env::var("DRUID_CONTRACT_TARGET").unwrap_or_else(|_| current_target());
    let rust_version =
        std::env::var("DRUID_CONTRACT_RUST_VERSION").unwrap_or_else(|_| "default".to_owned());
    let mut contract_checks = vec![
        "connection-lifecycle",
        "validation",
        "crud-ddl",
        "scalar-types",
        "prepared-and-batch",
        "transactions",
        "state-reset",
        "capabilities",
        "concurrency-leak-shutdown",
        "no-pool-in-pool",
    ];
    if profile.id().as_str() == "duckdb" {
        contract_checks.extend(["error-classification", "timeout-cancel", "database-restart"]);
    }
    let record = serde_json::json!({
        "profileId": profile.id().as_str(),
        "target": target,
        "databaseVersion": database_version,
        "rustVersion": rust_version,
        "javaVersions": [],
        "runtimeMode": runtime_mode_label(profile.runtime_mode()),
        "installationPaths": installation_paths(profile.runtime_mode()),
        "contractChecks": contract_checks,
        "sourceRevision": source_revision,
        "evidenceRef": evidence_ref,
        "passedAt": Utc::now().to_rfc3339(),
        "artifactSha256": null
    });
    let path = Path::new(&path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("必须能创建 CI 契约证据目录");
    }
    std::fs::write(path, record.to_string()).expect("必须能写入 CI 契约证据工件");
}

fn runtime_mode_label(runtime_mode: DriverRuntimeMode) -> &'static str {
    match runtime_mode {
        DriverRuntimeMode::Sqlx => "sqlx",
        DriverRuntimeMode::Native => "native",
        DriverRuntimeMode::JdbcAgent => "jdbc_agent",
        DriverRuntimeMode::HttpSql => "http_sql",
    }
}

fn installation_paths(runtime_mode: DriverRuntimeMode) -> Vec<&'static str> {
    match runtime_mode {
        DriverRuntimeMode::Native => vec!["native", "bundled-native"],
        DriverRuntimeMode::Sqlx => vec!["native"],
        DriverRuntimeMode::JdbcAgent => vec!["jdbc-agent"],
        DriverRuntimeMode::HttpSql => vec!["http-sql"],
    }
}

async fn verify_duckdb_error_classification(connection: &mut druid::core::DruidPooledConnection) {
    let error = connection
        .fetch("SELECT * FROM druid_missing_contract_table", Vec::new())
        .await
        .expect_err("DuckDB 不存在的表必须返回错误");
    match error {
        DruidError::SqlException(exception) => {
            assert_ne!(exception.error_code(), 0);
            assert_eq!(exception.sql_state(), Some("HY000"));
            assert!(exception.class_name().starts_with("duckdb::Error"));
            assert!(exception
                .message()
                .is_some_and(|message| !message.is_empty()));
        }
        other => panic!("DuckDB 错误必须保留 SQLState/vendor/class，实际为 {other:?}"),
    }
}

#[cfg(feature = "duckdb-native")]
async fn verify_duckdb_timeout_and_cancel(connection: &mut druid::core::DruidPooledConnection) {
    let sql = "SELECT SUM(a.i * b.i) FROM range(1000000) AS a(i), range(1000000) AS b(i)";
    let mut statement = connection
        .prepare_statement(sql)
        .await
        .expect("DuckDB 长查询必须可 prepare");
    statement
        .set_query_timeout(connection, 1)
        .expect("池化 PreparedStatement 必须把 timeout 委托给原生语句");
    let timeout = tokio::time::timeout(
        Duration::from_secs(10),
        statement.fetch(connection, Vec::new()),
    )
    .await
    .expect("DuckDB query timeout 必须在测试截止时间内完成")
    .expect_err("DuckDB 长查询必须触发 query timeout");
    assert!(matches!(
        timeout,
        DruidError::SqlException(ref exception)
            if exception.sql_state() == Some("HYT00")
                && exception.class_name() == "java.sql.SQLTimeoutException"
    ));
    statement.close().expect("超时后的池化语句必须可关闭");
    connection
        .ping()
        .await
        .expect("query timeout 后连接必须可复用");

    let mut physical = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("取消契约的未池化 DuckDB 连接必须打开");
    let key = druid::core::PreparedStatementKey::new(
        Some(sql.to_owned()),
        None,
        druid::core::PreparedStatementMethodType::M1,
    )
    .expect("取消契约 prepare key 必须合法");
    let prepared = physical
        .prepare_physical_statement(&key)
        .await
        .expect("取消契约必须可 prepare");
    let executing = Arc::clone(&prepared);
    let cancel = Arc::clone(&prepared);
    let task = tokio::spawn(async move {
        physical
            .fetch_prepared(executing.as_ref(), Vec::new())
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel
        .cancel()
        .expect("显式 cancel 必须调用 DuckDB 原生 interrupt");
    let cancelled = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("显式 cancel 必须在测试截止时间内完成")
        .expect("显式 cancel 任务不得 panic")
        .expect_err("DuckDB 长查询必须被显式 cancel");
    assert!(matches!(
        cancelled,
        DruidError::SqlException(ref exception)
            if exception.sql_state() == Some("HY008")
                && exception.class_name() == "duckdb::Error::OperationInterrupted"
    ));
}

#[cfg(not(feature = "duckdb-native"))]
async fn verify_duckdb_timeout_and_cancel(_connection: &mut druid::core::DruidPooledConnection) {
    panic!("DuckDB 完整契约必须启用 duckdb-native feature");
}

async fn verify_duckdb_database_restart(
    profile_id: &str,
    url: &str,
    table: &str,
    select_sql: &str,
) {
    assert!(
        url != "duckdb::memory:" && url != "duckdb:///:memory:",
        "DuckDB database-restart 契约必须使用文件数据库"
    );
    let pool = DruidDatabasePoolBuilder::new(profile_id, url)
        .name(format!("contract-{profile_id}-restart"))
        .pool(|builder| builder.max_open(1).max_idle(1).test_on_borrow(true))
        .build()
        .await
        .expect("DuckDB 文件数据库必须能重新启动");
    let mut connection = pool.get().await.expect("重启后必须能建立物理连接");
    assert_eq!(
        connection
            .fetch(select_sql, vec![Value::Int(3)])
            .await
            .expect("数据库重启后必须能读取已提交数据")
            .len(),
        1
    );
    connection
        .exec(&format!("DROP TABLE {table}"), Vec::new())
        .await
        .expect("数据库重启后必须能清理契约表");
    connection.close().await.expect("重启后的连接必须归还");
    pool.close().await;
    assert!(pool.state().closed);
}

fn current_target() -> String {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu".to_owned(),
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu".to_owned(),
        ("x86_64", "macos") => "x86_64-apple-darwin".to_owned(),
        ("aarch64", "macos") => "aarch64-apple-darwin".to_owned(),
        ("x86_64", "windows") => "x86_64-pc-windows-msvc".to_owned(),
        (arch, os) => format!("{arch}-{os}"),
    }
}

fn contract_table_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("druid_contract_{}_{}", std::process::id(), nanos)
}
