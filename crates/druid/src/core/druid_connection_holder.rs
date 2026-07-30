//! Druid 物理连接持有者。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidConnectionHolder`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/DruidConnectionHolder.java`。

use super::druid_pooled_prepared_statement::DruidPooledPreparedStatementShared;
use super::druid_pooled_statement::{DruidPooledStatement, DruidPooledStatementInner};
use super::{
    ConnectionDefaults, ConnectionEventListener, DruidError, PhysicalConnection,
    PhysicalConnectionInfo, PreparedStatementCacheStats, PreparedStatementHolder,
    PreparedStatementKey, PreparedStatementPool, StatementEventListener,
};
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// holder 持有的逻辑 Statement 强引用。
///
/// 对应 Java：`DruidConnectionHolder#statementTrace`。Java 使用统一的
/// `List<Statement>`；Rust 需要区分普通 Statement 与带缓存归还状态机的
/// Prepared/Callable Statement，确保连接回收时调用各自正确的关闭路径。
pub(crate) enum StatementTrace {
    /// 普通池化 Statement。
    Statement(Arc<DruidPooledStatementInner>),
    /// PreparedStatement；CallableStatement 复用同一共享内核。
    Prepared(Arc<DruidPooledPreparedStatementShared>),
}

impl StatementTrace {
    /// 返回逻辑 Statement 的稳定对象身份。
    pub(crate) fn identity(&self) -> usize {
        match self {
            Self::Statement(statement) => Arc::as_ptr(statement) as usize,
            Self::Prepared(statement) => Arc::as_ptr(statement) as usize,
        }
    }

    /// 按具体 Statement 类型执行幂等关闭。
    fn close(self) {
        match self {
            Self::Statement(statement) => {
                DruidPooledStatement::from_inner(statement).close_embedded_base();
            }
            Self::Prepared(statement) => statement.finish(),
        }
    }
}

/// 连接持有者生命周期状态。
///
/// Java 使用 `active`、`discard` 等字段表达状态；Rust 额外使用原子状态机，
/// 以便连接池统计和兼容 API 能够安全观察状态。
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// 连接位于空闲队列。
    Idle = 0,
    /// 连接已经借出。
    Active = 1,
    /// 连接正在执行有效性检查。
    Validating = 2,
    /// 连接正在关闭。
    Closing = 3,
    /// 物理连接已经关闭。
    Closed = 4,
    /// 连接发生不可恢复错误。
    Error = 5,
}

/// 保存一个物理连接及其完整池生命周期状态。
///
/// 对应 Java：`DruidConnectionHolder`。该对象是 native pool 与
/// `DruidPooledConnection` 之间唯一的物理连接所有者，保存首次入池时的默认
/// 属性、创建/活跃/执行/保活时间、借用次数、密码版本和回收标记。
///
/// PreparedStatement cache 已由同名 `PreparedStatementPool`/holder 对象承载；
/// ConnectionEventListener、StatementEventListener 与 statement trace 已迁入。
pub struct DruidConnectionHolder {
    physical_connection: Option<Box<dyn PhysicalConnection>>,
    /// 物理连接 ID。对应 Java：`connectionId`。
    pub id: u64,
    /// 物理连接创建时刻。对应 Java：`connectTimeMillis`。
    pub created_at: Instant,
    last_active_at: Mutex<Instant>,
    last_exec_at: Mutex<Instant>,
    last_keep_at: Mutex<Option<Instant>>,
    last_valid_at: Mutex<Option<Instant>>,
    /// 连接借用次数。对应 Java：`useCount`。
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
    statement_pool: Option<Arc<Mutex<PreparedStatementPool>>>,
    pool_prepared_statements: bool,
    max_pool_prepared_statements_per_connection: usize,
    share_prepared_statements: bool,
    use_oracle_implicit_cache: bool,
    prepared_statement_stats: Arc<PreparedStatementCacheStats>,
    connection_event_listeners: RwLock<Vec<Arc<dyn ConnectionEventListener>>>,
    statement_event_listeners: RwLock<Vec<Arc<dyn StatementEventListener>>>,
    statement_trace: Mutex<Vec<StatementTrace>>,
    /// 最近一次 SQL 指纹；保留现有 Rust 统计扩展。
    pub last_fingerprint: Mutex<Option<u64>>,
}

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
            .field(
                "variables",
                &self.variables.as_ref().map(std::collections::HashMap::len),
            )
            .field(
                "global_variables",
                &self
                    .global_variables
                    .as_ref()
                    .map(std::collections::HashMap::len),
            )
            .field("has_physical_connection", &self.has_physical_connection())
            .field(
                "statement_pool_size",
                &self.statement_pool.as_ref().map(|pool| {
                    pool.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .size()
                }),
            )
            .finish()
    }
}

impl DruidConnectionHolder {
    /// 创建不带物理连接的兼容状态持有者。
    ///
    /// 对应原有 Rust `ConnectionHolder::new(id)`；native pool 应使用
    /// [`Self::with_connection`]，避免构造不完整的实际池条目。
    ///
    /// # 参数
    /// - `id`：连接 ID。
    pub fn new(id: u64) -> Self {
        Self::new_internal(
            None,
            id,
            Duration::ZERO,
            0,
            None,
            None,
            ConnectionDefaults::default(),
        )
    }

    /// 从首次进入池的物理连接创建 holder。
    ///
    /// 对应 Java：`DruidConnectionHolder(DruidAbstractDataSource, Connection, long)`。
    ///
    /// # 参数
    /// - `physical_connection`：holder 独占的底层连接。
    /// - `id`：连接 ID。
    /// - `create_duration`：创建物理连接所耗时间。
    /// - `user_password_version`：创建时的数据源凭据版本。
    pub fn with_connection(
        physical_connection: Box<dyn PhysicalConnection>,
        id: u64,
        create_duration: Duration,
        user_password_version: u64,
    ) -> Self {
        let defaults = ConnectionDefaults::capture(physical_connection.as_ref());
        Self::with_connection_and_defaults(
            physical_connection,
            id,
            create_duration,
            user_password_version,
            defaults,
        )
    }

    /// 使用首次入池时已经捕获的默认属性创建 holder。
    ///
    /// 多次借用同一连接时必须保留原始 `defaults`，不能把上一个借用者留下的
    /// 状态重新解释为数据源默认值。
    ///
    /// # 参数
    /// - `physical_connection`：holder 独占的底层连接。
    /// - `id`：连接 ID。
    /// - `create_duration`：创建物理连接所耗时间。
    /// - `user_password_version`：创建时的数据源凭据版本。
    /// - `defaults`：首次进入池时捕获的连接属性。
    pub fn with_connection_and_defaults(
        physical_connection: Box<dyn PhysicalConnection>,
        id: u64,
        create_duration: Duration,
        user_password_version: u64,
        defaults: ConnectionDefaults,
    ) -> Self {
        Self::new_internal(
            Some(physical_connection),
            id,
            create_duration,
            user_password_version,
            None,
            None,
            defaults,
        )
    }

    /// 从完整物理连接创建结果构造 holder。
    ///
    /// 对应 Java：
    /// `DruidConnectionHolder(DruidAbstractDataSource, PhysicalConnectionInfo)`。
    /// connect 阶段耗时、会话变量和全局变量随物理连接一起移动，避免拆成
    /// 与连接生命周期不同步的旁路状态。
    pub fn with_connection_info(
        mut connection_info: PhysicalConnectionInfo,
        id: u64,
        user_password_version: u64,
    ) -> Result<Self, DruidError> {
        let create_duration = connection_info.connect_span();
        let variables = connection_info.take_variables();
        let global_variables = connection_info.take_global_variables();
        let physical_connection = connection_info
            .take_physical_connection()
            .ok_or(DruidError::ConnectionDiscarded)?;
        let defaults = ConnectionDefaults::capture(physical_connection.as_ref());
        Ok(Self::new_internal(
            Some(physical_connection),
            id,
            create_duration,
            user_password_version,
            variables,
            global_variables,
            defaults,
        ))
    }

    fn new_internal(
        physical_connection: Option<Box<dyn PhysicalConnection>>,
        id: u64,
        create_duration: Duration,
        user_password_version: u64,
        variables: Option<HashMap<String, Value>>,
        global_variables: Option<HashMap<String, Value>>,
        defaults: ConnectionDefaults,
    ) -> Self {
        let now = Instant::now();
        Self {
            physical_connection,
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
            statement_pool: None,
            pool_prepared_statements: false,
            max_pool_prepared_statements_per_connection: 10,
            share_prepared_statements: false,
            use_oracle_implicit_cache: false,
            prepared_statement_stats: Arc::new(PreparedStatementCacheStats::default()),
            connection_event_listeners: RwLock::new(Vec::new()),
            statement_event_listeners: RwLock::new(Vec::new()),
            statement_trace: Mutex::new(Vec::new()),
            last_fingerprint: Mutex::new(None),
        }
    }

    /// 返回物理连接 ID。
    pub fn connection_id(&self) -> u64 {
        self.id
    }

    /// 返回 holder 是否仍拥有物理连接。
    pub fn has_physical_connection(&self) -> bool {
        self.physical_connection.is_some()
    }

    /// 返回初始化阶段采集的会话变量。
    #[must_use]
    pub fn variables(&self) -> Option<&HashMap<String, Value>> {
        self.variables.as_ref()
    }

    /// 返回初始化阶段采集的全局变量。
    #[must_use]
    pub fn global_variables(&self) -> Option<&HashMap<String, Value>> {
        self.global_variables.as_ref()
    }

    /// 保存物理 connect Filter 最终使用的 Properties 快照。
    pub(crate) fn set_connection_properties(
        &mut self,
        connection_properties: Arc<HashMap<String, String>>,
    ) {
        self.connection_properties = connection_properties;
    }

    /// 返回物理 connect Filter 最终使用的 Properties 快照。
    #[must_use]
    pub(crate) fn connection_properties(&self) -> Arc<HashMap<String, String>> {
        Arc::clone(&self.connection_properties)
    }

    /// 添加 `javax.sql.ConnectionEventListener` 对应监听器。
    pub fn add_connection_event_listener(&self, listener: Arc<dyn ConnectionEventListener>) {
        self.connection_event_listeners.write().push(listener);
    }

    /// 按对象身份移除连接监听器。
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

    /// 返回连接监听器快照，避免回调期间持有锁。
    pub fn connection_event_listeners(&self) -> Vec<Arc<dyn ConnectionEventListener>> {
        self.connection_event_listeners.read().clone()
    }

    /// 回收 holder 时清空本次逻辑租约的监听器。
    pub fn clear_connection_event_listeners(&self) {
        self.connection_event_listeners.write().clear();
    }

    /// 添加 `javax.sql.StatementEventListener` 对应监听器。
    pub fn add_statement_event_listener(&self, listener: Arc<dyn StatementEventListener>) {
        self.statement_event_listeners.write().push(listener);
    }

    /// 按对象身份移除 Statement 监听器的首个匹配项。
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

    /// 返回 Statement 监听器快照。
    pub fn statement_event_listeners(&self) -> Vec<Arc<dyn StatementEventListener>> {
        self.statement_event_listeners.read().clone()
    }

    /// holder reset 时清空 Statement 监听器。
    pub fn clear_statement_event_listeners(&self) {
        self.statement_event_listeners.write().clear();
    }

    /// 追踪一个尚未关闭的逻辑 Statement。
    ///
    /// 对应 Java：`DruidConnectionHolder#addTrace(DruidPooledStatement)`。
    pub(crate) fn add_statement_trace(&self, statement: StatementTrace) {
        self.statement_trace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(statement);
    }

    /// 按逻辑对象身份移除 Statement trace。
    ///
    /// 对应 Java：`DruidConnectionHolder#removeTrace(DruidPooledStatement)`。
    pub(crate) fn remove_statement_trace(&self, identity: usize) -> bool {
        let mut statements = self
            .statement_trace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(index) = statements
            .iter()
            .position(|statement| statement.identity() == identity)
        else {
            return false;
        };
        statements.remove(index);
        true
    }

    /// 关闭并清空本次逻辑连接遗留的所有 Statement。
    ///
    /// 对应 Java：`DruidConnectionHolder#reset()` 对 `statementTrace` 的处理。
    /// 先从锁内取走快照，再逐项关闭，避免 PreparedStatement 的缓存归还路径
    /// 在关闭过程中重入 holder 时形成锁递归。
    pub(crate) fn close_statement_trace(&self) {
        let statements = {
            let mut statements = self
                .statement_trace
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *statements)
        };
        for statement in statements {
            statement.close();
        }
    }

    /// 返回物理连接只读引用。
    pub fn physical_connection(&self) -> Option<&(dyn PhysicalConnection + 'static)> {
        self.physical_connection.as_deref()
    }

    /// 返回物理连接可变引用。
    pub fn physical_connection_mut(&mut self) -> Option<&mut (dyn PhysicalConnection + 'static)> {
        self.physical_connection.as_deref_mut()
    }

    /// 返回物理连接盒装对象的可变引用。
    ///
    /// 仅供 native pool 调用 `PhysicalConnectionFactory#validate` 使用。
    pub fn physical_connection_box_mut(&mut self) -> Option<&mut Box<dyn PhysicalConnection>> {
        self.physical_connection.as_mut()
    }

    /// 取出物理连接所有权。
    ///
    /// 返回后 holder 不再包含连接，只用于兼容外部池回调或最终销毁。
    pub fn take_physical_connection(&mut self) -> Option<Box<dyn PhysicalConnection>> {
        self.close_statement_trace();
        self.clear_statement_cache();
        self.physical_connection.take()
    }

    /// 返回首次进入池时捕获的连接默认属性。
    pub fn defaults(&self) -> &ConnectionDefaults {
        &self.defaults
    }

    /// 配置单连接 PreparedStatement 缓存。
    ///
    /// 对应 Java：`poolPreparedStatements`、
    /// `maxPoolPreparedStatementPerConnectionSize`、`sharePreparedStatements` 和
    /// `useOracleImplicitCache`。配置只影响随后创建/访问的 statement pool；
    /// 已经存在的 pool 会先清空再按新配置惰性重建。
    pub fn configure_statement_pool(
        &mut self,
        pool_prepared_statements: bool,
        max_pool_prepared_statements_per_connection: usize,
        share_prepared_statements: bool,
        use_oracle_implicit_cache: bool,
        stats: Arc<PreparedStatementCacheStats>,
    ) {
        self.clear_statement_cache();
        self.statement_pool = None;
        self.pool_prepared_statements = pool_prepared_statements;
        self.max_pool_prepared_statements_per_connection =
            max_pool_prepared_statements_per_connection;
        self.share_prepared_statements = share_prepared_statements;
        self.use_oracle_implicit_cache = use_oracle_implicit_cache;
        self.prepared_statement_stats = stats;
    }

    /// 返回是否启用单连接 PreparedStatement 缓存。
    pub fn is_pool_prepared_statements(&self) -> bool {
        self.pool_prepared_statements
    }

    /// 惰性创建并返回 PreparedStatement pool。
    ///
    /// 对应 Java：`DruidConnectionHolder#getStatementPool()`。
    pub fn statement_pool(&mut self) -> Arc<Mutex<PreparedStatementPool>> {
        if self.statement_pool.is_none() {
            self.statement_pool = Some(Arc::new(Mutex::new(PreparedStatementPool::new(
                self.max_pool_prepared_statements_per_connection,
                self.share_prepared_statements,
                self.use_oracle_implicit_cache,
                self.prepared_statement_stats.clone(),
            ))));
        }
        self.statement_pool
            .as_ref()
            .expect("statement pool initialized above")
            .clone()
    }

    /// 返回已经创建的 PreparedStatement pool；不会触发惰性初始化。
    ///
    /// 对应 Java：`DruidConnectionHolder#getStatementPoolDirect()`。
    pub fn statement_pool_direct(&self) -> Option<Arc<Mutex<PreparedStatementPool>>> {
        self.statement_pool.clone()
    }

    /// 清空当前连接的 PreparedStatement cache；从未创建 cache 时不做任何事。
    ///
    /// 对应 Java：`DruidConnectionHolder#clearStatementCache()`。
    pub fn clear_statement_cache(&mut self) {
        if let Some(statement_pool) = self.statement_pool.as_ref() {
            statement_pool
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
        }
    }

    /// 返回已经创建的 cache 中是否存在尚未关闭的逻辑语句。
    pub fn has_in_use_prepared_statement(&self) -> bool {
        self.statement_pool.as_ref().is_some_and(|statement_pool| {
            statement_pool
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .has_in_use_statement()
        })
    }

    /// 从缓存取得 PreparedStatement holder。
    pub fn get_cached_prepared_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Option<Arc<PreparedStatementHolder>> {
        self.statement_pool()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
    }

    /// 将 PreparedStatement holder 放回缓存。
    pub fn cache_prepared_statement(&mut self, holder: Arc<PreparedStatementHolder>) {
        self.statement_pool()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .put(holder);
    }

    /// 从缓存删除 PreparedStatement holder。
    pub fn remove_cached_prepared_statement(&mut self, holder: &Arc<PreparedStatementHolder>) {
        self.statement_pool()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(holder);
    }

    /// 返回数据源级 PreparedStatement 统计。
    pub fn prepared_statement_stats(&self) -> &Arc<PreparedStatementCacheStats> {
        &self.prepared_statement_stats
    }

    /// 按 Java `DruidConnectionHolder#reset()` 顺序复位底层连接。
    ///
    /// # 参数
    /// - `keep_underlying_transaction_isolation`：是否保留借用期间设置的隔离级别。
    ///
    /// # 错误
    /// holder 已失去连接或任一驱动复位操作失败时返回错误。
    pub async fn reset(
        &mut self,
        keep_underlying_transaction_isolation: bool,
    ) -> Result<(), DruidError> {
        let defaults = self.defaults.clone();
        {
            let connection = self
                .physical_connection_mut()
                .ok_or(DruidError::ConnectionDiscarded)?;
            defaults
                .reset_state(connection, keep_underlying_transaction_isolation)
                .await?;
        }

        self.clear_connection_event_listeners();
        self.clear_statement_event_listeners();
        self.close_statement_trace();

        let connection = self
            .physical_connection_mut()
            .ok_or(DruidError::ConnectionDiscarded)?;
        if connection.capabilities().clear_warnings {
            connection.clear_warnings().await?;
        }
        Ok(())
    }

    /// 返回当前生命周期状态。
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

    /// 原子地执行状态转换。
    ///
    /// # 参数
    /// - `from`：期望的当前状态。
    /// - `to`：目标状态。
    pub fn try_transition(&self, from: ConnectionState, to: ConnectionState) -> bool {
        self.state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// 将空闲连接标记为借出，并增加 `useCount`。
    ///
    /// 仅 `Idle -> Active` 成功时增加计数。
    pub fn mark_active(&self) -> bool {
        if self.try_transition(ConnectionState::Idle, ConnectionState::Active) {
            self.use_count.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// 将借出连接标记为空闲并刷新 `lastActiveTimeMillis`。
    pub fn mark_idle(&self) -> bool {
        *self
            .last_active_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Instant::now();
        self.try_transition(ConnectionState::Active, ConnectionState::Idle)
    }

    /// 返回当前借用次数。
    pub fn use_count(&self) -> u64 {
        self.use_count.load(Ordering::Relaxed)
    }

    /// 返回连接创建后经过的时间。
    pub fn physical_age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// 返回最近一次归还/活跃时刻至今的时长。
    pub fn idle_duration(&self) -> Duration {
        self.last_active_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .elapsed()
    }

    /// 返回最近一次 SQL 或事务状态操作至今的时长。
    pub fn last_exec_idle_duration(&self) -> Duration {
        self.last_exec_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .elapsed()
    }

    /// 记录一次 SQL 或事务状态操作。
    pub fn record_execute(&self) {
        *self
            .last_exec_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Instant::now();
    }

    /// 记录最近一次连接有效性检查成功时刻。
    pub fn record_valid(&self) {
        *self
            .last_valid_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
    }

    /// 记录最近一次保活检查成功时刻。
    pub fn record_keep_alive(&self) {
        *self
            .last_keep_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
    }

    /// 返回最近一次保活检查成功后经过的时间；从未成功时返回 `None`。
    pub fn last_keep_elapsed(&self) -> Option<Duration> {
        self.last_keep_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map(|instant| instant.elapsed())
    }

    /// 返回最近一次有效性检查成功后经过的时间；从未成功时返回 `None`。
    pub fn last_valid_elapsed(&self) -> Option<Duration> {
        self.last_valid_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map(|instant| instant.elapsed())
    }

    /// 增加 holder 保活检查次数。
    pub fn increment_keep_alive_check_count(&self) {
        self.keep_alive_check_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 返回 holder 保活检查次数。
    pub fn keep_alive_check_count(&self) -> u64 {
        self.keep_alive_check_count.load(Ordering::Relaxed)
    }

    /// 保存最近一次等待非空队列的耗时。
    pub fn set_last_not_empty_wait(&self, duration: Duration) {
        *self
            .last_not_empty_wait
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = duration;
    }

    /// 返回最近一次等待非空队列的耗时。
    pub fn last_not_empty_wait(&self) -> Duration {
        *self
            .last_not_empty_wait
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 返回创建物理连接所耗时间。
    pub fn create_duration(&self) -> Duration {
        self.create_duration
    }

    /// 标记连接必须被丢弃，并同步通知物理 Adapter。
    pub fn mark_discarded(&mut self) {
        self.discard.store(true, Ordering::Release);
        if let Some(connection) = self.physical_connection.as_mut() {
            connection.mark_discarded();
        }
    }

    /// 返回 holder 或物理 Adapter 是否已经标记丢弃。
    pub fn is_discard(&self) -> bool {
        self.discard.load(Ordering::Acquire)
            || self
                .physical_connection
                .as_ref()
                .is_none_or(|connection| connection.is_discarded())
    }

    /// 保存首次修改 schema 前的原始值。
    ///
    /// 对应 Java MySQL 分支中的 `holder.initSchema`；只记录第一次值。
    pub fn remember_initial_schema(&self, schema: Option<String>) {
        let mut initial = self
            .init_schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if initial.is_none() {
            *initial = schema;
        }
    }

    /// 设置是否采用 MySQL-family 的 schema 回收语义。
    ///
    /// 对应 Java：`JdbcUtils.isMysqlDbType(...)` 分支。
    pub fn set_restore_schema_on_recycle(&self, restore_schema_on_recycle: bool) {
        self.restore_schema_on_recycle
            .store(restore_schema_on_recycle, Ordering::Release);
    }

    /// 返回回收时是否需要恢复首次记录的 schema。
    pub fn should_restore_schema_on_recycle(&self) -> bool {
        self.restore_schema_on_recycle.load(Ordering::Acquire)
    }

    /// 在回收时恢复首次记录的 schema。
    ///
    /// 与 Java 一致，只有设置成功后才清空 `initSchema`。
    ///
    /// # 错误
    /// holder 已失去连接或驱动拒绝设置 schema 时返回错误。
    pub async fn restore_initial_schema(&mut self) -> Result<(), DruidError> {
        let schema = self
            .init_schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(schema) = schema else {
            return Ok(());
        };

        self.physical_connection_mut()
            .ok_or(DruidError::ConnectionDiscarded)?
            .set_schema(&schema)
            .await?;
        *self
            .init_schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }

    /// 返回创建连接时的数据源用户名/密码版本。
    pub fn user_password_version(&self) -> u64 {
        self.user_password_version
    }

    /// 判断 holder 在指定空闲阈值内是否仍可使用。
    ///
    /// `Closed` 与 `Error` 状态始终返回 `false`。
    pub fn is_alive(&self, idle_timeout: Duration) -> bool {
        let state = self.state();
        state != ConnectionState::Closed
            && state != ConnectionState::Error
            && self.idle_duration() < idle_timeout
    }

    /// 返回最近一次活跃后经过的时间。
    ///
    /// 保留原有 Rust `ConnectionHolder#held_duration` 兼容语义。
    pub fn held_duration(&self) -> Duration {
        self.idle_duration()
    }
}
