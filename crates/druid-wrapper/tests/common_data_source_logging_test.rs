use druid::core::{Connection, ConnectionFactory, RdbcError};
use druid::pool::DruidPool;
use druid::sql::DataSource;
use druid_rdbc::data_source::{CommonDataSource, RdbcLogWriter};
use druid_wrapper::rdbc::DruidRdbcDataSource;
use std::io::Write;
use std::sync::{Arc, Mutex};

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

#[tokio::test]
async fn ordinary_rdbc_data_source_uses_the_canonical_common_data_source_contract() {
    let pool = DruidPool::builder()
        .name("ordinary-logging-test")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .build()
        .await
        .expect("build pool");
    let source = DruidRdbcDataSource::new(druid::pool::DruidDataSource::from_pool(pool));

    assert!(CommonDataSource::get_log_writer(&source).is_none());
    let (writer, output) = writer();
    CommonDataSource::set_log_writer(&source, Some(Arc::clone(&writer))).expect("configure writer");
    let stored = CommonDataSource::get_log_writer(&source).expect("configured writer");
    assert!(Arc::ptr_eq(&stored, &writer));
    writeln!(stored.lock(), "ordinary diagnostic").expect("write diagnostic");
    assert_eq!(
        output.lock().expect("output lock").as_slice(),
        b"ordinary diagnostic\n"
    );
    assert_eq!(
        CommonDataSource::get_parent_logger(&source),
        Ok("druid.pool")
    );

    CommonDataSource::set_log_writer(&source, None).expect("disable writer");
    assert!(CommonDataSource::get_log_writer(&source).is_none());
    source.inner().close().await;
}

#[tokio::test]
async fn ordinary_credential_failure_logs_safe_context_without_revealing_attempted_credentials() {
    let pool = DruidPool::builder()
        .name("ordinary-credential-diagnostic-test")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .build()
        .await
        .expect("build pool");
    let mut source = DruidRdbcDataSource::with_credentials(
        druid::pool::DruidDataSource::from_pool(pool),
        "expected-user",
        "expected-password",
    );
    let (writer, output) = writer();
    CommonDataSource::set_log_writer(&source, Some(writer)).expect("configure writer");

    let config_error = druid_rdbc::data_source::CommonDataSource::set_login_timeout(
        &mut source,
        std::time::Duration::from_secs(30),
    )
    .expect_err("built data-source timeout is immutable");
    assert!(matches!(
        config_error,
        RdbcError::UnsupportedOperation {
            operation: "set_login_timeout_after_build"
        }
    ));

    let error = DataSource::get_connection_with_credentials(
        &source,
        "attempted-user",
        "attempted-password",
    )
    .await
    .expect_err("wrong credentials must be rejected");
    assert_eq!(
        error
            .sql_exception()
            .and_then(|exception| exception.sql_state()),
        Some("28000")
    );
    let diagnostic = String::from_utf8(output.lock().expect("output lock").clone())
        .expect("diagnostic is UTF-8");
    assert!(diagnostic.contains("operation=set_login_timeout"));
    assert!(diagnostic.contains("operation=connection_credentials"));
    assert!(diagnostic.contains("sql_state=28000"));
    for secret in [
        "expected-user",
        "expected-password",
        "attempted-user",
        "attempted-password",
    ] {
        assert!(!diagnostic.contains(secret), "diagnostic leaked {secret}");
    }

    source.inner().close().await;
}
