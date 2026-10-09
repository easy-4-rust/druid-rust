use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{Blob, Clob, NClob};
use std::cell::Cell;
use std::future::Future;
#[path="task7-connection-resource-blob-fixture1.rs"] mod blob_fixture;
#[path="task7-connection-resource-clob-fixture1.rs"] mod clob_fixture;
/// 独占可变连接夹具；对应 Java: java.sql.Connection。
#[derive(Debug)]
struct ResourceConnection { closed: Cell<bool> }
druid_rdbc::impl_standard_wrapper!(ResourceConnection => Connection);
#[async_trait::async_trait]
impl Connection for ResourceConnection {
    async fn create_blob(&mut self) -> Result<Box<dyn Blob>, RdbcError> {
        if self.closed.get() { return Err(RdbcError::ConnectionDiscarded); }
        Ok(blob_fixture::resource())
    }
    async fn create_clob(&mut self) -> Result<Box<dyn Clob>, RdbcError> {
        if self.closed.get() { return Err(RdbcError::ConnectionDiscarded); }
        Ok(clob_fixture::resource())
    }
    async fn create_n_clob(&mut self) -> Result<Box<dyn NClob>, RdbcError> {
        if self.closed.get() { return Err(RdbcError::ConnectionDiscarded); }
        Ok(clob_fixture::national_resource())
    }
    async fn close(&mut self) -> Result<(), RdbcError> { self.closed.set(true); Ok(()) }
    fn is_closed(&self) -> Result<bool, RdbcError> { Ok(self.closed.get()) }
}
#[derive(Debug)]
struct UnsupportedConnection;
druid_rdbc::impl_standard_wrapper!(UnsupportedConnection => Connection);
#[async_trait::async_trait] impl Connection for UnsupportedConnection {}
fn send_future<F: Future + Send>(future: F) -> F { future }
#[tokio::test]
async fn connection_returns_owned_send_only_resource_objects_without_caller_borrow() {
    let (mut blob, mut clob, mut national) = {
        let mut connection: Box<dyn Connection> = Box::new(ResourceConnection { closed: Cell::new(false) });
        let blob = send_future(connection.create_blob()).await.unwrap();
        let clob = send_future(connection.create_clob()).await.unwrap();
        let national = send_future(connection.create_n_clob()).await.unwrap();
        (blob, clob, national)
    };
    assert_eq!(send_future(blob.length()).await.unwrap(), 4);
    assert_eq!(send_future(clob.length()).await.unwrap(), 4);
    assert_eq!(send_future(national.length()).await.unwrap(), 4);
    blob.free().await.unwrap(); clob.free().await.unwrap(); national.free().await.unwrap();
    assert!(blob.length().await.is_err()); assert!(clob.length().await.is_err()); assert!(national.length().await.is_err());
}
#[tokio::test]
async fn connection_creates_distinct_resources_and_rejects_creation_after_close() {
    let mut connection = ResourceConnection { closed: Cell::new(false) };
    let mut first = connection.create_blob().await.unwrap();
    let mut second = connection.create_blob().await.unwrap();
    first.set_bytes(1, &[9]).await.unwrap(); first.free().await.unwrap();
    assert_eq!(second.get_bytes(1,4).await.unwrap(), vec![1,2,3,4]);
    connection.close().await.unwrap(); assert!(connection.is_closed().unwrap());
    assert!(matches!(connection.create_blob().await, Err(RdbcError::ConnectionDiscarded)));
    assert!(matches!(connection.create_clob().await, Err(RdbcError::ConnectionDiscarded)));
    assert!(matches!(connection.create_n_clob().await, Err(RdbcError::ConnectionDiscarded)));
}
#[tokio::test]
async fn unsupported_connection_resource_factories_fail_closed() {
    let mut connection: Box<dyn Connection> = Box::new(UnsupportedConnection);
    let errors = [connection.create_blob().await.err().unwrap(), connection.create_clob().await.err().unwrap(), connection.create_n_clob().await.err().unwrap()];
    for error in errors { assert_eq!(error.class_name(), "java.sql.SQLFeatureNotSupportedException"); }
}
