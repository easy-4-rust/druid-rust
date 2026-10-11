//! 冻结 P05 收敛前 SQLite 三个公开解析入口的可观察行为。

use druid::core::{lookup_driver_extension, Connection};
use druid::sql::DriverManager;
use druid_wrapper::driver::{
    DatabaseConnectionConfig, DruidDatabasePoolBuilder, DruidDriverRegistry,
};

#[tokio::test]
async fn sqlite_entry_points_expose_the_current_provider_split() {
    const URL: &str = "sqlite::memory:";
    druid_wrapper::init_driver_extensions();

    // Java 兼容入口只考虑显式注册的 Driver；本测试进程没有注册产品驱动。
    assert!(DriverManager::get_driver(URL).is_err());

    let extension = lookup_driver_extension("sqlite").expect("SQLite inventory extension");
    let inventory_factory = (extension.factory)(URL)
        .await
        .expect("inventory factory initializes inside Tokio");
    assert_eq!(inventory_factory.name(), "SQLite");
    let mut inventory_connection = inventory_factory.create().await.expect("inventory SQLite");
    inventory_connection.close().await.expect("inventory close");

    let registry = DruidDriverRegistry::builtin().expect("product catalog");
    let config = DatabaseConnectionConfig::new("sqlite", URL).expect("SQLite config");
    let product = registry.resolve(&config).expect("product resolution");
    assert_eq!(product.url(), URL);
    assert_eq!(product.factory().name(), "sqlx");

    let pool = DruidDatabasePoolBuilder::new("sqlite", URL)
        .pool(|builder| builder.max_open(1).max_idle(1))
        .build()
        .await
        .expect("product pool");
    let mut connection = pool.get().await.expect("pooled SQLite");
    let rows = connection
        .fetch("SELECT 1", Vec::new())
        .await
        .expect("SQLite query");
    assert_eq!(rows.len(), 1);
    drop(connection);
    pool.close().await;
}
