//! Tests for druid-rbdc adapter crate.

use druid_core::*;
use druid_rbdc::RbdcAdapter;

struct MockConn;

#[async_trait::async_trait]
impl Connection for MockConn {
    async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }
    async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![])
    }
    async fn begin(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn commit(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn rollback(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn close(&mut self) -> Result<(), DruidError> { Ok(()) }
    fn driver_name(&self) -> &str { "mock" }
}

#[test]
fn test_rbdc_adapter_new() {
    let adapter = RbdcAdapter::new("postgres://localhost/test");
    drop(adapter);
}

#[test]
fn test_rbdc_adapter_new_string() {
    let adapter = RbdcAdapter::new(String::from("mysql://localhost/db"));
    drop(adapter);
}

#[tokio::test]
async fn test_rbdc_adapter_create_returns_error() {
    let adapter = RbdcAdapter::new("postgres://localhost/test");
    match adapter.create().await {
        Err(e) => {
            let err_msg = format!("{e}");
            assert!(err_msg.contains("rbdc"), "error should mention rbdc: {err_msg}");
        }
        Ok(_) => panic!("expected error"),
    }
}

#[tokio::test]
async fn test_rbdc_adapter_validate_delegates_to_ping() {
    let adapter = RbdcAdapter::new("postgres://localhost/test");
    let mut conn: Box<dyn Connection> = Box::new(MockConn);
    assert!(adapter.validate(&mut conn).await.is_ok());
}
