//! 连接工厂的诊断输出不得公开 URL 内的凭据。

#[cfg(feature = "libsql-native")]
#[test]
fn libsql_factory_debug_hides_url_credentials() {
    let factory = druid_wrapper::libsql::LibSqlConnectionFactory::new(
        "libsql://user:secret@host.invalid/db?token=canary-token",
    );
    let debug = format!("{factory:?}");
    assert!(!debug.contains("secret"), "{debug}");
    assert!(!debug.contains("canary-token"), "{debug}");
}

#[cfg(feature = "http-sql")]
#[test]
fn http_sql_factory_debug_hides_endpoint_credentials() {
    let factory = druid_wrapper::http_sql::HttpSqlConnectionFactory::new(
        druid_wrapper::http_sql::HttpSqlProvider::Rqlite,
        "https://user:secret@host.invalid/db?token=canary-token",
    );
    let debug = format!("{factory:?}");
    assert!(!debug.contains("secret"), "{debug}");
    assert!(!debug.contains("canary-token"), "{debug}");
}

#[cfg(feature = "duckdb-native")]
#[test]
fn duckdb_factory_debug_hides_native_url() {
    let factory = druid_wrapper::duckdb::DuckDbConnectionFactory::new(
        "duckdb:/private/canary-token/database.db",
    );
    let debug = format!("{factory:?}");
    assert!(!debug.contains("canary-token"), "{debug}");
}

#[cfg(feature = "toasty-sqlite")]
#[tokio::test]
async fn toasty_factory_debug_hides_url() {
    let factory = druid_wrapper::toasty::ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let debug = format!("{factory:?}");
    assert!(!debug.contains("sqlite::memory:"), "{debug}");
    assert!(debug.contains("<redacted connection URL>"), "{debug}");
}

#[cfg(feature = "jdbc-agent")]
#[test]
fn jdbc_agent_factory_debug_hides_connection_and_process_secrets() {
    let options = druid_wrapper::jdbc_agent::JdbcAgentOptions::new("canary-program-secret")
        .argument("canary-argument-secret")
        .runtime_identity(
            "canary-agent-key-secret",
            "canary-artifact-secret",
            "canary-hash-secret",
        );
    let options_debug = format!("{options:?}");
    let factory = druid_wrapper::jdbc_agent::JdbcAgentConnectionFactory::new(
        "jdbc:postgresql://host.invalid/db?password=canary-url-secret",
        Some("SELECT 'canary-validation-secret'".to_owned()),
        options,
    )
    .user_name("canary-user-secret")
    .password("canary-password-secret")
    .property("canary-property-name-secret", "canary-property-secret");
    let factory_debug = format!("{factory:?}");

    for secret in [
        "canary-program-secret",
        "canary-argument-secret",
        "canary-agent-key-secret",
        "canary-artifact-secret",
        "canary-hash-secret",
    ] {
        assert!(!options_debug.contains(secret), "{options_debug}");
        assert!(!factory_debug.contains(secret), "{factory_debug}");
    }
    for secret in [
        "canary-url-secret",
        "canary-validation-secret",
        "canary-user-secret",
        "canary-password-secret",
        "canary-property-secret",
        "canary-property-name-secret",
    ] {
        assert!(!factory_debug.contains(secret), "{factory_debug}");
    }
    assert!(
        factory_debug.contains("has_password: true"),
        "{factory_debug}"
    );
}
