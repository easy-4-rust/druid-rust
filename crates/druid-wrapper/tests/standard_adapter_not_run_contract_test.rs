//! Typed NOT_RUN boundaries for feature families without external services.

#[cfg(feature = "http-sql")]
#[tokio::test]
async fn http_sql_standard_factory_rejects_unconfigured_insecure_d1_before_network_io() {
    let factory = druid_wrapper::http_sql::HttpSqlConnectionFactory::new(
        druid_wrapper::http_sql::HttpSqlProvider::CloudflareD1,
        "http://127.0.0.1:9/query",
    );
    let Err(error) = druid_rdbc::driver::ConnectionFactory::create(&factory).await else {
        panic!("unconfigured D1 endpoint must be typed NOT_RUN");
    };
    assert!(matches!(
        error,
        druid_rdbc::error::RdbcError::InvalidArgument(_)
    ));
}

#[cfg(feature = "libsql-native")]
#[tokio::test]
async fn libsql_standard_factory_rejects_unconfigured_url_before_remote_execution() {
    let factory = druid_wrapper::libsql::LibSqlConnectionFactory::new("sqlite:unconfigured");
    let Err(error) = druid_rdbc::driver::ConnectionFactory::create(&factory).await else {
        panic!("unconfigured libSQL URL must be typed NOT_RUN");
    };
    assert!(matches!(
        error,
        druid_rdbc::error::RdbcError::InvalidArgument(_)
    ));
}

#[cfg(feature = "jdbc-agent")]
#[tokio::test]
async fn jdbc_agent_standard_factory_returns_typed_error_when_runtime_is_unconfigured() {
    let factory = druid_wrapper::jdbc_agent::JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:not_run",
        None,
        druid_wrapper::jdbc_agent::JdbcAgentOptions::new("/definitely/missing/druid-jdbc-agent"),
    );
    let Err(error) = druid_rdbc::driver::ConnectionFactory::create(&factory).await else {
        panic!("missing JDBC Agent runtime must be typed NOT_RUN");
    };
    assert!(matches!(
        error,
        druid_rdbc::error::RdbcError::DriverError(_)
            | druid_rdbc::error::RdbcError::SqlException(_)
            | druid_rdbc::error::RdbcError::ConnectionDiscarded
    ));
}
