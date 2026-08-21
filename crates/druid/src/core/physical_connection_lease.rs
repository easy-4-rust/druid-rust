//! 外部连接池租约到物理连接 SPI 的透明桥接。

use super::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalPreparedStatement, PhysicalResultSet, PreparedInputParameter, Row, Savepoint,
    SqlWarning, StatementExecuteResult, StatementGeneratedKeys, Value,
};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// 外部连接池租约的透明物理连接适配器。
///
/// 对应 Java: `DruidPooledConnection` 持有的底层连接句柄。该对象不创建、
/// 缓存或调度连接，只拥有一次外部池租约，并把所有连接操作委托给租约中的
/// `PhysicalConnection`。对象析构时，租约按外部池自身规则归还。
pub struct PhysicalConnectionLease<L>
where
    L: Deref<Target = Box<dyn PhysicalConnection>>
        + DerefMut<Target = Box<dyn PhysicalConnection>>
        + Send,
{
    lease: L,
}

impl<L> PhysicalConnectionLease<L>
where
    L: Deref<Target = Box<dyn PhysicalConnection>>
        + DerefMut<Target = Box<dyn PhysicalConnection>>
        + Send,
{
    /// 创建外部连接池租约桥接。
    ///
    /// 参数 `lease` 必须拥有一个已借出的物理连接；返回的桥接对象被丢弃时，
    /// `lease` 随之析构并由外部池完成归还。
    pub fn new(lease: L) -> Self {
        Self { lease }
    }

    fn physical_connection(&self) -> &dyn PhysicalConnection {
        self.lease.deref().as_ref()
    }

    fn physical_connection_mut(&mut self) -> &mut dyn PhysicalConnection {
        self.lease.deref_mut().as_mut()
    }
}

#[async_trait::async_trait]
impl<L> PhysicalConnection for PhysicalConnectionLease<L>
where
    L: Deref<Target = Box<dyn PhysicalConnection>>
        + DerefMut<Target = Box<dyn PhysicalConnection>>
        + Send
        + 'static,
{
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        self.physical_connection_mut().exec(sql, params).await
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        self.physical_connection_mut()
            .execute(sql, params, generated_keys)
            .await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.physical_connection_mut().fetch(sql, params).await
    }

    async fn fetch_result_set(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.physical_connection_mut()
            .fetch_result_set(sql, params)
            .await
    }

    async fn exec_prepared_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: Vec<Vec<Value>>,
    ) -> Result<Vec<i32>, DruidError> {
        self.physical_connection_mut()
            .exec_prepared_batch(statement, parameter_sets)
            .await
    }

    async fn exec_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<ExecResult, DruidError> {
        self.physical_connection_mut()
            .exec_prepared_parameters(statement, parameters)
            .await
    }

    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i32>, DruidError> {
        self.physical_connection_mut()
            .exec_prepared_parameter_batch(statement, parameter_sets)
            .await
    }

    async fn execute_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        self.physical_connection_mut()
            .execute_prepared(statement, params, generated_keys)
            .await
    }

    async fn execute_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        self.physical_connection_mut()
            .execute_prepared_parameters(statement, parameters, generated_keys)
            .await
    }

    async fn fetch_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Vec<Row>, DruidError> {
        self.physical_connection_mut()
            .fetch_prepared_parameters(statement, parameters)
            .await
    }

    async fn fetch_prepared_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.physical_connection_mut()
            .fetch_prepared_result_set(statement, params)
            .await
    }

    async fn fetch_prepared_parameters_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        self.physical_connection_mut()
            .fetch_prepared_parameters_result_set(statement, parameters)
            .await
    }

    async fn close_prepared_statement(
        &mut self,
        statement: Arc<dyn PhysicalPreparedStatement>,
    ) -> Result<(), DruidError> {
        self.physical_connection_mut()
            .close_prepared_statement(statement)
            .await
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        self.physical_connection_mut().begin().await
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        self.physical_connection_mut().commit().await
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        self.physical_connection_mut().rollback().await
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.physical_connection_mut().rollback_to(savepoint).await
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        self.physical_connection_mut().set_savepoint().await
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, DruidError> {
        self.physical_connection_mut()
            .set_savepoint_named(name)
            .await
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.physical_connection_mut()
            .release_savepoint(savepoint)
            .await
    }

    async fn abort(&mut self) -> Result<(), DruidError> {
        self.physical_connection_mut().abort().await
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        self.physical_connection_mut().ping().await
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.physical_connection_mut().close().await
    }

    fn is_closed(&self) -> bool {
        self.physical_connection().is_closed()
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        self.physical_connection().capabilities()
    }

    fn auto_commit(&self) -> bool {
        self.physical_connection().auto_commit()
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        self.physical_connection_mut()
            .set_auto_commit(auto_commit)
            .await
    }

    fn read_only(&self) -> bool {
        self.physical_connection().read_only()
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), DruidError> {
        self.physical_connection_mut()
            .set_read_only(read_only)
            .await
    }

    fn transaction_isolation(&self) -> u8 {
        self.physical_connection().transaction_isolation()
    }

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), DruidError> {
        self.physical_connection_mut()
            .set_transaction_isolation(level)
            .await
    }

    fn holdability(&self) -> i32 {
        self.physical_connection().holdability()
    }

    async fn set_holdability(&mut self, holdability: i32) -> Result<(), DruidError> {
        self.physical_connection_mut()
            .set_holdability(holdability)
            .await
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        self.physical_connection_mut().warnings().await
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.physical_connection_mut().clear_warnings().await
    }

    fn mark_discarded(&mut self) {
        self.physical_connection_mut().mark_discarded();
    }

    fn is_discarded(&self) -> bool {
        self.physical_connection().is_discarded()
    }

    fn catalog(&self) -> Option<&str> {
        self.physical_connection().catalog()
    }

    async fn set_catalog(&mut self, catalog: &str) -> Result<(), DruidError> {
        self.physical_connection_mut().set_catalog(catalog).await
    }

    fn schema(&self) -> Option<&str> {
        self.physical_connection().schema()
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        self.physical_connection_mut().set_schema(schema).await
    }

    fn driver_name(&self) -> &str {
        self.physical_connection().driver_name()
    }
}
