use super::driver_state::DriverState;
use super::prepared::HandoffPrepared;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{CallableStatement, PreparedStatement, Statement, StatementOptions};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 创建标准语句并记录关闭的受控连接。
/// 对应 Java: java.sql.Connection。
pub struct HandoffConnection(pub Arc<DriverState>);
druid_rdbc::impl_standard_wrapper!(HandoffConnection => Connection);
#[async_trait::async_trait]
impl Connection for HandoffConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(HandoffPrepared(Arc::clone(&self.0))))
    }
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Ok(Box::new(HandoffPrepared(Arc::clone(&self.0))))
    }
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(Box::new(HandoffPrepared(Arc::clone(&self.0))))
    }
    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.0.connection_closed.store(true, Ordering::Release);
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.0.connection_closed.load(Ordering::Acquire))
    }
}
