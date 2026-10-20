//! 只镜像冻结 MockCallableStatement 被七例实际使用的执行、wasNull 与 typed getter 行为。
use crate::standard_test_support::{TrackingFactory, TrackingState};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::statement::{
    CallableParameter, CallableStatement, ExecutionResult, PreparedStatement, Statement,
    StatementOptions,
};
use druid_rdbc::types::{RdbcObject, RdbcTargetType};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub struct Factory(pub Arc<TrackingState>, pub Arc<AtomicUsize>);
#[async_trait::async_trait]
impl ConnectionFactory for Factory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(MockConnection {
            inner: TrackingFactory::new(Arc::clone(&self.0)).create().await?,
            identity: Arc::clone(&self.1),
        }))
    }
}
struct MockConnection {
    inner: Box<dyn Connection>,
    identity: Arc<AtomicUsize>,
}
druid_rdbc::impl_standard_wrapper!(MockConnection => Connection);
#[async_trait::async_trait]
impl Connection for MockConnection {
    async fn prepare_call(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        assert_eq!(sql, "select 1");
        let raw = Box::new(MockCallable { closed: false });
        self.identity
            .store(std::ptr::from_ref(raw.as_ref()) as usize, Ordering::Release);
        Ok(raw)
    }
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.inner.auto_commit()
    }
    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.inner.set_auto_commit(enabled).await
    }
    fn read_only(&self) -> Result<bool, RdbcError> {
        self.inner.read_only()
    }
    async fn set_read_only(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.inner.set_read_only(enabled).await
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.inner.is_closed()
    }
    async fn is_valid(&mut self, seconds: u32) -> Result<bool, RdbcError> {
        self.inner.is_valid(seconds).await
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.inner.close().await
    }
}
struct MockCallable {
    closed: bool,
}
druid_rdbc::impl_standard_wrapper!(MockCallable => CallableStatement);
#[async_trait::async_trait]
impl Statement for MockCallable {
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
}
#[async_trait::async_trait]
impl PreparedStatement for MockCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("select 1")
    }
    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        assert!(!self.closed);
        // 原 MockPreparedStatement#execute 返回 false；七例不请求其延迟 ResultSet。
        Ok(Vec::new())
    }
}
#[async_trait::async_trait]
impl CallableStatement for MockCallable {
    fn was_null(&self) -> Result<bool, RdbcError> {
        if self.closed {
            Err(RdbcError::SqlException(Box::new(SqlException::new(
                0, None, None,
            ))))
        } else {
            Ok(false)
        }
    }
    fn out_parameter_as(
        &self,
        parameter: &CallableParameter,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        assert!(
            matches!(parameter, CallableParameter::Index(index) if index.get()==1)
                || matches!(parameter, CallableParameter::Name(name) if name=="1")
        );
        assert_eq!(target, &RdbcTargetType::String);
        Err(RdbcError::SqlException(Box::new(
            SqlException::new(0, None, None)
                .with_class_name("java.sql.SQLFeatureNotSupportedException"),
        )))
    }
}
