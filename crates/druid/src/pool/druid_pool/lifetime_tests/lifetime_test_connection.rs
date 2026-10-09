use crate::core::{Connection, ExecResult, Row, Value};
use druid_rdbc::error::RdbcError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 可观测物理关闭次数的寿命测试驱动。对应 Java：java.sql.Connection 测试替身。
#[derive(Debug)]
pub(super) struct LifetimeTestConnection {
    pub(super) closed: bool,
    pub(super) closes: Arc<AtomicU64>,
}

druid_rdbc::impl_standard_wrapper!(LifetimeTestConnection => Connection);

#[async_trait::async_trait]
impl Connection for LifetimeTestConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult::default())
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Ok(Vec::new())
    }
    async fn begin(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn commit(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn rollback(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.closed = true;
            self.closes.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("fault-test".to_owned())
    }
}
