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

    assert_eq!(manifest.schema_version(), 2);
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
        "schemaVersion": 2,
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
        "schemaVersion": 2,
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
fn manifest_rejects_incompatible_provider_runtime_and_protocol() {
    let invalid = r#"{
        "schemaVersion": 2,
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
