//! Druid canonical driver connection holder.

use super::connection_info::ConnectionInfo;
use super::standard_prepared_statement_cache::StandardPreparedStatementCache;
use super::standard_resource_registry::StandardResourceRegistry;
use super::{
    ConnectionDefaults, ConnectionEventListener, DruidError, PreparedStatementCacheStats,
    RdbcError, StatementEventListener,
};
use druid_rdbc::connection::Connection;
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Holder lifecycle state.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// The holder is idle.
    Idle = 0,
    /// The holder is checked out.
    Active = 1,
    /// The holder is being validated.
    Validating = 2,
    /// The holder is closing.
    Closing = 3,
    /// The driver connection is closed.
    Closed = 4,
    /// The holder cannot be reused.
    Error = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionOwnerPolicy {
    NativePool,
    ExternalLease,
}

/// Owns exactly one canonical driver connection and its pool lifecycle state.
pub struct DruidConnectionHolder {
    connection: Option<Box<dyn Connection>>,
    owner: ConnectionOwnerPolicy,
    /// Driver connection identifier.
    pub id: u64,
    /// Driver connection creation instant.
    pub created_at: Instant,
    last_active_at: Mutex<Instant>,
    last_exec_at: Mutex<Instant>,
    last_keep_at: Mutex<Option<Instant>>,
    last_valid_at: Mutex<Option<Instant>>,
    /// Number of successful checkouts.
    pub use_count: AtomicU64,
    keep_alive_check_count: AtomicU64,
    last_not_empty_wait: Mutex<Duration>,
    create_duration: Duration,
    state: AtomicU8,
    discard: AtomicBool,
    init_schema: Mutex<Option<String>>,
    restore_schema_on_recycle: AtomicBool,
    user_password_version: u64,
    connection_properties: Arc<HashMap<String, String>>,
    variables: Option<HashMap<String, Value>>,
    global_variables: Option<HashMap<String, Value>>,
    defaults: ConnectionDefaults,
    statement_pool: Arc<tokio::sync::Mutex<StandardPreparedStatementCache>>,
    resources: Arc<StandardResourceRegistry>,
    pool_prepared_statements: bool,
    prepared_statement_stats: Arc<PreparedStatementCacheStats>,
    connection_event_listeners: RwLock<Vec<Arc<dyn ConnectionEventListener>>>,
    statement_event_listeners: RwLock<Vec<Arc<dyn StatementEventListener>>>,
    /// Most recent SQL fingerprint.
    pub last_fingerprint: Mutex<Option<u64>>,
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for DruidConnectionHolder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidConnectionHolder")
            .field("id", &self.id)
            .field("created_at", &self.created_at)
            .field("use_count", &self.use_count())
            .field("state", &self.state())
            .field("discard", &self.is_discard())
            .field("user_password_version", &self.user_password_version)
            .field("has_connection", &self.has_standard_connection())
            .finish()
    }
}

impl DruidConnectionHolder {
    /// Creates an empty holder for state-only compatibility tests.
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self::new_internal(
            None,
            ConnectionOwnerPolicy::NativePool,
            id,
            Duration::ZERO,
            0,
            None,
            None,
            ConnectionDefaults::default(),
        )
    }

    /// Creates a native-pool holder from a canonical connection.
    pub async fn with_standard_connection(
        connection: Box<dyn Connection>,
        id: u64,
        create_duration: Duration,
        user_password_version: u64,
    ) -> Result<Self, DruidError> {
        Self::with_connection_owner(
            connection,
            id,
            create_duration,
            user_password_version,
            ConnectionOwnerPolicy::NativePool,
        )
        .await
    }

    /// Creates a holder owned by one external-pool lease.
    pub async fn with_external_connection(
        connection: Box<dyn Connection>,
        id: u64,
        create_duration: Duration,
        user_password_version: u64,
    ) -> Result<Self, DruidError> {
        Self::with_connection_owner(
            connection,
            id,
            create_duration,
            user_password_version,
            ConnectionOwnerPolicy::ExternalLease,
        )
        .await
    }

    async fn with_connection_owner(
        mut connection: Box<dyn Connection>,
        id: u64,
        create_duration: Duration,
        user_password_version: u64,
        owner: ConnectionOwnerPolicy,
    ) -> Result<Self, DruidError> {
        let defaults = ConnectionDefaults::capture_standard(connection.as_mut()).await?;
        Ok(Self::new_internal(
            Some(connection),
            owner,
            id,
            create_duration,
            user_password_version,
            None,
            None,
            defaults,
        ))
    }

    pub(crate) async fn with_connection_info(
        mut info: ConnectionInfo,
        id: u64,
        user_password_version: u64,
    ) -> Result<Self, DruidError> {
        let create_duration = info.connect_span();
        let variables = info.take_variables();
        let global_variables = info.take_global_variables();
        let mut connection = info
            .take_connection()
            .ok_or(DruidError::ConnectionDiscarded)?;
        let defaults = ConnectionDefaults::capture_standard(connection.as_mut()).await?;
        Ok(Self::new_internal(
            Some(connection),
            ConnectionOwnerPolicy::NativePool,
            id,
            create_duration,
            user_password_version,
            variables,
            global_variables,
            defaults,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn new_internal(
        connection: Option<Box<dyn Connection>>,
        owner: ConnectionOwnerPolicy,
        id: u64,
        create_duration: Duration,
        user_password_version: u64,
        variables: Option<HashMap<String, Value>>,
        global_variables: Option<HashMap<String, Value>>,
        defaults: ConnectionDefaults,
    ) -> Self {
        let now = Instant::now();
        Self {
            connection,
            owner,
            id,
            created_at: now,
            last_active_at: Mutex::new(now),
            last_exec_at: Mutex::new(now),
            last_keep_at: Mutex::new(None),
            last_valid_at: Mutex::new(None),
            use_count: AtomicU64::new(0),
            keep_alive_check_count: AtomicU64::new(0),
            last_not_empty_wait: Mutex::new(Duration::ZERO),
            create_duration,
            state: AtomicU8::new(ConnectionState::Idle as u8),
            discard: AtomicBool::new(false),
            init_schema: Mutex::new(None),
            restore_schema_on_recycle: AtomicBool::new(false),
            user_password_version,
            connection_properties: Arc::new(HashMap::new()),
            variables,
            global_variables,
            defaults,
            statement_pool: Arc::new(tokio::sync::Mutex::new(
                StandardPreparedStatementCache::new(0),
            )),
            resources: Arc::new(StandardResourceRegistry::default()),
            pool_prepared_statements: false,
            prepared_statement_stats: Arc::new(PreparedStatementCacheStats::default()),
            connection_event_listeners: RwLock::new(Vec::new()),
            statement_event_listeners: RwLock::new(Vec::new()),
            last_fingerprint: Mutex::new(None),
        }
    }

    /// Returns the connection identifier.
    #[must_use]
    pub const fn connection_id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn has_standard_connection(&self) -> bool {
        self.connection.is_some()
    }

    pub(crate) fn standard_connection(&self) -> Option<&(dyn Connection + 'static)> {
        self.connection.as_deref()
    }

    pub(crate) fn standard_connection_mut(&mut self) -> Option<&mut (dyn Connection + 'static)> {
        self.connection.as_deref_mut()
    }

    pub(crate) fn standard_connection_box_mut(&mut self) -> Option<&mut Box<dyn Connection>> {
        self.connection.as_mut()
    }

    pub(crate) fn take_standard_connection(&mut self) -> Option<Box<dyn Connection>> {
        self.connection.take()
    }

    pub(crate) fn is_external_lease(&self) -> bool {
        self.owner == ConnectionOwnerPolicy::ExternalLease
    }

    pub(crate) fn raw_connection_is_unusable(&self) -> bool {
        self.connection
            .as_ref()
            .is_none_or(|connection| connection.is_closed().unwrap_or(true))
    }

    pub(crate) fn standard_resource_registry(&self) -> Arc<StandardResourceRegistry> {
        Arc::clone(&self.resources)
    }

    pub(crate) fn begin_standard_lease(&self) -> Result<u64, RdbcError> {
        self.resources.open_next_generation()
    }

    pub(crate) fn has_open_standard_resources(&self) -> bool {
        self.resources.has_open_resources()
    }

    /// Closes resources, cached statements, and the raw connection exactly once.
    pub async fn close_standard_connection(&mut self) -> Result<(), DruidError> {
        let mut first_error = self
            .resources
            .close_all()
            .await
            .map_err(DruidError::from)
            .err();
        if let Err(error) = self.clear_standard_statement_pool().await {
            first_error.get_or_insert_with(|| error.into());
        }
        if let Some(mut connection) = self.connection.take() {
            if let Err(error) = connection.close().await {
                first_error.get_or_insert_with(|| error.into());
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Invalidates every idle cached PreparedStatement and closes each handle once.
    pub(crate) async fn clear_standard_statement_pool(&mut self) -> Result<(), RdbcError> {
        let (cached, pending_close) = self.statement_pool.lock().await.clear();
        let mut first_error = None;
        let mut failed = Vec::new();
        for mut statement in cached {
            if let Err(error) = statement.close().await {
                first_error.get_or_insert(error);
                failed.push(statement);
            }
            self.prepared_statement_stats.record_cache_delete();
        }
        for mut statement in pending_close {
            if let Err(error) = statement.close().await {
                first_error.get_or_insert(error);
                failed.push(statement);
            }
        }
        if !failed.is_empty() {
            self.statement_pool.lock().await.retain_failed_close(failed);
        }
        first_error.map_or(Ok(()), Err)
    }

    #[must_use]
    pub fn variables(&self) -> Option<&HashMap<String, Value>> {
        self.variables.as_ref()
    }

    #[must_use]
    pub fn global_variables(&self) -> Option<&HashMap<String, Value>> {
        self.global_variables.as_ref()
    }

    pub(crate) fn set_connection_properties(
        &mut self,
        connection_properties: Arc<HashMap<String, String>>,
    ) {
        self.connection_properties = connection_properties;
    }

    pub(crate) fn connection_properties(&self) -> Arc<HashMap<String, String>> {
        Arc::clone(&self.connection_properties)
    }

    /// Configures the canonical per-connection prepared statement cache.
    pub fn configure_statement_pool(
        &mut self,
        enabled: bool,
        capacity: usize,
        _share_prepared_statements: bool,
        _use_oracle_implicit_cache: bool,
        stats: Arc<PreparedStatementCacheStats>,
    ) {
        self.statement_pool = Arc::new(tokio::sync::Mutex::new(
            StandardPreparedStatementCache::new(if enabled { capacity } else { 0 }),
        ));
        self.pool_prepared_statements = enabled;
        self.prepared_statement_stats = stats;
    }

    #[must_use]
    pub fn is_pool_prepared_statements(&self) -> bool {
        self.pool_prepared_statements
    }

    pub(crate) fn standard_statement_pool(
        &self,
    ) -> Arc<tokio::sync::Mutex<StandardPreparedStatementCache>> {
        Arc::clone(&self.statement_pool)
    }

    #[must_use]
    pub fn prepared_statement_stats(&self) -> &Arc<PreparedStatementCacheStats> {
        &self.prepared_statement_stats
    }

    /// Restores the captured connection state and listener scopes.
    pub async fn reset(&mut self, _keep_transaction_isolation: bool) -> Result<(), DruidError> {
        let defaults = self.defaults.clone();
        let reset_schema = !self.should_restore_schema_on_recycle();
        defaults
            .reset_standard_state(
                self.standard_connection_mut()
                    .ok_or(DruidError::ConnectionDiscarded)?,
                reset_schema,
            )
            .await?;
        self.clear_connection_event_listeners();
        self.clear_statement_event_listeners();
        if defaults.standard_warnings_supported() {
            self.standard_connection_mut()
                .ok_or(DruidError::ConnectionDiscarded)?
                .clear_warnings()
                .await?;
        }
        Ok(())
    }

    #[must_use]
    pub const fn defaults(&self) -> &ConnectionDefaults {
        &self.defaults
    }

    pub fn add_connection_event_listener(&self, listener: Arc<dyn ConnectionEventListener>) {
        self.connection_event_listeners.write().push(listener);
    }

    pub fn remove_connection_event_listener(
        &self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> bool {
        let mut listeners = self.connection_event_listeners.write();
        let Some(index) = listeners
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, listener))
        else {
            return false;
        };
        listeners.remove(index);
        true
    }

    #[must_use]
    pub fn connection_event_listeners(&self) -> Vec<Arc<dyn ConnectionEventListener>> {
        self.connection_event_listeners.read().clone()
    }

    pub fn clear_connection_event_listeners(&self) {
        self.connection_event_listeners.write().clear();
    }

    pub fn add_statement_event_listener(&self, listener: Arc<dyn StatementEventListener>) {
        self.statement_event_listeners.write().push(listener);
    }

    pub fn remove_statement_event_listener(
        &self,
        listener: &Arc<dyn StatementEventListener>,
    ) -> bool {
        let mut listeners = self.statement_event_listeners.write();
        let Some(index) = listeners
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, listener))
        else {
            return false;
        };
        listeners.remove(index);
        true
    }

    #[must_use]
    pub fn statement_event_listeners(&self) -> Vec<Arc<dyn StatementEventListener>> {
        self.statement_event_listeners.read().clone()
    }

    pub fn clear_statement_event_listeners(&self) {
        self.statement_event_listeners.write().clear();
    }

    #[must_use]
    pub fn state(&self) -> ConnectionState {
        match self.state.load(Ordering::Acquire) {
            0 => ConnectionState::Idle,
            1 => ConnectionState::Active,
            2 => ConnectionState::Validating,
            3 => ConnectionState::Closing,
            4 => ConnectionState::Closed,
            _ => ConnectionState::Error,
        }
    }

    pub fn try_transition(&self, from: ConnectionState, to: ConnectionState) -> bool {
        self.state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    pub fn mark_active(&self) -> bool {
        if self.try_transition(ConnectionState::Idle, ConnectionState::Active) {
            self.use_count.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn mark_idle(&self) -> bool {
        *self
            .last_active_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Instant::now();
        self.try_transition(ConnectionState::Active, ConnectionState::Idle)
    }

    #[must_use]
    pub fn use_count(&self) -> u64 {
        self.use_count.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn connection_age(&self) -> Duration {
        self.created_at.elapsed()
    }

    #[must_use]
    pub fn idle_duration(&self) -> Duration {
        self.last_active_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .elapsed()
    }

    #[must_use]
    pub fn last_exec_idle_duration(&self) -> Duration {
        self.last_exec_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .elapsed()
    }

    pub fn record_execute(&self) {
        *self
            .last_exec_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Instant::now();
    }

    pub fn record_valid(&self) {
        *self
            .last_valid_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
    }

    pub fn record_keep_alive(&self) {
        *self
            .last_keep_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
    }

    #[must_use]
    pub fn last_keep_elapsed(&self) -> Option<Duration> {
        self.last_keep_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map(|instant| instant.elapsed())
    }

    #[must_use]
    pub fn last_valid_elapsed(&self) -> Option<Duration> {
        self.last_valid_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map(|instant| instant.elapsed())
    }

    pub fn increment_keep_alive_check_count(&self) {
        self.keep_alive_check_count.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn keep_alive_check_count(&self) -> u64 {
        self.keep_alive_check_count.load(Ordering::Relaxed)
    }

    pub fn set_last_not_empty_wait(&self, duration: Duration) {
        *self
            .last_not_empty_wait
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = duration;
    }

    #[must_use]
    pub fn last_not_empty_wait(&self) -> Duration {
        *self
            .last_not_empty_wait
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[must_use]
    pub const fn create_duration(&self) -> Duration {
        self.create_duration
    }

    pub fn mark_discarded(&mut self) {
        self.discard.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_discard(&self) -> bool {
        self.discard.load(Ordering::Acquire) || self.raw_connection_is_unusable()
    }

    pub fn remember_initial_schema(&self, schema: Option<String>) {
        let mut initial = self
            .init_schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if initial.is_none() {
            *initial = schema;
        }
    }

    pub fn set_restore_schema_on_recycle(&self, enabled: bool) {
        self.restore_schema_on_recycle
            .store(enabled, Ordering::Release);
    }

    #[must_use]
    pub fn should_restore_schema_on_recycle(&self) -> bool {
        self.restore_schema_on_recycle.load(Ordering::Acquire)
    }

    pub async fn restore_initial_schema(&mut self) -> Result<(), DruidError> {
        let schema = self
            .init_schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(schema) = schema else {
            return Ok(());
        };
        self.standard_connection_mut()
            .ok_or(DruidError::ConnectionDiscarded)?
            .set_schema(Some(&schema))
            .await?;
        *self
            .init_schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }

    #[must_use]
    pub const fn user_password_version(&self) -> u64 {
        self.user_password_version
    }

    #[must_use]
    pub fn is_alive(&self, idle_timeout: Duration) -> bool {
        !matches!(
            self.state(),
            ConnectionState::Closed | ConnectionState::Error
        ) && self.idle_duration() < idle_timeout
    }

    #[must_use]
    pub fn held_duration(&self) -> Duration {
        self.idle_duration()
    }
}
