use druid_rdbc::data_source::{
    CommonDataSource, ConnectionPoolDataSource, DataSource, PooledConnection, RdbcLogWriter,
    XaConnection, XaDataSource,
};
use druid_rdbc::error::RdbcError;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Settings {
    writer: Mutex<Option<RdbcLogWriter>>,
}

impl CommonDataSource for Settings {
    fn get_log_writer(&self) -> Option<RdbcLogWriter> {
        self.writer.lock().expect("settings lock").clone()
    }

    fn set_log_writer(&self, writer: Option<RdbcLogWriter>) -> Result<(), RdbcError> {
        *self.writer.lock().expect("settings lock") = writer;
        Ok(())
    }

    fn get_parent_logger(&self) -> Result<&'static str, RdbcError> {
        Ok("consumer.datasource")
    }
}

struct OrdinaryDataSource(Settings);
struct PooledDataSource(Settings);
struct XaSource(Settings);

macro_rules! delegate_common_data_source {
    ($type:ty) => {
        impl CommonDataSource for $type {
            fn get_log_writer(&self) -> Option<RdbcLogWriter> {
                self.0.get_log_writer()
            }

            fn set_log_writer(&self, writer: Option<RdbcLogWriter>) -> Result<(), RdbcError> {
                self.0.set_log_writer(writer)
            }

            fn get_parent_logger(&self) -> Result<&'static str, RdbcError> {
                self.0.get_parent_logger()
            }
        }
    };
}

delegate_common_data_source!(OrdinaryDataSource);
delegate_common_data_source!(PooledDataSource);
delegate_common_data_source!(XaSource);

#[async_trait::async_trait]
impl DataSource for OrdinaryDataSource {
    async fn connection(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        Err(RdbcError::feature_not_supported("test_connection"))
    }
}

#[async_trait::async_trait]
impl ConnectionPoolDataSource for PooledDataSource {
    async fn pooled_connection(&self) -> Result<Box<dyn PooledConnection>, RdbcError> {
        Err(RdbcError::feature_not_supported("test_pooled_connection"))
    }
}

druid_rdbc::impl_standard_wrapper!(OrdinaryDataSource => DataSource);
druid_rdbc::impl_standard_wrapper!(PooledDataSource => ConnectionPoolDataSource);
druid_rdbc::impl_standard_wrapper!(XaSource => XaDataSource);

#[async_trait::async_trait]
impl XaDataSource for XaSource {
    async fn xa_connection(&self) -> Result<Box<dyn XaConnection>, RdbcError> {
        Err(RdbcError::feature_not_supported("test_xa_connection"))
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

fn assert_configurable(source: &dyn CommonDataSource) {
    assert!(source.get_log_writer().is_none());
    let (writer, output) = writer();
    source
        .set_log_writer(Some(Arc::clone(&writer)))
        .expect("set writer");
    let stored = source.get_log_writer().expect("configured writer");
    assert!(Arc::ptr_eq(&stored, &writer));
    writeln!(stored.lock(), "source diagnostic").expect("write diagnostic");
    assert_eq!(
        output.lock().expect("output lock").as_slice(),
        b"source diagnostic\n"
    );
    source.set_log_writer(None).expect("disable writer");
    assert!(source.get_log_writer().is_none());
}

#[test]
fn standard_data_source_families_share_configurable_logging_contract() {
    let ordinary = OrdinaryDataSource(Settings::default());
    let pooled = PooledDataSource(Settings::default());
    let xa = XaSource(Settings::default());

    assert_configurable(&ordinary);
    assert_configurable(&pooled);
    assert_configurable(&xa);
    assert_eq!(ordinary.get_parent_logger(), Ok("consumer.datasource"));
    assert_eq!(pooled.get_parent_logger(), Ok("consumer.datasource"));
    assert_eq!(xa.get_parent_logger(), Ok("consumer.datasource"));
}

struct UnsupportedDataSource;

impl CommonDataSource for UnsupportedDataSource {}

#[test]
fn unsupported_parent_logger_uses_feature_not_supported_sql_state() {
    let error = UnsupportedDataSource
        .get_parent_logger()
        .expect_err("default parent logger is unsupported");
    assert_eq!(
        error
            .sql_exception()
            .and_then(|exception| exception.sql_state()),
        Some("0A000")
    );
}

#[test]
fn data_source_diagnostic_redacts_credentials_and_preserves_safe_error_context() {
    let source = OrdinaryDataSource(Settings::default());
    let (writer, output) = writer();
    source
        .set_log_writer(Some(writer))
        .expect("configure writer");
    let error = RdbcError::SqlException(Box::new(
        druid_rdbc::error::SqlException::new(
            7,
            Some("08001".to_owned()),
            Some("failed for alice:super-secret token=private-token".to_owned()),
        )
        .with_class_name("java.sql.SQLNonTransientConnectionException"),
    ));

    source
        .write_diagnostic("connection", &error)
        .expect("write sanitized diagnostic");

    let output = String::from_utf8(output.lock().expect("output lock").clone())
        .expect("diagnostic is UTF-8");
    assert!(output.contains("operation=connection"));
    assert!(output.contains("sql_state=08001"));
    assert!(!output.contains("alice"));
    assert!(!output.contains("super-secret"));
    assert!(!output.contains("private-token"));
}

#[test]
fn data_source_diagnostic_sink_failure_is_observable_without_echoing_sink_text() {
    let source = OrdinaryDataSource(Settings::default());
    let writer: RdbcLogWriter = Arc::new(parking_lot::Mutex::new(Box::new(FailingWriter)));
    source
        .set_log_writer(Some(writer))
        .expect("configure writer");
    let error = RdbcError::AcquireTimeout;

    let write_error = source
        .write_diagnostic("connection", &error)
        .expect_err("sink failure must be observable");

    assert!(write_error.to_string().contains("diagnostic sink failed"));
    assert!(!write_error.to_string().contains("sink-private-detail"));
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("sink-private-detail"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
