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
