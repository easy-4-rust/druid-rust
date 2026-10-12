//! 验证 SQLite 产品解析与建池使用同一未池化驱动工厂。

use druid::sql::DriverManager;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionProperties;
use druid_wrapper::driver::{
    DatabaseConnectionConfig, DruidDatabasePoolBuilder, DruidDriverRegistry,
};

#[tokio::test]
async fn sqlite_registry_and_pool_use_the_same_provider() {
    const URL: &str = "sqlite::memory:";

    // Java 兼容入口只考虑显式注册的 Driver；产品解析由唯一 catalog 入口负责。
    assert!(DriverManager::get_driver(URL).is_err());

    let registry = DruidDriverRegistry::builtin().expect("product catalog");
    assert!(registry
        .standard_registry()
        .factory_provider("sqlx")
        .is_some());
    assert!(registry
        .standard_registry()
        .factory_provider("not-installed")
        .is_none());
    let config = DatabaseConnectionConfig::new("sqlite", URL).expect("SQLite config");
    let product = registry.resolve(&config).expect("product resolution");
    assert_eq!(product.url(), URL);
    assert_eq!(product.factory().name(), "sqlx");
    let canonical_factory = registry
        .standard_registry()
        .factory_provider("sqlx")
        .expect("SQLx provider")
        .factory(URL, &ConnectionProperties::new(), None);
    assert_eq!(canonical_factory.name(), product.factory().name());
    let mut direct_connection = product.factory().create().await.expect("direct SQLite");
    direct_connection.close().await.expect("direct close");

    let pool = DruidDatabasePoolBuilder::new("sqlite", URL)
        .pool(|builder| builder.max_open(1).max_idle(1))
        .build()
        .await
        .expect("product pool");
    assert_eq!(pool.raw_driver().name(), product.factory().name());
    let mut connection = pool.get().await.expect("pooled SQLite");
    let rows = connection
        .fetch("SELECT 1", Vec::new())
        .await
        .expect("SQLite query");
    assert_eq!(rows.len(), 1);
    drop(connection);
    pool.close().await;
}

#[cfg(all(
    feature = "duckdb-native",
    feature = "libsql-native",
    feature = "http-sql"
))]
#[test]
fn native_and_http_catalog_paths_use_installed_standard_providers() {
    let registry = DruidDriverRegistry::builtin().expect("product catalog");
    for (profile_id, url, provider_key, factory_name) in [
        ("duckdb", "duckdb::memory:", "duckdb-native", "duckdb-rs"),
        ("turso", "libsql://localhost", "libsql-native", "libsql-rs"),
        (
            "rqlite",
            "https://localhost",
            "http-sql:rqlite-http",
            "http-sql",
        ),
        (
            "cloudflare-d1",
            "https://localhost",
            "http-sql:cloudflare-d1-http",
            "http-sql",
        ),
    ] {
        assert!(registry
            .standard_registry()
            .factory_provider(provider_key)
            .is_some());
        let config = DatabaseConnectionConfig::new(profile_id, url).expect("product config");
        let resolved = registry.resolve(&config).expect("catalog resolution");
        assert_eq!(resolved.factory().name(), factory_name);
    }
}
