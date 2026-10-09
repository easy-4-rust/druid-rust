use super::stateful_connection::StatefulConnection;
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::statement::ResultSetHoldability;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 在真实可变状态上记录 getter/setter 顺序，只在指定操作注入精确错误。
/// 对应 Java: java.sql.Connection 的状态 getter/setter。
pub(super) struct CaptureProbeConnection {
    pub(super) state: StatefulConnection,
    pub(super) errors: HashMap<&'static str, RdbcError>,
    pub(super) calls: Arc<Mutex<Vec<&'static str>>>,
}

impl CaptureProbeConnection {
    fn record(&self, operation: &'static str) -> Result<(), RdbcError> {
        self.calls.lock().unwrap().push(operation);
        match self.errors.get(operation) {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

druid_rdbc::impl_standard_wrapper!(CaptureProbeConnection => Connection);

#[async_trait::async_trait]
impl Connection for CaptureProbeConnection {
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.record("auto_commit")?;
        self.state.auto_commit()
    }
    async fn set_auto_commit(&mut self, value: bool) -> Result<(), RdbcError> {
        self.record("set_auto_commit")?;
        self.state.set_auto_commit(value).await
    }
    fn read_only(&self) -> Result<bool, RdbcError> {
        self.record("read_only")?;
        self.state.read_only()
    }
    async fn set_read_only(&mut self, value: bool) -> Result<(), RdbcError> {
        self.record("set_read_only")?;
        self.state.set_read_only(value).await
    }
    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        self.record("transaction_isolation")?;
        self.state.transaction_isolation()
    }
    async fn set_transaction_isolation(
        &mut self,
        value: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.record("set_transaction_isolation")?;
        self.state.set_transaction_isolation(value).await
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.record("holdability")?;
        self.state.holdability()
    }
    async fn set_holdability(&mut self, value: ResultSetHoldability) -> Result<(), RdbcError> {
        self.record("set_holdability")?;
        self.state.set_holdability(value).await
    }
    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.record("catalog")?;
        self.state.catalog()
    }
    async fn set_catalog(&mut self, value: Option<&str>) -> Result<(), RdbcError> {
        self.record("set_catalog")?;
        self.state.set_catalog(value).await
    }
    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.record("schema")?;
        self.state.schema()
    }
    async fn set_schema(&mut self, value: Option<&str>) -> Result<(), RdbcError> {
        self.record("set_schema")?;
        self.state.set_schema(value).await
    }
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        self.record("warnings")?;
        self.state.warnings().await
    }
}
