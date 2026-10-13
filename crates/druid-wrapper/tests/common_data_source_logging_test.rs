use druid::core::{Connection, ConnectionFactory, RdbcError};
use druid::pool::DruidPool;
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
