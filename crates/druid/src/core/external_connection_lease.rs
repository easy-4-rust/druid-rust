//! Canonical connection ownership wrapper for one external-pool lease.

use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::ConnectionCapabilities;
use druid_rdbc::metadata::DatabaseMetaData;
use druid_rdbc::statement::{CallableStatement, PreparedStatement, Statement, StatementOptions};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::ops::{Deref, DerefMut};

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

    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        self.connection_mut().database_meta_data().await
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().commit().await
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().rollback().await
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

    fn capabilities(&self) -> ConnectionCapabilities {
        self.connection().capabilities()
    }

    async fn is_valid(&mut self, timeout_seconds: u32) -> Result<bool, RdbcError> {
        self.connection_mut().is_valid(timeout_seconds).await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.connection_mut().close().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.connection().is_closed()
    }
}
