//! 外部消费者通过统一 RDBC DataSource 合同使用动态路由数据源。

use std::any::TypeId;
use std::sync::Arc;

use druid::core::Pool;
use druid::dynamic::{DataSourceGroup, RoundRobinBalancer};
use druid_rdbc::data_source::{CommonDataSource, DataSource as StandardDataSource};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use druid_wrapper::driver::DruidDatabasePoolBuilder;
use druid_wrapper::rdbc::DynamicRdbcDataSource;

#[tokio::test]
async fn standard_dynamic_source_uses_current_group_and_preserves_unwrap_identity() {
    let master = Arc::new(
        DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
            .name("standard-dynamic-master")
            .build()
            .await
            .expect("build first pool"),
    );
    let source = DynamicRdbcDataSource::new(DataSourceGroup::new(
        "first",
        Arc::clone(&master) as Arc<dyn Pool>,
        Vec::new(),
        Arc::new(RoundRobinBalancer::new()),
    ));
    let standard: &dyn StandardDataSource = &source;
    let common: &dyn CommonDataSource = &source;
    let wrapper: &dyn Wrapper = &source;

    for target in [
        TypeId::of::<dyn StandardDataSource>(),
        TypeId::of::<dyn CommonDataSource>(),
    ] {
        assert!(wrapper.is_wrapper_for(Some(target)).expect("inspect view"));
        match wrapper.unwrap(Some(target)).expect("unwrap view") {
            StandardUnwrapped::DataSource(view) => {
                assert_eq!(target, TypeId::of::<dyn StandardDataSource>());
                assert!(std::ptr::addr_eq(view, standard));
            }
            StandardUnwrapped::CommonDataSource(view) => {
                assert_eq!(target, TypeId::of::<dyn CommonDataSource>());
                assert!(std::ptr::addr_eq(view, common));
            }
            other => panic!("unexpected view: {other:?}"),
        }
    }

    let mut first = standard.connection().await.expect("borrow first pool");
    assert_eq!(master.state().active_count, 1);
    first.close().await.expect("return first holder");
    drop(first);
    assert_eq!(master.state().active_count, 0);

    let next = Arc::new(
        DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
            .name("standard-dynamic-next")
            .build()
            .await
            .expect("build second pool"),
    );
    source.switch(DataSourceGroup::new(
        "second",
        Arc::clone(&next) as Arc<dyn Pool>,
        Vec::new(),
        Arc::new(RoundRobinBalancer::new()),
    ));
    let mut second = standard.connection().await.expect("borrow switched pool");
    assert_eq!(master.state().active_count, 0);
    assert_eq!(next.state().active_count, 1);
    second.close().await.expect("return second holder");
    drop(second);

    let error = standard
        .connection_with_credentials("user", "password")
        .await
        .err()
        .expect("dynamic source does not verify credentials");
    assert_eq!(
        error.sql_exception().and_then(|value| value.sql_state()),
        Some("0A000")
    );
    assert_eq!(next.state().active_count, 0);
    master.close().await;
    next.close().await;
}
