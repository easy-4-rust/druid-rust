//! Tests for druid-sqlx-bb8 adapter crate.

use druid_core::*;
use druid_sqlx_bb8::SqlxBb8Adapter;

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
fn test_sqlx_bb8_adapter_new() {
    let adapter = SqlxBb8Adapter::new("postgres://localhost/test");
    drop(adapter);
}

#[test]
fn test_sqlx_bb8_adapter_new_string() {
    let adapter = SqlxBb8Adapter::new(String::from("mysql://localhost/db"));
    drop(adapter);
}

#[tokio::test]
async fn test_sqlx_bb8_adapter_create_returns_error() {
    let adapter = SqlxBb8Adapter::new("postgres://localhost/test");
    match adapter.create().await {
        Err(e) => {
            let err_msg = format!("{e}");
            assert!(err_msg.contains("sqlx-bb8"), "error should mention sqlx-bb8: {err_msg}");
        }
        Ok(_) => panic!("expected error"),
    }
}

#[tokio::test]
async fn test_sqlx_bb8_adapter_validate_delegates_to_ping() {
    let adapter = SqlxBb8Adapter::new("postgres://localhost/test");
    let mut conn: Box<dyn Connection> = Box::new(MockConn);
    assert!(adapter.validate(&mut conn).await.is_ok());
}
