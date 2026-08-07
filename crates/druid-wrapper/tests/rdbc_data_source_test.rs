use std::sync::Arc;

use druid::core::{DruidError, PhysicalConnection, Pool};
use druid::dynamic::{DataSourceGroup, RoundRobinBalancer, SqlHint};
use druid::rdbc::DataSource;
use druid_wrapper::driver::DruidDatabasePoolBuilder;
use druid_wrapper::rdbc::{DruidRdbcDataSource, DynamicRdbcDataSource};

#[tokio::test]
async fn catalog_builder_creates_canonical_druid_data_source() {
    let data_source = DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
        .name("rdbc-sqlite")
        .pool(|builder| builder.max_open(1).max_idle(1))
        .build_data_source()
        .await
        .expect("数据库目录必须构建 canonical DruidDataSource");
    let wrapped = DruidRdbcDataSource::with_credentials(data_source, "druid", "secret");

    let mut connection = wrapped
        .get_connection_with_credentials("druid", "secret")
        .await
        .expect("匹配凭据必须从同一 Druid pool 借出连接");
    connection.ping().await.expect("SQLite 连接必须可用");
    drop(connection);

    assert!(matches!(
        wrapped.get_connection_with_credentials("druid", "wrong").await,
        Err(DruidError::SqlException(exception))
            if exception.sql_state() == Some("28000")
                && exception.class_name() == "java.sql.SQLInvalidAuthorizationSpecException"
    ));
    wrapped.inner().close().await;
}

#[tokio::test]
async fn dynamic_data_source_routes_reads_and_writes_and_switches_atomically() {
    let master = Arc::new(
        DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
            .name("master")
            .build()
            .await
            .expect("主库必须构建"),
    );
    let slave = Arc::new(
        DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
            .name("slave")
            .build()
            .await
            .expect("从库必须构建"),
    );
    let group = DataSourceGroup::new(
        "v1",
        Arc::clone(&master) as Arc<dyn Pool>,
        vec![Arc::clone(&slave) as Arc<dyn Pool>],
        Arc::new(RoundRobinBalancer::new()),
    );
    let data_source = DynamicRdbcDataSource::new(group);

    assert_eq!(data_source.route(SqlHint::Write).name(), "master");
    assert_eq!(data_source.route(SqlHint::Read).name(), "slave");
    let connection = data_source
        .get_connection()
        .await
        .expect("默认路由必须可借连接");
    drop(connection);

    let next = Arc::new(
        DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
            .name("next-master")
            .build()
            .await
            .expect("切换主库必须构建"),
    );
    data_source.switch(DataSourceGroup::new(
        "v2",
        Arc::clone(&next) as Arc<dyn Pool>,
        Vec::new(),
        Arc::new(RoundRobinBalancer::new()),
    ));
    assert_eq!(data_source.current_name(), "v2");
    assert_eq!(data_source.route(SqlHint::Write).name(), "next-master");

    master.close().await;
    slave.close().await;
    next.close().await;
}
