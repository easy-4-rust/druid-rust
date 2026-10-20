//! 对应冻结 MockPreparedStatement 的属性与查询；池化和恢复由真实逻辑代理执行。
use crate::standard_test_support::{TrackingFactory, TrackingState};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    FetchDirection, PreparedParameter, PreparedStatement, Statement, StatementOptions,
};
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
    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        assert_eq!(sql, "select ?");
        let inner = self.inner.prepare_statement(sql, options).await?;
        let raw = Box::new(MockPrepared {
            inner,
            properties: [0; 5],
        });
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
struct MockPrepared {
    inner: Box<dyn PreparedStatement>,
    properties: [i32; 5],
}
druid_rdbc::impl_standard_wrapper!(MockPrepared => PreparedStatement);
#[async_trait::async_trait]
impl Statement for MockPrepared {
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        Ok(self.properties[0])
    }
    fn set_fetch_size(&mut self, value: i32) -> Result<(), RdbcError> {
        self.properties[0] = value;
        Ok(())
    }
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        Ok(FetchDirection::DriverValue(self.properties[1]))
    }
    fn set_fetch_direction(&mut self, value: FetchDirection) -> Result<(), RdbcError> {
        self.properties[1] = value.rdbc_code();
        Ok(())
    }
    fn max_rows(&self) -> Result<i64, RdbcError> {
        Ok(i64::from(self.properties[2]))
    }
    fn set_max_rows(&mut self, value: i64) -> Result<(), RdbcError> {
        self.properties[2] = i32::try_from(value).unwrap();
        Ok(())
    }
    fn max_field_size(&self) -> Result<i32, RdbcError> {
        Ok(self.properties[3])
    }
    fn set_max_field_size(&mut self, value: i32) -> Result<(), RdbcError> {
        self.properties[3] = value;
        Ok(())
    }
    fn query_timeout(&self) -> Result<i32, RdbcError> {
        Ok(self.properties[4])
    }
    fn set_query_timeout(&mut self, value: i32) -> Result<(), RdbcError> {
        self.properties[4] = value;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.inner.is_closed()
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.inner.close().await
    }
}
#[async_trait::async_trait]
impl PreparedStatement for MockPrepared {
    fn sql(&self) -> Result<&str, RdbcError> {
        self.inner.sql()
    }
    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.inner.set_parameter(parameter)
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.inner.clear_parameters()
    }
    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.execute_prepared_query().await
    }
}
