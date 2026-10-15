use druid::core::{Connection, RdbcError};
use druid::sql::DataSource;
use druid_wrapper::driver::DruidDatabasePoolBuilder;
use druid_wrapper::rdbc::DruidRdbcDataSource;

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
