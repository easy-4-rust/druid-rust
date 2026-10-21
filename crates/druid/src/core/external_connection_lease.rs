//! Canonical connection ownership wrapper for one external-pool lease.

use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::ConnectionCapabilities;
use druid_rdbc::metadata::DatabaseMetaData;
use druid_rdbc::resource::{Array, Blob, Clob, NClob, SqlXml};
use druid_rdbc::statement::{
    CallableStatement, ExecResult, PreparedInputParameter, PreparedStatement, ResultSetHoldability,
    Statement, StatementOptions,
};
use druid_rdbc::types::{RdbcObject, RdbcTypeMap, Row, Savepoint, Value};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

#[cfg(test)]
mod generated_keys_tests;

/// Owns one bb8/deadpool lease and delegates the canonical connection contract.
pub struct ExternalConnectionLease<L>
where
    L: Deref<Target = Box<dyn Connection>> + DerefMut<Target = Box<dyn Connection>> + Send,
{
    lease: L,
}

impl<L> ExternalConnectionLease<L>
where
    L: Deref<Target = Box<dyn Connection>> + DerefMut<Target = Box<dyn Connection>> + Send,
{
    /// Wraps one checked-out external-pool lease.
    pub fn new(lease: L) -> Self {
        Self { lease }
    }

    fn connection(&self) -> &dyn Connection {
        self.lease.deref().as_ref()
    }

    fn connection_mut(&mut self) -> &mut dyn Connection {
        self.lease.deref_mut().as_mut()
    }
}

impl<L> Wrapper for ExternalConnectionLease<L>
where
    L: Deref<Target = Box<dyn Connection>>
        + DerefMut<Target = Box<dyn Connection>>
        + Send
        + 'static,
{
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }

    fn canonical_unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>> {
        (target == TypeId::of::<dyn Connection>()).then_some(StandardUnwrapped::Connection(self))
    }
}

#[async_trait::async_trait]
impl<L> Connection for ExternalConnectionLease<L>
where
    L: Deref<Target = Box<dyn Connection>>
        + DerefMut<Target = Box<dyn Connection>>
        + Send
        + 'static,
{
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        self.connection_mut().exec(sql, params).await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        self.connection_mut().fetch(sql, params).await
    }

    async fn exec_batch(
        &mut self,
        batch: Vec<(String, Vec<Value>)>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.connection_mut().exec_batch(batch).await
    }

    async fn exec_prepared_batch(
        &mut self,
        statement: &mut dyn PreparedStatement,
        parameter_sets: Vec<Vec<Value>>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.connection_mut()
            .exec_prepared_batch(statement, parameter_sets)
            .await
    }

    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &mut dyn PreparedStatement,
        parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.connection_mut()
            .exec_prepared_parameter_batch(statement, parameter_sets)
            .await
    }

    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        self.connection_mut().create_statement(options).await
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.connection_mut().prepare_statement(sql, options).await
    }

    async fn prepare_call(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        self.connection_mut().prepare_call(sql, options).await
    }

    /// 将生成键模式和列选择原样交给外部池租约中的连接，保留底层错误。
    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: StatementOptions,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.connection_mut()
            .prepare_statement_with_generated_keys(sql, options, generated_keys)
            .await
    }

    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        self.connection_mut().database_meta_data().await
    }

    async fn database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
        self.connection_mut().database_product_name().await
    }

    async fn database_product_version(&mut self) -> Result<Option<String>, RdbcError> {
        self.connection_mut().database_product_version().await
    }

    fn driver_major_version(&self) -> Result<i32, RdbcError> {
        self.connection().driver_major_version()
    }

    fn driver_minor_version(&self) -> Result<i32, RdbcError> {
        self.connection().driver_minor_version()
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().commit().await
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().rollback().await
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.connection_mut().rollback_to(savepoint).await
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().begin().await
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        self.connection_mut().set_savepoint().await
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, RdbcError> {
        self.connection_mut().set_savepoint_named(name).await
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.connection_mut().release_savepoint(savepoint).await
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.connection().auto_commit()
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.connection_mut().set_auto_commit(enabled).await
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        self.connection().read_only()
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        self.connection_mut().set_read_only(read_only).await
    }

    async fn set_client_info(&mut self, name: &str, value: Option<&str>) -> Result<(), RdbcError> {
        let result = self.connection_mut().set_client_info(name, value).await;
        result.map_err(|error| {
            error.with_client_info_failure(std::collections::HashMap::from([(
                name.to_owned(),
                "REASON_UNKNOWN".to_owned(),
            )]))
        })
    }

    fn client_info(&self, name: &str) -> Result<Option<String>, RdbcError> {
        self.connection().client_info(name)
    }

    async fn client_info_properties(
        &mut self,
    ) -> Result<std::collections::HashMap<String, String>, RdbcError> {
        self.connection_mut().client_info_properties().await
    }

    async fn set_client_info_properties(
        &mut self,
        properties: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<(), RdbcError> {
        let result = self
            .connection_mut()
            .set_client_info_properties(properties)
            .await;
        result.map_err(|error| {
            error.with_client_info_failure(
                properties
                    .into_iter()
                    .flat_map(|values| values.keys())
                    .map(|name| (name.clone(), "REASON_UNKNOWN".to_owned()))
                    .collect(),
            )
        })
    }

    async fn native_sql(&mut self, sql: &str) -> Result<String, RdbcError> {
        self.connection_mut().native_sql(sql).await
    }

    async fn set_network_timeout(&mut self, timeout: Duration) -> Result<(), RdbcError> {
        self.connection_mut().set_network_timeout(timeout).await
    }

    fn network_timeout(&self) -> Result<Duration, RdbcError> {
        self.connection().network_timeout()
    }

    fn type_map(&self) -> Result<Option<RdbcTypeMap>, RdbcError> {
        self.connection().type_map()
    }

    async fn set_type_map(&mut self, type_map: RdbcTypeMap) -> Result<(), RdbcError> {
        self.connection_mut().set_type_map(type_map).await
    }

    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        self.connection().transaction_isolation()
    }

    async fn set_transaction_isolation(
        &mut self,
        level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.connection_mut().set_transaction_isolation(level).await
    }

    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.connection().holdability()
    }

    async fn set_holdability(
        &mut self,
        holdability: ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        self.connection_mut().set_holdability(holdability).await
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.connection().catalog()
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        self.connection_mut().set_catalog(catalog).await
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.connection().schema()
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        self.connection_mut().set_schema(schema).await
    }

    async fn warnings(&mut self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.connection_mut().warnings().await
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().clear_warnings().await
    }

    async fn create_blob(&mut self) -> Result<Box<dyn Blob>, RdbcError> {
        self.connection_mut().create_blob().await
    }

    async fn create_clob(&mut self) -> Result<Box<dyn Clob>, RdbcError> {
        self.connection_mut().create_clob().await
    }

    async fn create_n_clob(&mut self) -> Result<Box<dyn NClob>, RdbcError> {
        self.connection_mut().create_n_clob().await
    }

    /// 委托外部租约连接创建 SQLXML；对应 Java: Connection#createSQLXML。
    async fn create_sql_xml(&mut self) -> Result<Box<dyn SqlXml>, RdbcError> {
        self.connection_mut().create_sql_xml().await
    }

    /// 将SQL结构创建的nullable类型名与有序属性原样交给外部池连接；返回独立拥有的对象或原错误。
    /// 对应 Java: Connection#createStruct，不引入第二层资源所有者。
    async fn create_struct(
        &mut self,
        type_name: Option<&str>,
        attributes: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Struct>, RdbcError> {
        self.connection_mut()
            .create_struct(type_name, attributes)
            .await
    }

    /// 原样委托类型名与元素数组，返回独占 Array；对应 Java: Connection#createArrayOf。
    async fn create_array_of(
        &mut self,
        type_name: Option<&str>,
        elements: Option<Vec<RdbcObject>>,
    ) -> Result<Box<dyn Array>, RdbcError> {
        self.connection_mut()
            .create_array_of(type_name, elements)
            .await
    }

    fn capabilities(&self) -> ConnectionCapabilities {
        self.connection().capabilities()
    }

    async fn is_valid(&mut self, timeout_seconds: u32) -> Result<bool, RdbcError> {
        self.connection_mut().is_valid(timeout_seconds).await
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().ping().await
    }

    async fn abort(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().abort().await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().close().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.connection().is_closed()
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        self.connection().driver_name()
    }
}
