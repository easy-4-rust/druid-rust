use druid::core::{Connection, ConnectionFactory, RdbcError};
use druid::pool::{DruidDataSource, DruidPool};
use druid::sql::DriverManager;
use druid_rdbc::data_source::{CommonDataSource, RdbcLogWriter};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct MockConnection;
druid_rdbc::impl_standard_wrapper!(MockConnection => Connection);

#[async_trait::async_trait]
impl Connection for MockConnection {}

struct MockFactory;

#[async_trait::async_trait]
impl ConnectionFactory for MockFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(MockConnection))
    }

    async fn validate(&self, _connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        Ok(())
    }
}

struct FailingFactory;

#[async_trait::async_trait]
impl ConnectionFactory for FailingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::SqlException(Box::new(
            druid::core::SqlException::new(
                71,
                Some("08001".to_owned()),
                Some(
                    "connect failed: postgres://alice:pw123@db/app?token=tok456&api_key=api789&sslpassword=ssl123"
                        .to_owned(),
                ),
            ),
        )))
    }

    async fn validate(&self, _connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        Ok(())
    }
}

fn writer() -> (RdbcLogWriter, Arc<Mutex<Vec<u8>>>) {
    let output = Arc::new(Mutex::new(Vec::new()));
    let shared = Arc::clone(&output);
    let writer: RdbcLogWriter = Arc::new(parking_lot::Mutex::new(Box::new(Capture(shared))));
    (writer, output)
}

struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("output lock").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("writer-private-detail"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn pooled_druid_data_source_exposes_standard_log_settings() {
    let pool = DruidPool::builder()
        .name("logging-test")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .build()
        .await
        .expect("build pool");
    let source = DruidDataSource::from_pool(pool);

    assert!(CommonDataSource::get_log_writer(&source).is_none());
    let (writer, output) = writer();
    CommonDataSource::set_log_writer(&source, Some(Arc::clone(&writer))).expect("configure writer");
    let stored = CommonDataSource::get_log_writer(&source).expect("configured writer");
    assert!(Arc::ptr_eq(&stored, &writer));
    let legacy_stored = druid::sql::CommonDataSource::get_log_writer(&source)
        .expect("legacy facade sees canonical writer");
    assert!(Arc::ptr_eq(&legacy_stored, &writer));
    writeln!(stored.lock(), "pool diagnostic").expect("write diagnostic");
    assert_eq!(
        output.lock().expect("output lock").as_slice(),
        b"pool diagnostic\n"
    );
    assert_eq!(
        CommonDataSource::get_parent_logger(&source),
        Ok("druid.pool")
    );

    CommonDataSource::set_log_writer(&source, None).expect("disable writer");
    assert!(CommonDataSource::get_log_writer(&source).is_none());
    source.close().await;
}

#[tokio::test]
async fn pool_connection_failure_is_logged_safely_without_replacing_sql_error() {
    let pool = DruidPool::builder()
        .name("logging-failure-test")
        .driver_name("mock")
        .factory(Arc::new(FailingFactory))
        .fail_fast(true)
        .login_timeout(7)
        .build()
        .await
        .expect("build pool");
    let source = DruidDataSource::from_pool(pool);
    let (writer, output) = writer();
    CommonDataSource::set_log_writer(&source, Some(Arc::clone(&writer))).expect("configure writer");

    let primary = source
        .get_connection()
        .await
        .expect_err("connection must fail");
    let cause = std::error::Error::source(&primary)
        .and_then(|source| source.downcast_ref::<druid::core::DruidError>())
        .expect("pool error retains typed driver cause");
    assert_eq!(
        cause
            .sql_exception()
            .and_then(|exception| exception.sql_state()),
        Some("08001")
    );
    let diagnostic = String::from_utf8(output.lock().expect("output lock").clone())
        .expect("diagnostic is UTF-8");
    assert!(diagnostic.contains("operation=get_connection"));
    assert!(
        diagnostic.contains("sql_state=08001"),
        "diagnostic: {diagnostic}"
    );
    for secret in ["alice", "pw123", "db/app", "tok456", "api789", "ssl123"] {
        assert!(!diagnostic.contains(secret), "diagnostic leaked {secret}");
    }

    source.close().await;
}

#[tokio::test]
async fn pool_log_io_failure_preserves_primary_connection_error_and_configuration() {
    let pool = DruidPool::builder()
        .name("logging-io-failure-test")
        .driver_name("mock")
        .factory(Arc::new(FailingFactory))
        .fail_fast(true)
        .login_timeout(11)
        .build()
        .await
        .expect("build pool");
    let mut source = DruidDataSource::from_pool(pool);
    CommonDataSource::set_log_writer(
        &source,
        Some(Arc::new(parking_lot::Mutex::new(Box::new(FailingWriter)))),
    )
    .expect("configure failing writer");

    let config_error = CommonDataSource::set_login_timeout(&mut source, Duration::from_secs(99))
        .expect_err("built pool timeout is immutable");
    assert!(matches!(
        config_error,
        RdbcError::UnsupportedOperation {
            operation: "set_login_timeout_after_build"
        }
    ));

    let primary = source
        .get_connection()
        .await
        .expect_err("failed connection must remain failed");
    let cause = std::error::Error::source(&primary)
        .and_then(|source| source.downcast_ref::<druid::core::DruidError>())
        .expect("pool error retains typed driver cause");
    assert_eq!(
        cause
            .sql_exception()
            .and_then(|exception| exception.sql_state()),
        Some("08001")
    );
    assert_eq!(source.login_timeout(), 11);
    assert!(CommonDataSource::get_log_writer(&source).is_some());
    source.close().await;
}

#[tokio::test]
async fn data_source_diagnostics_are_isolated_from_other_sources_and_driver_manager() {
    let first_pool = DruidPool::builder()
        .name("logging-isolation-first")
        .driver_name("mock")
        .factory(Arc::new(FailingFactory))
        .fail_fast(true)
        .build()
        .await
        .expect("build first pool");
    let second_pool = DruidPool::builder()
        .name("logging-isolation-second")
        .driver_name("mock")
        .factory(Arc::new(FailingFactory))
        .fail_fast(true)
        .build()
        .await
        .expect("build second pool");
    let first = DruidDataSource::from_pool(first_pool);
    let second = DruidDataSource::from_pool(second_pool);
    let (first_writer, first_output) = writer();
    let (second_writer, second_output) = writer();
    let (global_writer, global_output) = writer();
    CommonDataSource::set_log_writer(&first, Some(first_writer)).expect("configure first logger");
    CommonDataSource::set_log_writer(&second, Some(second_writer))
        .expect("configure second logger");
    DriverManager::set_log_writer(Some(global_writer));

    assert!(first.get_connection().await.is_err());

    assert!(!first_output.lock().expect("first output").is_empty());
    assert!(second_output.lock().expect("second output").is_empty());
    assert!(global_output.lock().expect("global output").is_empty());
    DriverManager::set_log_writer(None);
    first.close().await;
    second.close().await;
}
