use super::cursor::Cursor;
use super::driver_state::DriverState;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CallableStatement, CurrentResultDisposition, ExecutionResult, GeneratedKeys, PreparedStatement,
    Statement,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 一个同时提供普通、预编译和调用语句视图的受控驱动语句。
/// 对应 Java: java.sql.CallableStatement。
pub struct HandoffPrepared(pub Arc<DriverState>);
druid_rdbc::impl_standard_wrapper!(HandoffPrepared => CallableStatement);
impl HandoffPrepared {
    fn results(&self) -> Vec<ExecutionResult> {
        self.0.current.store(0, Ordering::Release);
        vec![
            ExecutionResult::ResultSet(Box::new(Cursor {
                state: Arc::clone(&self.0.first),
                from_getter: false,
            })),
            ExecutionResult::UpdateCount(2),
            ExecutionResult::ResultSet(Box::new(Cursor {
                state: Arc::clone(&self.0.third),
                from_getter: false,
            })),
        ]
    }
}
#[async_trait::async_trait]
impl Statement for HandoffPrepared {
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.execute_prepared_query().await
    }
    async fn execute(
        &mut self,
        _sql: &str,
        _keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        Ok(self.results())
    }
    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        self.0.getter_calls.fetch_add(1, Ordering::AcqRel);
        let state = match self.0.current.load(Ordering::Acquire) {
            0 => Some(Arc::clone(&self.0.first)),
            2 => Some(Arc::clone(&self.0.third)),
            _ => None,
        };
        Ok(state.map(|state| {
            Box::new(Cursor {
                state,
                from_getter: true,
            }) as Box<dyn ResultSet>
        }))
    }
    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        Ok((self.0.current.load(Ordering::Acquire) == 1).then_some(2))
    }
    async fn get_more_results(
        &mut self,
        _disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        let current = self.0.current.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(current == 2)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.0.statement_closed.store(true, Ordering::Release);
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.0.statement_closed.load(Ordering::Acquire))
    }
}
#[async_trait::async_trait]
impl PreparedStatement for HandoffPrepared {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("MULTI")
    }
    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.0.current.store(0, Ordering::Release);
        Ok(Box::new(Cursor {
            state: Arc::clone(&self.0.first),
            from_getter: false,
        }))
    }
    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        Ok(self.results())
    }
}
#[async_trait::async_trait]
impl CallableStatement for HandoffPrepared {}
