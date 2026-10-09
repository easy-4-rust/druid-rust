use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::statement::ResultSetHoldability;

/// 固定 BASE 的可变连接状态，不合成 defaults 或 reset 的执行结果。
/// 对应 Java: java.sql.Connection 的状态 getter/setter。
pub(super) struct StatefulConnection {
    pub(super) auto_commit: bool,
    pub(super) read_only: bool,
    pub(super) isolation: TransactionIsolation,
    pub(super) holdability: ResultSetHoldability,
    pub(super) catalog: String,
    pub(super) schema: String,
}
druid_rdbc::impl_standard_wrapper!(StatefulConnection => Connection);
#[async_trait::async_trait]
impl Connection for StatefulConnection {
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(self.auto_commit)
    }
    async fn set_auto_commit(&mut self, value: bool) -> Result<(), RdbcError> {
        self.auto_commit = value;
        Ok(())
    }
    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(self.read_only)
    }
    async fn set_read_only(&mut self, value: bool) -> Result<(), RdbcError> {
        self.read_only = value;
        Ok(())
    }
    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        Ok(self.isolation)
    }
    async fn set_transaction_isolation(
        &mut self,
        value: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.isolation = value;
        Ok(())
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Ok(self.holdability)
    }
    async fn set_holdability(&mut self, value: ResultSetHoldability) -> Result<(), RdbcError> {
        self.holdability = value;
        Ok(())
    }
    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        Ok(Some(self.catalog.clone()))
    }
    async fn set_catalog(&mut self, value: Option<&str>) -> Result<(), RdbcError> {
        self.catalog = value.unwrap_or_default().to_owned();
        Ok(())
    }
    fn schema(&self) -> Result<Option<String>, RdbcError> {
        Ok(Some(self.schema.clone()))
    }
    async fn set_schema(&mut self, value: Option<&str>) -> Result<(), RdbcError> {
        self.schema = value.unwrap_or_default().to_owned();
        Ok(())
    }
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        Ok(None)
    }
}
