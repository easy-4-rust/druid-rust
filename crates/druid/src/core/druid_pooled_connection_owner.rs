//! Native pool 的标准 `PooledConnection` owner 与可重复借出的逻辑连接。

use super::druid_pooled_connection::DruidPooledConnection;
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::data_source::{
    ConnectionEventListener, PooledConnection, PooledConnectionEventRegistry,
    PooledConnectionSource, StatementEventListener,
};
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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{Mutex, OwnedMutexGuard};

struct OwnerState {
    connection: Arc<Mutex<Option<DruidPooledConnection>>>,
    closed: AtomicBool,
    lease_active: AtomicBool,
    lease_generation: AtomicU64,
    events: PooledConnectionEventRegistry,
    source: OnceLock<PooledConnectionSource>,
}

struct OwnerLogicalGuard {
    guard: OwnedMutexGuard<Option<DruidPooledConnection>>,
}

impl std::ops::Deref for OwnerLogicalGuard {
    type Target = DruidPooledConnection;
    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().expect("owner connection is open")
    }
}

impl std::ops::DerefMut for OwnerLogicalGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_mut().expect("owner connection is open")
    }
}

/// 持有一个原生 Druid holder，并按标准合同发放可重复借用的逻辑连接。
pub struct DruidPooledConnectionOwner {
    state: Arc<OwnerState>,
}

impl DruidPooledConnectionOwner {
    pub(crate) fn new(mut connection: DruidPooledConnection) -> Self {
        connection.prepare_for_pooled_owner();
        let state = Arc::new(OwnerState {
            connection: Arc::new(Mutex::new(Some(connection))),
            closed: AtomicBool::new(false),
            lease_active: AtomicBool::new(false),
            lease_generation: AtomicU64::new(0),
            events: PooledConnectionEventRegistry::new(),
            source: OnceLock::new(),
        });
        let source: PooledConnectionSource = Arc::new(Mutex::new(Box::new(OwnerEventSource {
            state: Arc::clone(&state),
        })));
        let _ = state.source.set(source);
        Self { state }
    }
}

struct OwnerEventSource {
    state: Arc<OwnerState>,
}

impl Wrapper for OwnerEventSource {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }
    fn canonical_unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>> {
        (target == TypeId::of::<dyn PooledConnection>())
            .then_some(StandardUnwrapped::PooledConnection(self))
    }
}

#[async_trait::async_trait]
impl PooledConnection for OwnerEventSource {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        DruidPooledConnectionOwner::issue_connection(Arc::clone(&self.state)).await
    }
    fn add_connection_event_listener(
        &mut self,
        listener: Arc<dyn ConnectionEventListener>,
    ) -> Result<(), RdbcError> {
        self.state.events.add_connection_event_listener(listener);
        Ok(())
    }
    fn remove_connection_event_listener(
        &mut self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, RdbcError> {
        Ok(self.state.events.remove_connection_event_listener(listener))
    }
    fn add_statement_event_listener(
        &mut self,
        listener: Arc<dyn StatementEventListener>,
    ) -> Result<(), RdbcError> {
        self.state.events.add_statement_event_listener(listener);
        Ok(())
    }
    fn remove_statement_event_listener(
        &mut self,
        listener: &Arc<dyn StatementEventListener>,
    ) -> Result<bool, RdbcError> {
        Ok(self.state.events.remove_statement_event_listener(listener))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        DruidPooledConnectionOwner::close_state(Arc::clone(&self.state)).await
    }
}

impl Wrapper for DruidPooledConnectionOwner {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }
    fn canonical_unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>> {
        (target == TypeId::of::<dyn PooledConnection>())
            .then_some(StandardUnwrapped::PooledConnection(self))
    }
}

#[async_trait::async_trait]
impl PooledConnection for DruidPooledConnectionOwner {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Self::issue_connection(Arc::clone(&self.state)).await
    }
    fn add_connection_event_listener(
        &mut self,
        listener: Arc<dyn ConnectionEventListener>,
    ) -> Result<(), RdbcError> {
        self.state.events.add_connection_event_listener(listener);
        Ok(())
    }
    fn remove_connection_event_listener(
        &mut self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, RdbcError> {
        Ok(self.state.events.remove_connection_event_listener(listener))
    }
    fn add_statement_event_listener(
        &mut self,
        listener: Arc<dyn StatementEventListener>,
    ) -> Result<(), RdbcError> {
        self.state.events.add_statement_event_listener(listener);
        Ok(())
    }
    fn remove_statement_event_listener(
        &mut self,
        listener: &Arc<dyn StatementEventListener>,
    ) -> Result<bool, RdbcError> {
        Ok(self.state.events.remove_statement_event_listener(listener))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Self::close_state(Arc::clone(&self.state)).await
    }
}

impl DruidPooledConnectionOwner {
    async fn issue_connection(state: Arc<OwnerState>) -> Result<Box<dyn Connection>, RdbcError> {
        if state.closed.load(Ordering::Acquire) {
            return Err(RdbcError::PoolClosed);
        }
        if state
            .lease_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(RdbcError::InvalidArgument(
                "pooled owner already has an active logical connection".into(),
            ));
        }
        let generation = state
            .lease_generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        let mut guard = state.connection.lock().await;
        let result = match guard.as_mut() {
            Some(connection) if !state.closed.load(Ordering::Acquire) => {
                connection.activate_owner_logical_lease()
            }
            _ => Err(RdbcError::PoolClosed),
        };
        if let Err(error) = result {
            state.lease_active.store(false, Ordering::Release);
            return Err(error);
        }
        drop(guard);
        Ok(Box::new(DruidPooledConnectionOwnerLogicalConnection {
            state,
            generation,
            closed: false,
        }))
    }

    async fn close_state(state: Arc<OwnerState>) -> Result<(), RdbcError> {
        if state.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let had_active_lease = state.lease_active.swap(false, Ordering::AcqRel);
        if had_active_lease {
            if let Some(source) = state.source.get() {
                state.events.notify_connection_closed(Arc::clone(source));
            }
        }
        let mut guard = state.connection.lock().await;
        if let Some(mut connection) = guard.take() {
            let close_error = if had_active_lease {
                connection.close().await.err()
            } else {
                None
            };
            if let Some(error) = close_error {
                connection.discard_from_pooled_owner();
                return Err(error);
            }
            connection.return_from_pooled_owner();
        }
        Ok(())
    }
}

struct DruidPooledConnectionOwnerLogicalConnection {
    state: Arc<OwnerState>,
    generation: u64,
    closed: bool,
}

impl DruidPooledConnectionOwnerLogicalConnection {
    async fn lock(&self) -> Result<OwnerLogicalGuard, RdbcError> {
        if self.closed
            || self.state.closed.load(Ordering::Acquire)
            || !self.state.lease_active.load(Ordering::Acquire)
            || self.state.lease_generation.load(Ordering::Acquire) != self.generation
        {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let guard = Arc::clone(&self.state.connection).lock_owned().await;
        if self.state.closed.load(Ordering::Acquire)
            || self.state.lease_generation.load(Ordering::Acquire) != self.generation
        {
            return Err(RdbcError::ConnectionDiscarded);
        }
        if guard.is_none() {
            return Err(RdbcError::PoolClosed);
        }
        Ok(OwnerLogicalGuard { guard })
    }

    fn try_lock(&self) -> Result<OwnerBorrowedLogicalGuard<'_>, RdbcError> {
        if self.closed
            || self.state.closed.load(Ordering::Acquire)
            || !self.state.lease_active.load(Ordering::Acquire)
            || self.state.lease_generation.load(Ordering::Acquire) != self.generation
        {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let guard = self
            .state
            .connection
            .try_lock()
            .map_err(|_| RdbcError::ConnectionDiscarded)?;
        if guard.is_none() {
            return Err(RdbcError::PoolClosed);
        }
        Ok(OwnerBorrowedLogicalGuard { guard })
    }

    async fn close_logical(&mut self) -> Result<(), RdbcError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        if self.state.lease_generation.load(Ordering::Acquire) != self.generation
            || !self.state.lease_active.swap(false, Ordering::AcqRel)
        {
            return Ok(());
        }
        if let Some(source) = self.state.source.get() {
            self.state
                .events
                .notify_connection_closed(Arc::clone(source));
        }
        let result = {
            let mut guard = self.state.connection.lock().await;
            match guard.as_mut() {
                Some(connection) => connection.close().await,
                None => Err(RdbcError::PoolClosed),
            }
        };
        result
    }
}

struct OwnerBorrowedLogicalGuard<'a> {
    guard: tokio::sync::MutexGuard<'a, Option<DruidPooledConnection>>,
}
impl std::ops::Deref for OwnerBorrowedLogicalGuard<'_> {
    type Target = DruidPooledConnection;
    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().expect("owner connection is open")
    }
}
impl std::ops::DerefMut for OwnerBorrowedLogicalGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_mut().expect("owner connection is open")
    }
}

impl Wrapper for DruidPooledConnectionOwnerLogicalConnection {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }
    fn canonical_unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>> {
        (target == TypeId::of::<dyn Connection>()).then_some(StandardUnwrapped::Connection(self))
    }
}

#[async_trait::async_trait]
impl Connection for DruidPooledConnectionOwnerLogicalConnection {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        self.lock().await?.exec(sql, params).await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        self.lock().await?.fetch(sql, params).await
    }

    async fn exec_batch(
        &mut self,
        batch: Vec<(String, Vec<Value>)>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.lock().await?.exec_batch(batch).await
    }

    async fn exec_prepared_batch(
        &mut self,
        statement: &mut dyn PreparedStatement,
        parameter_sets: Vec<Vec<Value>>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.lock()
            .await?
            .exec_prepared_batch(statement, parameter_sets)
            .await
    }

    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &mut dyn PreparedStatement,
        parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.lock()
            .await?
            .exec_prepared_parameter_batch(statement, parameter_sets)
            .await
    }

    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        self.lock().await?.create_statement(options).await
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.lock().await?.prepare_statement(sql, options).await
    }

    async fn prepare_call(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        self.lock().await?.prepare_call(sql, options).await
    }

    /// 将生成键模式和列选择原样交给外部池租约中的连接，保留底层错误。
    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: StatementOptions,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.lock()
            .await?
            .prepare_statement_with_generated_keys(sql, options, generated_keys)
            .await
    }

    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        self.lock().await?.database_meta_data().await
    }

    async fn database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
        self.lock().await?.database_product_name().await
    }

    async fn database_product_version(&mut self) -> Result<Option<String>, RdbcError> {
        self.lock().await?.database_product_version().await
    }

    fn driver_major_version(&self) -> Result<i32, RdbcError> {
        self.try_lock()?.driver_major_version()
    }

    fn driver_minor_version(&self) -> Result<i32, RdbcError> {
        self.try_lock()?.driver_minor_version()
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.lock().await?.commit().await
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.lock().await?.rollback().await
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.lock().await?.rollback_to(savepoint).await
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        self.lock().await?.begin().await
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        self.lock().await?.set_savepoint().await
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, RdbcError> {
        self.lock().await?.set_savepoint_named(name).await
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.lock().await?.release_savepoint(savepoint).await
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.try_lock()?.auto_commit()
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.lock().await?.set_auto_commit(enabled).await
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        self.try_lock()?.read_only()
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        self.lock().await?.set_read_only(read_only).await
    }

    async fn set_client_info(&mut self, name: &str, value: Option<&str>) -> Result<(), RdbcError> {
        self.lock().await?.set_client_info(name, value).await
    }

    fn client_info(&self, name: &str) -> Result<Option<String>, RdbcError> {
        self.try_lock()?.client_info(name)
    }

    async fn native_sql(&mut self, sql: &str) -> Result<String, RdbcError> {
        self.lock().await?.native_sql(sql).await
    }

    async fn set_network_timeout(&mut self, timeout: Duration) -> Result<(), RdbcError> {
        self.lock().await?.set_network_timeout(timeout).await
    }

    fn network_timeout(&self) -> Result<Duration, RdbcError> {
        self.try_lock()?.network_timeout()
    }

    fn type_map(&self) -> Result<Option<RdbcTypeMap>, RdbcError> {
        self.try_lock()?.type_map()
    }

    async fn set_type_map(&mut self, type_map: RdbcTypeMap) -> Result<(), RdbcError> {
        self.lock().await?.set_type_map(type_map).await
    }

    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        self.try_lock()?.transaction_isolation()
    }

    async fn set_transaction_isolation(
        &mut self,
        level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.lock().await?.set_transaction_isolation(level).await
    }

    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.try_lock()?.holdability()
    }

    async fn set_holdability(
        &mut self,
        holdability: ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        self.lock().await?.set_holdability(holdability).await
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.try_lock()?.catalog()
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        self.lock().await?.set_catalog(catalog).await
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.try_lock()?.schema()
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        self.lock().await?.set_schema(schema).await
    }

    async fn warnings(&mut self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.lock().await?.warnings().await
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.lock().await?.clear_warnings().await
    }

    async fn create_blob(&mut self) -> Result<Box<dyn Blob>, RdbcError> {
        self.lock().await?.create_blob().await
    }

    async fn create_clob(&mut self) -> Result<Box<dyn Clob>, RdbcError> {
        self.lock().await?.create_clob().await
    }

    async fn create_n_clob(&mut self) -> Result<Box<dyn NClob>, RdbcError> {
        self.lock().await?.create_n_clob().await
    }

    /// 委托外部租约连接创建 SQLXML；对应 Java: Connection#createSQLXML。
    async fn create_sql_xml(&mut self) -> Result<Box<dyn SqlXml>, RdbcError> {
        self.lock().await?.create_sql_xml().await
    }

    /// 将SQL结构创建的nullable类型名与有序属性原样交给外部池连接；返回独立拥有的对象或原错误。
    /// 对应 Java: Connection#createStruct，不引入第二层资源所有者。
    async fn create_struct(
        &mut self,
        type_name: Option<&str>,
        attributes: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Struct>, RdbcError> {
        self.lock()
            .await?
            .create_struct(type_name, attributes)
            .await
    }

    /// 原样委托类型名与元素数组，返回独占 Array；对应 Java: Connection#createArrayOf。
    async fn create_array_of(
        &mut self,
        type_name: Option<&str>,
        elements: Option<Vec<RdbcObject>>,
    ) -> Result<Box<dyn Array>, RdbcError> {
        self.lock()
            .await?
            .create_array_of(type_name, elements)
            .await
    }

    fn capabilities(&self) -> ConnectionCapabilities {
        self.try_lock().map_or_else(
            |_| ConnectionCapabilities::default(),
            |connection| connection.capabilities(),
        )
    }

    async fn is_valid(&mut self, timeout_seconds: u32) -> Result<bool, RdbcError> {
        self.lock().await?.is_valid(timeout_seconds).await
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        self.lock().await?.ping().await
    }

    async fn abort(&mut self) -> Result<(), RdbcError> {
        self.lock().await?.abort().await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.close_logical().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        if self.closed
            || self.state.closed.load(Ordering::Acquire)
            || !self.state.lease_active.load(Ordering::Acquire)
            || self.state.lease_generation.load(Ordering::Acquire) != self.generation
        {
            return Ok(true);
        }
        self.try_lock()?.is_closed()
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        self.try_lock()?.driver_name()
    }
}
