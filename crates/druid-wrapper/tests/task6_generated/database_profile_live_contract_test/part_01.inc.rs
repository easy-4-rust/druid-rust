// Generated from the pinned Task 6 BASE test denominator.
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};
use druid::core::{
    Connection, RdbcError, Value,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{
    ParameterIndex, PreparedParameter, PreparedStatement, Statement, StatementOptions,
};
use druid_wrapper::driver::{
    DatabaseConnectionConfig, DriverRuntimeMode, DruidDatabasePoolBuilder, DruidDriverRegistry,
    ProtocolFamily,
};
#[cfg(feature = "duckdb-native")]
use druid_wrapper::duckdb::DuckDbConnectionAdapter;
#[cfg(feature = "jdbc-agent")]
use druid_wrapper::jdbc_agent::{JdbcAgentConnection, JdbcAgentOptions};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 对环境显式提供的真实数据库运行统一产品契约。
///
/// 默认工作区测试不会访问外部服务；认证 CI 必须设置 profile、URL 和
/// `DRUID_REQUIRE_DATABASE_PROFILE_CONTRACT=1`，缺少配置时直接失败而非静默跳过。
#[tokio::test]
async fn task6_database_profile_live_contract_test_database_profile_live_contract_when_configured() {
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
    let table = contract_table_name();
    let create_sql = create_table_sql(profile.protocol_family(), &table);
    let insert_sql = insert_sql(profile.protocol_family(), &table);
    let select_sql = select_sql(profile.protocol_family(), &table);
    let pool = configured_pool_builder(&profile_id, &url, profile.runtime_mode())
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
    let physical_capabilities = connection.capabilities();
    let declared_capabilities = profile.capabilities();
    assert!(
        !declared_capabilities.transactions || physical_capabilities.transactions,
        "产品目录不得声明物理适配器未提供的事务能力"
    );
    assert!(
        !declared_capabilities.savepoints || physical_capabilities.savepoints,
        "产品目录不得声明物理适配器未提供的保存点能力"
    );
    assert!(
        !declared_capabilities.auto_commit || connection.auto_commit().is_ok(),
        "产品目录不得声明 canonical 适配器未提供的 auto-commit 能力"
    );
    let supports_transactions = physical_capabilities.transactions;
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
        .prepare_statement(&insert_sql, StatementOptions::default())
        .await
        .expect("必须创建真实 PreparedStatement");
    bind_prepared_values(insert.as_mut(), scalar_values.clone());
    let inserted = insert
        .execute_prepared_update()
        .await
        .expect("参数化 INSERT 必须成功");
    assert_eq!(inserted, 1);
    insert
        .close()
        .await
        .expect("PreparedStatement 必须可关闭并回缓存");

    let mut select = connection
        .prepare_statement(&select_sql, StatementOptions::default())
        .await
        .expect("必须创建参数化 SELECT");
    bind_prepared_values(select.as_mut(), vec![Value::Int(1)]);
    let mut result_set = select
        .execute_prepared_query()
        .await
        .expect("参数化 SELECT 必须成功");
    assert!(result_set.next().await.expect("SELECT 必须返回一行"));
    let actual = (1..=scalar_values.len())
        .map(|index| result_set.value(ColumnIndex::new(index).unwrap()).unwrap())
        .collect::<Vec<_>>();
    assert_scalar_row(profile.protocol_family(), &actual, &scalar_values);
    result_set.close().await.expect("ResultSet 必须可关闭");
    select.close().await.expect("SELECT PreparedStatement 必须可关闭");

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

    if supports_transactions {
        connection
            .set_auto_commit(false)
            .await
            .expect("必须进入事务");
        connection
            .exec(&insert_sql, contract_values(2, "rollback"))
            .await
            .expect("事务内 INSERT 必须成功");
        connection.rollback().await.expect("rollback 必须成功");
        connection
            .set_auto_commit(true)
            .await
            .expect("rollback 后必须恢复 auto-commit");
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
        connection
            .set_auto_commit(true)
            .await
            .expect("commit 后必须恢复 auto-commit");
        assert_eq!(
            connection
                .fetch(&select_sql, vec![Value::Int(3)])
                .await
                .expect("commit 后查询必须成功")
                .len(),
            1
        );

        if physical_capabilities.savepoints {
            connection
                .set_auto_commit(false)
                .await
                .expect("保存点合同必须进入事务");
            let savepoint = connection
                .set_savepoint_named("druid_contract_savepoint")
                .await
                .expect("声明保存点能力的驱动必须创建命名保存点");
            connection
                .exec(&insert_sql, contract_values(7, "savepoint"))
                .await
                .expect("保存点之后的写入必须成功");
            connection
                .rollback_to(&savepoint)
                .await
                .expect("声明保存点能力的驱动必须回滚到保存点");
            connection
                .release_savepoint(&savepoint)
                .await
                .expect("声明保存点能力的驱动必须释放保存点");
            connection.commit().await.expect("保存点事务必须可提交");
            connection
                .set_auto_commit(true)
                .await
                .expect("保存点事务后必须恢复 auto-commit");
            assert!(connection
                .fetch(&select_sql, vec![Value::Int(7)])
                .await
                .expect("保存点回滚后查询必须成功")
                .is_empty());
        }
    } else {
        assert!(
            connection.set_auto_commit(false).await.is_err(),
            "不支持事务的产品必须明确拒绝 setAutoCommit(false)"
        );
    }

    let mut batch = connection
        .prepare_statement(&insert_sql, StatementOptions::default())
        .await
        .expect("必须创建 batch PreparedStatement");
    batch
        .add_parameter_batch(prepared_values(contract_values(4, "batch-a")))
        .expect("第一批参数必须可加入");
    batch
        .add_parameter_batch(prepared_values(contract_values(5, "batch-b")))
        .expect("第二批参数必须可加入");
    let counts = batch
        .execute_prepared_batch()
        .await
        .expect("batch 必须成功");
    assert_eq!(counts.len(), 2);
    batch.close().await.expect("batch statement 必须可关闭");

    // 归还一个未提交连接，Druid 必须 rollback 并恢复默认 auto-commit。
    if supports_transactions {
        connection
            .set_auto_commit(false)
            .await
            .expect("状态复位前必须进入事务");
        connection
            .exec(&insert_sql, contract_values(6, "recycle"))
            .await
            .expect("待回收事务写入必须成功");
    }
    connection.close().await.expect("连接归还必须成功");
    let mut connection = pool.get().await.expect("必须能重新借出连接");
    if supports_transactions {
        assert!(connection.auto_commit().unwrap(), "归还后必须恢复 auto-commit");
        assert!(connection
            .fetch(&select_sql, vec![Value::Int(6)])
            .await
            .expect("状态复位后查询必须成功")
            .is_empty());
    }

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
    eprintln!("contract-stage: error-classification");
    verify_error_classification(&mut connection, &table).await;
    let is_duckdb = profile.id().as_str() == "duckdb";
    let requires_embedded_restart = matches!(profile.id().as_str(), "duckdb" | "sqlite");
    if is_duckdb {
        eprintln!("contract-stage: duckdb-timeout-cancel");
        verify_duckdb_timeout_and_cancel(&mut connection).await;
    }
    eprintln!("contract-stage: final-lease-close");
    tokio::time::timeout(Duration::from_secs(30), connection.close())
        .await
        .expect("最终连接归还必须在截止时间内完成")
        .expect("最终连接归还必须成功");

    let mut leases = Vec::new();
    for index in 0..4 {
        eprintln!("contract-stage: pool-lease-{index}");
        leases.push(
            tokio::time::timeout(Duration::from_secs(30), pool.get())
                .await
                .expect("max-open 内借用必须在截止时间内完成")
                .expect("max-open 内借用必须成功"),
        );
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
    let timeout_cancel = if is_duckdb {
        true
    } else {
        eprintln!("contract-stage: timeout-cancel");
        verify_timeout_and_cancel_if_configured(profile, &url).await
    };
    let jdbc_faults = verify_jdbc_agent_failures_if_required(profile, &url).await;
    eprintln!("contract-stage: pool-close");
    pool.close().await;
    assert!(pool.state().closed);
    eprintln!("contract-stage: database-restart");
    let database_restart = verify_database_restart_if_configured(
        profile,
        &url,
        &table,
        &select_sql,
        requires_embedded_restart,
    )
    .await;
    eprintln!("contract-stage: evidence-write");
    write_contract_evidence_if_configured(
        profile,
        &database_version,
        timeout_cancel,
        database_restart,
        jdbc_faults,
    );
}

fn configured_pool_builder(
    profile_id: &str,
    url: &str,
    runtime_mode: DriverRuntimeMode,
) -> DruidDatabasePoolBuilder {
    let mut builder = DruidDatabasePoolBuilder::new(profile_id, url);
    for (environment, property) in [
        ("DRUID_DATABASE_USER", "user"),
        ("DRUID_DATABASE_PASSWORD", "password"),
        ("DRUID_DATABASE_TOKEN", "token"),
        ("DRUID_DATABASE_API_TOKEN", "api_token"),
    ] {
        if let Some(value) = std::env::var(environment)
            .ok()
            .filter(|value| !value.is_empty())
        {
            builder = builder.property(property, value);
        }
    }
    if let Some(properties) = std::env::var("DRUID_DATABASE_PROPERTIES_JSON")
        .ok()
        .filter(|value| !value.is_empty())
    {
        let properties: std::collections::HashMap<String, String> =
            serde_json::from_str(&properties)
                .expect("DRUID_DATABASE_PROPERTIES_JSON 必须是字符串键值对象");
        for (name, value) in properties {
            builder = builder.property(name, value);
        }
    }
    if runtime_mode == DriverRuntimeMode::JdbcAgent {
        builder = configure_jdbc_agent(builder);
    }
    builder
}

#[cfg(feature = "jdbc-agent")]
fn configure_jdbc_agent(builder: DruidDatabasePoolBuilder) -> DruidDatabasePoolBuilder {
    builder.jdbc_agent(configured_jdbc_agent_options())
}

#[cfg(feature = "jdbc-agent")]
fn configured_jdbc_agent_options() -> JdbcAgentOptions {
    let java = std::env::var_os("DRUID_JDBC_JAVA")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| std::ffi::OsString::from("java"));
    let agent = std::env::var_os("DRUID_JDBC_AGENT_JAR")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("JDBC 产品合同必须设置 DRUID_JDBC_AGENT_JAR"));
    let drivers = std::env::var_os("DRUID_JDBC_DRIVER_JARS")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("DRUID_JDBC_DRIVER_JAR"))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("JDBC 产品合同必须设置 DRUID_JDBC_DRIVER_JARS"));
    let class_path = std::iter::once(std::path::PathBuf::from(agent))
        .chain(std::env::split_paths(&drivers))
        .collect::<Vec<_>>();
    JdbcAgentOptions::java(java, class_path).expect("JDBC Agent classpath 必须可在当前平台编码")
}

#[cfg(not(feature = "jdbc-agent"))]
fn configure_jdbc_agent(_builder: DruidDatabasePoolBuilder) -> DruidDatabasePoolBuilder {
    panic!("JDBC 产品合同必须启用 jdbc-agent feature")
}

fn configured_connection_config(profile_id: &str, url: &str) -> DatabaseConnectionConfig {
    let mut config =
        DatabaseConnectionConfig::new(profile_id, url).expect("合同 profile ID 必须合法");
    for (environment, property) in [
        ("DRUID_DATABASE_USER", "user"),
        ("DRUID_DATABASE_PASSWORD", "password"),
        ("DRUID_DATABASE_TOKEN", "token"),
        ("DRUID_DATABASE_API_TOKEN", "api_token"),
    ] {
        if let Some(value) = std::env::var(environment)
            .ok()
            .filter(|value| !value.is_empty())
        {
            config = config.property(property, value);
        }
    }
    if let Some(properties) = std::env::var("DRUID_DATABASE_PROPERTIES_JSON")
        .ok()
        .filter(|value| !value.is_empty())
    {
        let properties: std::collections::HashMap<String, String> =
            serde_json::from_str(&properties)
                .expect("DRUID_DATABASE_PROPERTIES_JSON 必须是字符串键值对象");
        for (name, value) in properties {
            config = config.property(name, value);
        }
    }
    config
}

#[cfg(feature = "jdbc-agent")]
fn configured_properties() -> std::collections::HashMap<String, String> {
    let mut properties = std::collections::HashMap::new();
    for (environment, property) in [
        ("DRUID_DATABASE_USER", "user"),
        ("DRUID_DATABASE_PASSWORD", "password"),
        ("DRUID_DATABASE_TOKEN", "token"),
        ("DRUID_DATABASE_API_TOKEN", "api_token"),
    ] {
        if let Some(value) = std::env::var(environment)
            .ok()
            .filter(|value| !value.is_empty())
        {
            properties.insert(property.to_owned(), value);
        }
    }
    if let Some(extra) = std::env::var("DRUID_DATABASE_PROPERTIES_JSON")
        .ok()
        .filter(|value| !value.is_empty())
    {
        properties.extend(
            serde_json::from_str::<std::collections::HashMap<String, String>>(&extra)
                .expect("DRUID_DATABASE_PROPERTIES_JSON 必须是字符串键值对象"),
        );
    }
    properties
}

fn create_table_sql(family: ProtocolFamily, table: &str) -> String {
    if let Some(template) = std::env::var("DRUID_CONTRACT_CREATE_TABLE_SQL")
        .ok()
        .filter(|value| !value.is_empty())
    {
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
        if matches!(family, ProtocolFamily::SQLite | ProtocolFamily::HttpSql) {
            let sqlite_equivalent = match (actual, expected) {
                (Value::Int(1), Value::Bool(true)) => true,
                (Value::Float(actual), Value::Decimal(expected)) => {
                    (*actual - 12.5).abs() < f64::EPSILON && expected.to_string() == "12.5"
                }
                (Value::String(actual), Value::Decimal(expected)) => {
                    actual == &expected.to_string()
                }
                (Value::String(actual), Value::Date(expected)) => actual == &expected.to_string(),
                (Value::String(actual), Value::Time(expected)) => actual == &expected.to_string(),
                (Value::String(actual), Value::Timestamp(expected)) => {
                    actual == &expected.to_string()
                }
                _ => false,
            };
            if sqlite_equivalent {
                // SQLite 只有动态 storage class；适配器保留真实运行时值，合同只接受
                // 可逆的 SQLite 表示，不伪造 Decimal/日期时间的强类型身份。
                continue;
            }
        }
        assert_eq!(actual, expected, "scalar type mismatch at column {index}");
    }
}

async fn database_version(
    connection: &mut druid::core::DruidPooledConnection,
    family: ProtocolFamily,
) -> String {
    let configured_sql = std::env::var("DRUID_CONTRACT_DATABASE_VERSION_SQL")
        .ok()
        .filter(|value| !value.is_empty());
    let sql = configured_sql.as_deref().unwrap_or(match family {
        ProtocolFamily::SQLite | ProtocolFamily::HttpSql => "SELECT sqlite_version()",
        ProtocolFamily::MySql => "SELECT version()",
        ProtocolFamily::PostgreSql => "SHOW server_version",
        _ => "SELECT version()",
    });
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
        verify_duckdb_rejected_driver_connection(&bad_url).await;
        return;
    }
    let registry = DruidDriverRegistry::builtin().expect("内置数据库档案必须可解析");
    let runtime_mode = registry
        .profiles()
        .find(|profile| profile.id().as_str() == profile_id)
        .expect("坏连接合同必须引用已知档案")
        .runtime_mode();
    let pool = configured_pool_builder(profile_id, &bad_url, runtime_mode)
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
async fn verify_duckdb_rejected_driver_connection(bad_url: &str) {
    assert!(
        bad_url.starts_with("duckdb:"),
        "DuckDB 坏连接测试输入必须保留 duckdb scheme"
    );
    let malformed_url = format!("{bad_url}?druid-invalid-option=1");
    assert!(
        matches!(
            DuckDbConnectionAdapter::connect(&malformed_url).await,
            Err(RdbcError::InvalidArgument(_))
        ),
        "DuckDB 不支持凭据认证，必须在物理连接前拒绝非法 URL"
    );
}

#[cfg(not(feature = "duckdb-native"))]
async fn verify_duckdb_rejected_driver_connection(_bad_url: &str) {
    panic!("DuckDB 完整契约必须启用 duckdb-native feature");
}

fn write_contract_evidence_if_configured(
    profile: &druid_wrapper::driver::DatabaseProfile,
    database_version: &str,
    timeout_cancel: bool,
    database_restart: bool,
    jdbc_faults: bool,
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
    assert!(timeout_cancel, "认证证据必须真实执行 timeout-cancel");
    assert!(database_restart, "认证证据必须真实执行 database-restart");
    assert!(
        profile.runtime_mode() != DriverRuntimeMode::JdbcAgent || jdbc_faults,
        "JDBC 认证证据必须执行 agent-crash 与 protocol-failure"
    );
    let mut contract_checks = vec![
        "connection-lifecycle",
        "validation",
        "crud-ddl",
        "scalar-types",
        "prepared-and-batch",
        "transactions",
        "state-reset",
        "capabilities",
        "error-classification",
        "timeout-cancel",
        "database-restart",
        "concurrency-leak-shutdown",
        "no-pool-in-pool",
    ];
    if profile.runtime_mode() == DriverRuntimeMode::JdbcAgent {
        contract_checks.extend(["agent-crash", "protocol-failure"]);
    }
    let java_versions = if profile.runtime_mode() == DriverRuntimeMode::JdbcAgent {
        vec![std::env::var("DRUID_CONTRACT_JAVA_VERSION")
            .unwrap_or_else(|_| "17".to_owned())
            .parse::<u16>()
            .expect("DRUID_CONTRACT_JAVA_VERSION 必须是 Java 主版本")]
    } else {
        Vec::new()
    };
    let artifact_sha256 = std::env::var("DRUID_CONTRACT_ARTIFACT_SHA256")
        .ok()
        .filter(|value| !value.is_empty());
    let record = serde_json::json!({
        "profileId": profile.id().as_str(),
        "target": target,
        "databaseVersion": database_version,
        "rustVersion": rust_version,
        "javaVersions": java_versions,
        "runtimeMode": runtime_mode_label(profile.runtime_mode()),
        "installationPaths": installation_paths(profile.runtime_mode()),
        "contractChecks": contract_checks,
        "sourceRevision": source_revision,
        "evidenceRef": evidence_ref,
        "passedAt": Utc::now().to_rfc3339(),
        "artifactSha256": artifact_sha256
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
        DriverRuntimeMode::JdbcAgent => {
            vec!["jdbc-agent", "offline-preinstalled", "explicit-install"]
        }
        DriverRuntimeMode::HttpSql => vec!["http-sql"],
    }
}

async fn verify_error_classification(
    connection: &mut druid::core::DruidPooledConnection,
    table: &str,
) {
    let error = connection
        .fetch(&format!("SELECT * FROM {table}_missing"), Vec::new())
        .await
        .expect_err("不存在的表必须返回结构化驱动错误");
    match error {
        RdbcError::SqlException(exception) => {
            assert!(
                exception.error_code() != 0 || exception.sql_state().is_some(),
                "驱动必须至少保留 vendor code 或 SQLState"
            );
            assert!(!exception.class_name().is_empty());
            assert!(exception
                .message()
                .is_some_and(|message| !message.is_empty()));
        }
        other => panic!("错误必须保留 SQLState/vendor/class，实际为 {other:?}"),
    }
}

fn prepared_values(values: Vec<Value>) -> Vec<PreparedParameter> {
    values
        .into_iter()
        .enumerate()
        .map(|(offset, value)| PreparedParameter {
            index: ParameterIndex::new(offset + 1).unwrap(),
            value,
        })
        .collect()
}

fn bind_prepared_values(statement: &mut dyn PreparedStatement, values: Vec<Value>) {
    for parameter in prepared_values(values) {
        statement.set_parameter(parameter).unwrap();
    }
}
