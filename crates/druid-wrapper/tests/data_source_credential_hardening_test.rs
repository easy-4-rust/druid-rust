use std::any::TypeId;

use druid::core::{Connection, RdbcError};
use druid::sql::DataSource;
use druid_rdbc::data_source::{
    CommonDataSource, ConnectionPoolDataSource, DataSource as StandardDataSource,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use druid_wrapper::driver::DruidDatabasePoolBuilder;
use druid_wrapper::rdbc::DruidRdbcDataSource;

#[tokio::test]
async fn standard_data_source_unwrap_returns_the_same_wrapper_object() {
    let data_source = DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
        .name("standard-data-source-unwrap")
        .build_data_source()
        .await
        .expect("build canonical pool");
    let source = DruidRdbcDataSource::new(data_source);
    let wrapper: &dyn Wrapper = &source;
    let standard: &dyn StandardDataSource = &source;
    let common: &dyn CommonDataSource = &source;

    for target in [
        TypeId::of::<dyn StandardDataSource>(),
        TypeId::of::<dyn CommonDataSource>(),
    ] {
        assert!(wrapper.is_wrapper_for(Some(target)).expect("inspect view"));
        let view = wrapper.unwrap(Some(target)).expect("unwrap view");
        match view {
            StandardUnwrapped::DataSource(view) => {
                assert_eq!(target, TypeId::of::<dyn StandardDataSource>());
                assert!(std::ptr::addr_eq(view, standard));
            }
            StandardUnwrapped::CommonDataSource(view) => {
                assert_eq!(target, TypeId::of::<dyn CommonDataSource>());
                assert!(std::ptr::addr_eq(view, common));
            }
            other => panic!("unexpected standard view: {other:?}"),
        }
    }
    let unsupported = TypeId::of::<dyn ConnectionPoolDataSource>();
    assert!(!wrapper
        .is_wrapper_for(Some(unsupported))
        .expect("inspect unsupported view"));
    let error = wrapper
        .unwrap(Some(unsupported))
        .expect_err("unsupported view must fail");
    assert_eq!(
        error.sql_exception().and_then(|value| value.sql_state()),
        Some("0A000")
    );
    assert_eq!(source.inner().state().active_count, 0);
    source.inner().close().await;
}

#[tokio::test]
async fn configured_credentials_are_redacted_and_pair_boundaries_are_preserved() {
    let data_source = DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
        .name("credential-hardening")
        .pool(|builder| builder.max_open(1).max_idle(1))
        .build_data_source()
        .await
        .expect("build canonical pool");
    let source = DruidRdbcDataSource::with_credentials(data_source, "account:part", "secret");

    let diagnostic = format!("{source:?}");
    assert!(!diagnostic.contains("account:part"));
    assert!(!diagnostic.contains("secret"));

    let mut connection = source
        .get_connection_with_credentials("account:part", "secret")
        .await
        .expect("configured credential pair must connect");
    connection
        .ping()
        .await
        .expect("borrowed connection is live");
    drop(connection);

    for (username, password) in [
        ("account", "part:secret"),
        ("account:part", "wrong"),
        ("wrong", "secret"),
    ] {
        let error = source
            .get_connection_with_credentials(username, password)
            .await
            .expect_err("different credential pair must fail");
        assert!(matches!(
            error,
            RdbcError::SqlException(exception) if exception.sql_state() == Some("28000")
        ));
    }

    source.inner().close().await;
}

#[tokio::test]
async fn unconfigured_credentials_still_reject_explicit_login() {
    let data_source = DruidDatabasePoolBuilder::new("sqlite", "sqlite::memory:")
        .name("unconfigured-credential-gate")
        .build_data_source()
        .await
        .expect("build canonical pool");
    let source = DruidRdbcDataSource::new(data_source);
    let diagnostic = format!("{source:?}");
    assert!(diagnostic.contains("credentials_configured: false"));

    let error = source
        .get_connection_with_credentials("user", "password")
        .await
        .expect_err("no configured credential pair must reject explicit login");
    assert!(matches!(
        error,
        RdbcError::SqlException(exception) if exception.sql_state() == Some("28000")
    ));
    source.inner().close().await;
}
