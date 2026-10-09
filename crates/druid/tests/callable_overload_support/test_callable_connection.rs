use super::{TestCallableState, TestCallableStatement};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{CallableStatement, Statement, StatementOptions};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
/// BASE 准备调用记录连接，记录实际 raw SQL/options 与每个真实句柄。对应 Java: java.sql.Connection。
pub(crate) struct TestCallableConnection {
    pub(crate) prepare_count: Arc<AtomicU64>,
    pub(crate) prepared_options: Arc<Mutex<Vec<(String, StatementOptions)>>>,
    pub(crate) states: Arc<Mutex<Vec<Arc<TestCallableState>>>>,
    pub(crate) closed: bool,
}
druid_rdbc::impl_standard_wrapper!(TestCallableConnection => Connection);
#[async_trait::async_trait]
impl Connection for TestCallableConnection {
    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        let mut validation = crate::standard_test_support::TrackingConnection::new(
            crate::standard_test_support::TrackingState::configured(),
        );
        validation.create_statement(options).await
    }
    async fn prepare_call(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        self.prepare_count.fetch_add(1, Ordering::Relaxed);
        self.prepared_options
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((sql.to_owned(), options));
        let state = Arc::new(TestCallableState::default());
        self.states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Arc::clone(&state));
        Ok(Box::new(TestCallableStatement::new(sql, state)))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn is_valid(&mut self, _timeout: u32) -> Result<bool, RdbcError> {
        Ok(!self.closed)
    }
    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        Ok(None)
    }
}
