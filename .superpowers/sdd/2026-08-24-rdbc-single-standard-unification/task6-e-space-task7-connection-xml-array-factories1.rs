use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{Array, SqlXml};
use druid_rdbc::types::{RdbcObject, Value};
use std::cell::Cell;
use std::future::Future;
/// 记录标准创建请求的外部连接，明确不支持实际数据库资源创建。
/// 对应 Java: java.sql.Connection。
#[derive(Debug, Default)]
struct RecordingConnection {
    calls: Vec<(Option<String>, Option<Vec<RdbcObject>>)>,
    xml_calls: Cell<usize>,
    closed: bool,
}
druid_rdbc::impl_standard_wrapper!(RecordingConnection => Connection);
#[async_trait::async_trait]
impl Connection for RecordingConnection {
    async fn create_sql_xml(&mut self) -> Result<Box<dyn SqlXml>, RdbcError> {
        if self.closed { return Err(RdbcError::ConnectionDiscarded); }
        self.xml_calls.set(self.xml_calls.get()+1);
        Err(RdbcError::feature_not_supported("recording_create_sql_xml"))
    }
    async fn create_array_of(&mut self, type_name: Option<&str>, elements: Option<Vec<RdbcObject>>) -> Result<Box<dyn Array>, RdbcError> {
        if self.closed { return Err(RdbcError::ConnectionDiscarded); }
        self.calls.push((type_name.map(str::to_owned), elements));
        Err(RdbcError::feature_not_supported("recording_create_array_of"))
    }
    async fn close(&mut self) -> Result<(), RdbcError> { self.closed=true; Ok(()) }
    fn is_closed(&self) -> Result<bool, RdbcError> { Ok(self.closed) }
}
#[derive(Debug)]
struct UnsupportedConnection;
druid_rdbc::impl_standard_wrapper!(UnsupportedConnection => Connection);
#[async_trait::async_trait] impl Connection for UnsupportedConnection {}
fn send_future<F:Future+Send>(future:F)->F {future}
fn assert_unsupported(error:RdbcError) {assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");}
#[tokio::test]
async fn default_sql_xml_and_array_factories_are_owned_send_and_fail_closed() {
    let mut connection:Box<dyn Connection>=Box::new(UnsupportedConnection);
    assert_unsupported(send_future(connection.create_sql_xml()).await.err().unwrap());
    assert_unsupported(send_future(connection.create_array_of(Some("INTEGER"),Some(vec![RdbcObject::Integer(7)]))).await.err().unwrap());
}
#[tokio::test]
async fn array_factory_preserves_nullable_type_and_null_empty_element_array_distinction() {
    let mut connection=RecordingConnection::default();
    assert_unsupported(send_future(connection.create_array_of(None,None)).await.err().unwrap());
    assert_unsupported(send_future(connection.create_array_of(Some(""),Some(Vec::new()))).await.err().unwrap());
    assert_eq!(connection.calls,vec![(None,None),(Some(String::new()),Some(Vec::new()))]);
}
#[tokio::test]
async fn array_factory_preserves_vendor_type_order_typed_values_and_sql_null_elements() {
    let mut connection=RecordingConnection::default();
    let elements=vec![RdbcObject::Integer(8),RdbcObject::Scalar(Value::Null),RdbcObject::String("x".into()),RdbcObject::Long(9)];
    assert_unsupported(send_future(connection.create_array_of(Some("schema.VENDOR_ARRAY"),Some(elements.clone()))).await.err().unwrap());
    assert_eq!(connection.calls,vec![(Some("schema.VENDOR_ARRAY".into()),Some(elements))]);
}
#[tokio::test]
async fn sql_xml_and_array_factory_dispatch_rejects_closed_connection_before_driver_work() {
    let mut connection=RecordingConnection::default();
    assert_unsupported(send_future(connection.create_sql_xml()).await.err().unwrap());
    assert_eq!(connection.xml_calls.get(),1);
    connection.close().await.unwrap();
    assert!(matches!(connection.create_sql_xml().await,Err(RdbcError::ConnectionDiscarded)));
    assert!(matches!(connection.create_array_of(Some("INTEGER"),Some(Vec::new())).await,Err(RdbcError::ConnectionDiscarded)));
    assert_eq!(connection.xml_calls.get(),1);assert!(connection.calls.is_empty());
}
