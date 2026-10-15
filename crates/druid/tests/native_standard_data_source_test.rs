//! 外部消费者通过唯一 RDBC DataSource 合同借用 Druid 原生池。

use std::any::TypeId;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use druid::core::{Connection, ConnectionFactory};
use druid::pool::{DruidDataSource, DruidPool};
use druid_rdbc::data_source::DataSource as StandardDataSource;
use druid_rdbc::error::RdbcError;
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};

struct PhysicalConnection;
druid_rdbc::impl_standard_wrapper!(PhysicalConnection => Connection);

#[async_trait::async_trait]
impl Connection for PhysicalConnection {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

struct CountingFactory(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl ConnectionFactory for CountingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(PhysicalConnection))
    }
}

async fn source(creates: Arc<AtomicUsize>) -> DruidDataSource {
    let pool = DruidPool::builder()
        .name("native-standard-data-source")
        .driver_name("counting")
        .factory(Arc::new(CountingFactory(creates)))
        .max_open(1)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .build()
        .await
        .expect("build native pool");
    DruidDataSource::from_pool(pool)
}

#[tokio::test]
async fn standard_consumer_reuses_the_same_native_pool() {
    let creates = Arc::new(AtomicUsize::new(0));
    let source = source(Arc::clone(&creates)).await;
    let standard: &dyn StandardDataSource = &source;

    let mut first = tokio::time::timeout(std::time::Duration::from_secs(5), standard.connection())
        .await
        .expect("first acquisition must finish")
        .expect("borrow first connection");
    assert_eq!(source.state().active_count, 1);
    first.close().await.expect("return first connection");
    drop(first);
    assert_eq!(source.state().active_count, 0);
    assert_eq!(source.state().idle_count, 1);

    let mut second = tokio::time::timeout(std::time::Duration::from_secs(5), standard.connection())
        .await
        .expect("second acquisition must finish")
        .expect("reuse first holder");
    assert_eq!(creates.load(Ordering::SeqCst), 1);
    second.close().await.expect("return second connection");
    drop(second);
    tokio::time::timeout(std::time::Duration::from_secs(5), source.close())
        .await
        .expect("native pool closes");
}

#[tokio::test]
async fn standard_wrapper_identity_and_unsupported_credentials() {
    let creates = Arc::new(AtomicUsize::new(0));
    let source = source(Arc::clone(&creates)).await;
    let standard: &dyn StandardDataSource = &source;
    let wrapper: &dyn Wrapper = &source;
    let target = TypeId::of::<dyn StandardDataSource>();
    assert!(wrapper
        .is_wrapper_for(Some(target))
        .expect("inspect standard view"));
    match wrapper.unwrap(Some(target)).expect("unwrap standard view") {
        StandardUnwrapped::DataSource(view) => assert!(std::ptr::addr_eq(view, standard)),
        other => panic!("expected DataSource view, got {other:?}"),
    }

    let error = match standard
        .connection_with_credentials("caller", "secret")
        .await
    {
        Ok(_) => panic!("core source cannot verify caller credentials"),
        Err(error) => error,
    };
    assert_eq!(
        error.sql_exception().and_then(|value| value.sql_state()),
        Some("0A000")
    );
    assert_eq!(creates.load(Ordering::SeqCst), 0);
    assert_eq!(source.state().active_count, 0);
    source.close().await;
}
