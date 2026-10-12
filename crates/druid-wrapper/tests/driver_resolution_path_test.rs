//! 验证 SQLite 产品解析与建池使用同一未池化驱动工厂。

use druid::sql::DriverManager;
use druid_rdbc::connection::Connection;
use druid_wrapper::driver::{
    DatabaseConnectionConfig, DruidDatabasePoolBuilder, DruidDriverRegistry,
};

#[tokio::test]
async fn sqlite_registry_and_pool_use_the_same_provider() {
    const URL: &str = "sqlite::memory:";

    // Java 兼容入口只考虑显式注册的 Driver；产品解析由唯一 catalog 入口负责。
    assert!(DriverManager::get_driver(URL).is_err());

    let registry = DruidDriverRegistry::builtin().expect("product catalog");
    let config = DatabaseConnectionConfig::new("sqlite", URL).expect("SQLite config");
    let product = registry.resolve(&config).expect("product resolution");
    assert_eq!(product.url(), URL);
    assert_eq!(product.factory().name(), "sqlx");
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
