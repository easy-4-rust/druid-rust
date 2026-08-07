use druid::core::PhysicalConnection;
use druid_wrapper::driver::{
    DatabaseConnectionConfig, DatabaseProfileId, DriverManifest, DriverRegistryError,
    DriverSupportStatus, DruidDatabasePoolBuilder, DruidDriverRegistry,
};
#[cfg(feature = "jdbc-agent")]
use druid_wrapper::jdbc_agent::JdbcAgentOptions;

#[test]
fn builtin_catalog_has_exact_phased_sql_scope() {
    let manifest = DriverManifest::builtin().expect("内置数据库目录必须可解析");

    assert_eq!(manifest.schema_version(), 3);
    assert_eq!(manifest.profiles().len(), 80);
    assert_eq!(
        manifest
            .profiles()
            .iter()
            .filter(|profile| profile.delivery_phase() == 1)
            .count(),
        15
    );
    assert_eq!(
        manifest
            .profiles()
            .iter()
            .filter(|profile| profile.delivery_phase() == 2)
            .count(),
        25
    );
    assert_eq!(
        manifest
            .profiles()
            .iter()
            .filter(|profile| profile.delivery_phase() == 3)
            .count(),
        40
    );

    let excluded_non_sql_products = [
        "redis",
        "mongodb",
        "elasticsearch",
        "kafka",
        "rabbitmq",
        "etcd",
        "zookeeper",
    ];
    assert!(manifest
        .profiles()
        .iter()
        .all(|profile| { !excluded_non_sql_products.contains(&profile.id().as_str()) }));
    assert!(manifest.profiles().iter().all(|profile| {
        !profile.artifact_id().is_empty() && !profile.exception_sorter().is_empty()
    }));
}

#[test]
fn catalog_count_only_includes_profiles_with_support_evidence() {
    let registry = DruidDriverRegistry::builtin().expect("内置注册中心必须可创建");
    let supported = registry.supported_count();
    let declared = registry
        .profiles()
        .filter(|profile| profile.support_status() == DriverSupportStatus::Declared)
        .count();

    assert_eq!(supported, 0, "尚未产生三平台证据前不得计入公开支持数");
    assert!(declared > 0);
}

#[test]
fn registry_resolves_protocol_compatible_sqlx_profiles() {
    let registry = DruidDriverRegistry::builtin().expect("内置注册中心必须可创建");
    let config =
        DatabaseConnectionConfig::new("sqlite", "sqlite::memory:").expect("SQLite 配置必须合法");
    let resolved = registry
        .resolve(&config)
        .expect("SQLite SQLx 驱动必须可解析");

    assert_eq!(resolved.profile().id().as_str(), "sqlite");
    assert_eq!(resolved.url(), "sqlite::memory:");

    let invalid = DatabaseConnectionConfig::new("mysql", "postgres://localhost/demo")
        .expect("产品 ID 本身合法");
    assert!(matches!(
        registry.resolve(&invalid),
        Err(DriverRegistryError::InvalidUrl { .. })
    ));

    let jdbc_mysql = DatabaseConnectionConfig::new("mysql", "jdbc:mysql://localhost/demo")
        .expect("JDBC 兼容 URL 配置必须合法");
    let resolved = registry
        .resolve(&jdbc_mysql)
        .expect("JDBC MySQL URL 必须规范化给 SQLx");
    assert_eq!(
        resolved.factory().connection_url(),
        Some("mysql://localhost/demo")
    );
    assert_eq!(resolved.url(), "jdbc:mysql://localhost/demo");
}

#[test]
fn manifest_rejects_unknown_druid_dialect() {
    let invalid = r#"{
        "schemaVersion": 3,
        "catalogVersion": "test",
        "profiles": [{
            "profileId": "invalid",
            "displayName": "Invalid",
            "dbType": "not-a-druid-dialect",
            "protocolFamily": "jdbc",
            "runtimeMode": "jdbc_agent",
            "providerId": "jdbc-agent",
            "artifactId": "invalid-jdbc",
            "exceptionSorter": "auto",
            "supportStatus": "declared",
            "wallMode": "generic",
            "deliveryPhase": 3
        }]
    }"#;

    assert!(matches!(
        DriverManifest::from_json(invalid),
        Err(DriverRegistryError::InvalidManifest(_))
    ));
}

#[test]
fn manifest_rejects_verified_profiles_without_cross_platform_evidence() {
    let invalid = r#"{
        "schemaVersion": 3,
        "catalogVersion": "test",
        "profiles": [{
            "profileId": "verified-without-evidence",
            "displayName": "Verified Without Evidence",
            "dbType": "h2",
            "protocolFamily": "embedded",
            "runtimeMode": "jdbc_agent",
            "providerId": "jdbc-agent",
            "artifactId": "h2-jdbc",
            "exceptionSorter": "auto",
            "supportStatus": "verified",
            "wallMode": "dedicated",
            "deliveryPhase": 1,
            "validationQuery": "SELECT 1",
            "capabilities": {
                "query": true,
                "update": true,
                "preparedStatements": true,
                "transactions": true
            }
        }]
    }"#;

    assert!(matches!(
        DriverManifest::from_json(invalid),
        Err(DriverRegistryError::InvalidManifest(message))
            if message.contains("lacks Linux/macOS/Windows evidence")
    ));
}

#[test]
fn manifest_rejects_platform_labels_without_auditable_contract_runs() {
    let invalid = verified_sqlx_manifest(serde_json::json!([]));

    assert!(matches!(
        DriverManifest::from_json(&invalid),
        Err(DriverRegistryError::InvalidManifest(message))
            if message.contains("lacks Linux/macOS/Windows evidence")
    ));
}

#[test]
fn manifest_accepts_only_complete_five_target_contract_evidence() {
    let targets = [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ];
    let runs = targets
        .into_iter()
        .flat_map(|target| {
            ["1.95", "default"].map(|rust_version| {
                serde_json::json!({
                    "target": target,
                    "databaseVersion": "3.46.0",
                    "rustVersion": rust_version,
                    "runtimeMode": "sqlx",
                    "installationPaths": ["native"],
                    "contractChecks": [
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
                        "no-pool-in-pool"
                    ],
                    "sourceRevision": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "evidenceRef": "https://github.com/easy-rust/druid-rust/actions/runs/1",
                    "passedAt": "2026-08-07T00:00:00Z"
                })
            })
        })
        .collect::<Vec<_>>();
    let manifest = DriverManifest::from_json(&verified_sqlx_manifest(serde_json::json!(runs)))
        .expect("五目标真实契约记录应满足 Verified 证据 schema");
    let registry = DruidDriverRegistry::from_manifest(manifest).expect("证据清单应可注册");

    assert_eq!(registry.supported_count(), 1);
    assert_eq!(
        registry
            .profiles()
            .next()
            .expect("必须存在测试档案")
            .evidence()
            .expect("Verified 档案必须带证据")
            .runs()
            .len(),
        10
    );
}

#[test]
fn manifest_rejects_incompatible_provider_runtime_and_protocol() {
    let invalid = r#"{
        "schemaVersion": 3,
        "catalogVersion": "test",
        "profiles": [{
            "profileId": "bad-provider",
            "displayName": "Bad Provider",
            "dbType": "mysql",
            "protocolFamily": "mysql",
            "runtimeMode": "jdbc_agent",
            "providerId": "sqlx",
            "artifactId": "bad-provider",
            "exceptionSorter": "auto",
            "supportStatus": "declared",
            "wallMode": "dedicated",
            "deliveryPhase": 1
        }]
    }"#;

    assert!(matches!(
        DriverManifest::from_json(invalid),
        Err(DriverRegistryError::InvalidManifest(message))
            if message.contains("incompatible runtimeMode")
    ));
}

#[tokio::test]
async fn profile_builder_keeps_canonical_druid_pool() {
    let pool = DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
        .name("catalog-sqlite")
        .pool(|builder| builder.max_open(1).max_idle(1))
        .build()
        .await
        .expect("产品档案必须能构建 canonical DruidPool");
    let mut connection = pool.get().await.expect("必须能借出 SQLite 连接");
    let rows = connection
        .fetch("SELECT 1", Vec::new())
        .await
        .expect("统一查询契约必须可用");

    assert_eq!(rows.len(), 1);
    assert_eq!(pool.state().name, "catalog-sqlite");
    drop(connection);
    pool.close().await;
}

#[tokio::test]
async fn unified_rdbc_url_routes_through_catalog_into_real_sqlite_driver() {
    let pool = DruidDatabasePoolBuilder::from_rdbc_url("rdbc://sqlite/:memory:")
        .expect("统一 RDBC URL 必须可解析")
        .name("unified-rdbc-sqlite")
        .build()
        .await
        .expect("RDBC URL 必须转换为真实 SQLite URL 并构建 DruidPool");
    let mut connection = pool.get().await.expect("RDBC SQLite 必须可借出连接");
    let rows = connection
        .fetch("SELECT 1", Vec::new())
        .await
        .expect("RDBC 转换后的真实连接必须可查询");
    assert_eq!(rows.len(), 1);
    assert_eq!(pool.state().url, "rdbc://sqlite/:memory:");
    drop(connection);
    pool.close().await;
}

#[test]
fn unified_rdbc_url_rejects_non_rdbc_scheme_and_user_info() {
    assert!(DruidDatabasePoolBuilder::from_rdbc_url("mysql://localhost/app").is_err());
    assert!(
        DruidDatabasePoolBuilder::from_rdbc_url("rdbc://user:secret@mysql/localhost/app").is_err()
    );
}

#[tokio::test]
async fn profile_builder_installs_protocol_family_checker_and_sorter() {
    let mysql_pool = DruidDatabasePoolBuilder::new("tidb", "mysql://localhost/contract")
        .build()
        .await
        .expect("MySQL 协议族档案无需建连即可构建池");
    let mysql_stat = mysql_pool.stat_value_and_reset();
    assert!(mysql_stat
        .exception_sorter_class_name
        .as_deref()
        .is_some_and(|name| name.ends_with("MySqlExceptionSorter")));
    assert!(mysql_stat
        .valid_connection_checker_class_name
        .as_deref()
        .is_some_and(|name| name.ends_with("MySqlValidConnectionChecker")));
    mysql_pool.close().await;

    let pg_pool = DruidDatabasePoolBuilder::new("redshift", "postgresql://localhost/contract")
        .build()
        .await
        .expect("PostgreSQL 协议族档案无需建连即可构建池");
    let pg_stat = pg_pool.stat_value_and_reset();
    assert!(pg_stat
        .exception_sorter_class_name
        .as_deref()
        .is_some_and(|name| name.ends_with("PgExceptionSorter")));
    assert!(pg_stat
        .valid_connection_checker_class_name
        .as_deref()
        .is_some_and(|name| name.ends_with("PgValidConnectionChecker")));
    pg_pool.close().await;
}

#[test]
fn profile_id_is_stable_and_rejects_path_syntax() {
    let id = DatabaseProfileId::new("cloudflare-d1").expect("kebab-case ID 必须合法");
    assert_eq!(id.as_str(), "cloudflare-d1");
    assert!(DatabaseProfileId::new("../driver").is_err());
}

#[test]
#[cfg(feature = "jdbc-agent")]
fn jdbc_agent_runtime_requires_explicit_installation() {
    let config =
        DatabaseConnectionConfig::new("h2", "jdbc:h2:mem:catalog").expect("H2 产品配置必须合法");
    let registry = DruidDriverRegistry::builtin().expect("内置注册中心必须可创建");
    assert!(matches!(
        registry.resolve(&config),
        Err(DriverRegistryError::UnsupportedRuntime { .. })
    ));

    let registry = registry.with_jdbc_agent(JdbcAgentOptions::new("java"));
    let resolved = registry
        .resolve(&config)
        .expect("显式安装 Agent 后应解析 H2 驱动工厂");
    assert_eq!(resolved.profile().id().as_str(), "h2");
    assert_eq!(
        resolved.factory().connection_url(),
        Some("jdbc:h2:mem:catalog")
    );
}

fn verified_sqlx_manifest(runs: serde_json::Value) -> String {
    serde_json::json!({
        "schemaVersion": 3,
        "catalogVersion": "test",
        "profiles": [{
            "profileId": "verified-sqlite",
            "displayName": "Verified SQLite",
            "dbType": "sqlite",
            "protocolFamily": "sqlite",
            "runtimeMode": "sqlx",
            "providerId": "sqlx",
            "artifactId": "sqlite",
            "exceptionSorter": "auto",
            "supportStatus": "verified",
            "wallMode": "dedicated",
            "deliveryPhase": 1,
            "validationQuery": "SELECT 1",
            "capabilities": {
                "query": true,
                "update": true,
                "preparedStatements": true,
                "transactions": true
            },
            "evidence": {
                "contractVersion": "database-profile-v1",
                "testedAt": "2026-08-07T00:00:00Z",
                "platforms": ["linux", "macos", "windows"],
                "javaVersions": [],
                "artifactSha256": null,
                "runs": runs
            }
        }]
    })
    .to_string()
}
