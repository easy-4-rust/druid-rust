//! 对外池化预编译语句。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidPooledPreparedStatement`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/DruidPooledPreparedStatement.java`。

use super::druid_pooled_statement::DruidPooledStatementInner;
use super::prepared_statement_physical_statement::PreparedStatementPhysicalStatement;
use super::{
    DruidError, DruidPooledConnection, DruidPooledResultSet, DruidPooledStatement, ExecResult,
    FilterChain, JdbcArray, JdbcBlob, JdbcCalendar, JdbcCalendarArgument, JdbcCharacterLength,
    JdbcClob, JdbcInputStream, JdbcNClob, JdbcObject, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml,
    JdbcStreamLength, JdbcUrl, PhysicalResultSet, PhysicalStatement, PhysicalStatementOptions,
    PreparedInputParameter, PreparedStatementCacheStats, PreparedStatementHolder,
    PreparedStatementKey, PreparedStatementPool, Row, SqlWarning, Unwrapped, Value, Wrapper,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct DruidPooledPreparedStatementState {
    parameters: Vec<Option<PreparedInputParameter>>,
    batch_parameter_sets: Vec<Vec<PreparedInputParameter>>,
    exception_count: u64,
    fetch_row_peak: i32,
    default_max_field_size: i32,
    default_max_rows: i32,
    default_query_timeout: i32,
    default_fetch_direction: i32,
    default_fetch_size: i32,
    current_max_field_size: i32,
    current_max_rows: i32,
    current_query_timeout: i32,
    current_fetch_direction: i32,
    current_fetch_size: i32,
    closed: bool,
}

macro_rules! prepared_value_setter {
    ($method:ident, $value_type:ty, $variant:ident, $java_method:literal) => {
        #[doc = concat!("执行 Java `PreparedStatement#", $java_method, "(int, ..)`。")]
        ///
        /// 参数在 setter 调用时立即进入物理 PreparedStatement；成功后才更新
        /// Rust 绑定快照，错误由同一连接的 ExceptionSorter 分类。
        pub fn $method(
            &mut self,
            connection: &mut DruidPooledConnection,
            parameter_index: usize,
            value: $value_type,
        ) -> Result<(), DruidError> {
            self.set_parameter(
                connection,
                parameter_index,
                PreparedInputParameter::$variant(value),
            )
        }
    };
}

struct DruidPooledPreparedStatementShared {
    holder: Arc<PreparedStatementHolder>,
    pooled: bool,
    statement_pool: Option<Arc<Mutex<PreparedStatementPool>>>,
    stats: Arc<PreparedStatementCacheStats>,
    lease_active: Arc<AtomicBool>,
    statement_inner: Arc<DruidPooledStatementInner>,
    state: Mutex<DruidPooledPreparedStatementState>,
}

impl DruidPooledPreparedStatementShared {
    fn is_closed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
    }

    fn increment_exception_count(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.exception_count = state.exception_count.saturating_add(1);
    }

    fn restore_statement_defaults(&self) -> Result<(), DruidError> {
        if !self.pooled || !self.lease_active.load(Ordering::Acquire) {
            return Ok(());
        }

        let statement = self.holder.statement();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // 严格保持 Java close() 的恢复顺序；任一 setter 失败即停止，后续属性
        // 保持当前值，使显式调用者能够观察并重试关闭。
        if state.default_max_field_size != state.current_max_field_size {
            statement.set_max_field_size(state.default_max_field_size)?;
            state.current_max_field_size = state.default_max_field_size;
        }
        if state.default_max_rows != state.current_max_rows {
            statement.set_max_rows(state.default_max_rows)?;
            state.current_max_rows = state.default_max_rows;
        }
        if state.default_query_timeout != state.current_query_timeout {
            statement.set_query_timeout(state.default_query_timeout)?;
            state.current_query_timeout = state.default_query_timeout;
        }
        if state.default_fetch_direction != state.current_fetch_direction {
            statement.set_fetch_direction(state.default_fetch_direction)?;
            state.current_fetch_direction = state.default_fetch_direction;
        }
        if state.default_fetch_size != state.current_fetch_size {
            statement.set_fetch_size(state.default_fetch_size)?;
            state.current_fetch_size = state.default_fetch_size;
        }
        Ok(())
    }

    fn finish(&self) {
        if self.restore_statement_defaults().is_err() {
            // Drop 没有可用连接上下文，不能运行 ExceptionSorter；但脏 statement
            // 绝不能重新进入缓存，因此记为异常并走删除分支。
            self.increment_exception_count();
        }
        let (exception_count, fetch_row_peak) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.closed {
                return;
            }
            state.closed = true;
            (state.exception_count, state.fetch_row_peak)
        };

        let statement_base = DruidPooledStatement::from_inner(Arc::clone(&self.statement_inner));
        statement_base.close_embedded_base();
        self.holder.decrement_in_use_count();
        if !self.lease_active.load(Ordering::Acquire) {
            // 连接已经归还：禁止旧 wrapper 重新进入下一次租约的 cache。
            if !self.holder.statement().is_closed() {
                let _ = self.holder.statement().close();
                if self.holder.hit_count() > 0 {
                    self.stats.record_cache_delete();
                } else {
                    self.stats.record_close();
                }
            }
            return;
        }

        let has_exception = exception_count > 0 || statement_base.exception_count() > 0;
        if self.pooled && !has_exception {
            self.holder
                .set_fetch_row_peak(fetch_row_peak.max(statement_base.fetch_row_peak()));
            if let Some(statement_pool) = &self.statement_pool {
                statement_pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .put(Arc::clone(&self.holder));
            } else {
                let _ = self.holder.statement().close();
                self.stats.record_close();
            }
        } else if self.pooled {
            if let Some(statement_pool) = &self.statement_pool {
                statement_pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&self.holder);
            } else {
                let _ = self.holder.statement().close();
                self.stats.record_close();
            }
        } else {
            let _ = self.holder.statement().close();
            self.stats.record_close();
        }
    }
}

impl Drop for DruidPooledPreparedStatementShared {
    fn drop(&mut self) {
        self.finish();
    }
}

/// 借用池化连接执行并在关闭时复用物理 PreparedStatement 的逻辑语句。
///
/// 语句句柄不独占连接借用，因此同一连接可同时持有多个 PreparedStatement，
/// 保留 Java `inUseCount`、`sharePreparedStatements` 和 LRU 替换语义。执行时
/// 显式传入原 `DruidPooledConnection`，关闭/Drop 则通过共享 statement pool
/// 归还物理语句。
pub struct DruidPooledPreparedStatement {
    shared: Arc<DruidPooledPreparedStatementShared>,
    statement_base: DruidPooledStatement,
}

/// `ResultSet#getStatement()` 返回的 PreparedStatement 共享身份句柄。
///
/// 对应 Java：`DruidPooledResultSet#getStatement()` 返回原
/// `DruidPooledPreparedStatement` 实例。Rust 句柄与原对象共享 holder、
/// Statement 状态、关闭状态和缓存归还所有权；即使原局部变量先离开作用域，
/// ResultSet 持有的句柄也会阻止物理语句被提前回收。
#[derive(Clone)]
pub struct DruidPooledPreparedStatementHandle {
    shared: Arc<DruidPooledPreparedStatementShared>,
    statement_base: DruidPooledStatement,
}

impl DruidPooledPreparedStatementHandle {
    pub(crate) fn physical_statement(&self) -> &dyn super::PhysicalPreparedStatement {
        self.shared.holder.statement().as_ref()
    }

    /// 返回原 PreparedStatement 的完整缓存键。
    pub fn key(&self) -> &PreparedStatementKey {
        self.shared.holder.key()
    }

    /// 返回继承的池化 Statement 视图。
    pub fn pooled_statement(&self) -> &DruidPooledStatement {
        &self.statement_base
    }

    /// 返回原逻辑 PreparedStatement 是否已关闭。
    pub fn is_closed(&self) -> bool {
        self.shared.is_closed()
    }

    /// 判断句柄是否与给定 PreparedStatement 表示同一逻辑 Java 对象。
    pub fn is_same_statement(&self, statement: &DruidPooledPreparedStatement) -> bool {
        Arc::ptr_eq(&self.shared, &statement.shared)
    }

    /// 通过 ResultSet 返回的句柄关闭原逻辑 PreparedStatement。
    pub fn close(&self) -> Result<(), DruidError> {
        if self.shared.is_closed() {
            return Ok(());
        }
        if let Err(error) = self.shared.restore_statement_defaults() {
            self.shared.increment_exception_count();
            return Err(error);
        }
        if self.shared.pooled {
            if let Err(error) = self.shared.holder.statement().clear_parameters() {
                self.shared.increment_exception_count();
                return Err(error);
            }
            if let Err(error) = self.shared.holder.statement().clear_batch() {
                self.shared.increment_exception_count();
                return Err(error);
            }
        }
        self.shared.finish();
        Ok(())
    }
}

impl Wrapper for DruidPooledPreparedStatementHandle {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        let Some(iface) = iface else {
            return false;
        };
        iface == TypeId::of::<Self>()
            || iface == TypeId::of::<dyn super::PhysicalPreparedStatement>()
            || self.shared.holder.statement().as_any().type_id() == iface
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if iface == TypeId::of::<Self>() {
            return Some(Unwrapped::Object(self));
        }
        if iface == TypeId::of::<dyn super::PhysicalPreparedStatement>() {
            return Some(Unwrapped::PreparedStatement(
                self.shared.holder.statement().as_ref(),
            ));
        }
        (self.shared.holder.statement().as_any().type_id() == iface)
            .then(|| Unwrapped::Object(self.shared.holder.statement().as_any()))
    }
}

impl Wrapper for DruidPooledPreparedStatement {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        let Some(iface) = iface else {
            return false;
        };
        iface == TypeId::of::<Self>()
            || iface == TypeId::of::<dyn super::PhysicalPreparedStatement>()
            || self.shared.holder.statement().as_any().type_id() == iface
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if iface == TypeId::of::<Self>() {
            return Some(Unwrapped::Object(self));
        }
        if iface == TypeId::of::<dyn super::PhysicalPreparedStatement>() {
            return Some(Unwrapped::PreparedStatement(
                self.shared.holder.statement().as_ref(),
            ));
        }
        (self.shared.holder.statement().as_any().type_id() == iface)
            .then(|| Unwrapped::Object(self.shared.holder.statement().as_any()))
    }
}

impl std::fmt::Debug for DruidPooledPreparedStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        formatter
            .debug_struct("DruidPooledPreparedStatement")
            .field("key", self.shared.holder.key())
            .field("pooled", &self.shared.pooled)
            .field("exception_count", &state.exception_count)
            .field("statement_base", &self.statement_base)
            .field("closed", &state.closed)
            .finish()
    }
}

impl DruidPooledPreparedStatement {
    pub(crate) fn new(
        holder: Arc<PreparedStatementHolder>,
        pooled: bool,
        statement_pool: Option<Arc<Mutex<PreparedStatementPool>>>,
        stats: Arc<PreparedStatementCacheStats>,
        lease_active: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Self {
        let defaults = PhysicalStatementOptions::default();
        let key = holder.key();
        let statement_options = PhysicalStatementOptions {
            result_set_type: match key.result_set_type() {
                0 => defaults.result_set_type,
                value => value,
            },
            result_set_concurrency: match key.result_set_concurrency() {
                0 => defaults.result_set_concurrency,
                value => value,
            },
            result_set_holdability: match key.result_set_holdability() {
                0 => defaults.result_set_holdability,
                value => value,
            },
        };
        let physical_statement = Arc::clone(holder.statement());
        let (
            default_max_field_size,
            default_max_rows,
            default_query_timeout,
            default_fetch_direction,
            default_fetch_size,
        ) = if pooled {
            (
                physical_statement.max_field_size().unwrap_or_default(),
                physical_statement.max_rows().unwrap_or_default(),
                physical_statement.query_timeout().unwrap_or_default(),
                physical_statement.fetch_direction().unwrap_or_default(),
                physical_statement.fetch_size().unwrap_or_default(),
            )
        } else {
            (0, 0, 0, 0, 0)
        };
        let base_physical: Arc<dyn PhysicalStatement> = Arc::new(
            PreparedStatementPhysicalStatement::new(physical_statement, statement_options),
        );
        let statement_base =
            DruidPooledStatement::new(base_physical, lease_active.clone(), filter_chain);
        let shared = Arc::new(DruidPooledPreparedStatementShared {
            holder,
            pooled,
            statement_pool,
            stats,
            lease_active,
            statement_inner: Arc::clone(&statement_base.inner),
            state: Mutex::new(DruidPooledPreparedStatementState {
                parameters: Vec::new(),
                batch_parameter_sets: Vec::new(),
                exception_count: 0,
                fetch_row_peak: -1,
                default_max_field_size,
                default_max_rows,
                default_query_timeout,
                default_fetch_direction,
                default_fetch_size,
                current_max_field_size: default_max_field_size,
                current_max_rows: default_max_rows,
                current_query_timeout: default_query_timeout,
                current_fetch_direction: default_fetch_direction,
                current_fetch_size: default_fetch_size,
                closed: false,
            }),
        });
        Self {
            shared,
            statement_base,
        }
    }

    /// 返回完整 `PreparedStatement` 缓存键。
    pub fn key(&self) -> &PreparedStatementKey {
        self.shared.holder.key()
    }

    /// 返回内部 statement holder。
    ///
    /// 对应 Java：`getPreparedStatementHolder()`。
    pub fn prepared_statement_holder(&self) -> &PreparedStatementHolder {
        &self.shared.holder
    }

    /// 返回逻辑语句是否关闭。
    pub fn is_closed(&self) -> bool {
        self.shared.is_closed()
    }

    /// 返回 Java 继承语义对应的池化 `Statement` 基类视图。
    ///
    /// `DruidPooledResultSet#getStatement()` 对 `PreparedStatement` 结果返回该基类
    /// 身份；结果状态、trace 与关闭级联都由同一对象承载。
    pub fn pooled_statement(&self) -> &DruidPooledStatement {
        &self.statement_base
    }

    /// 返回 ResultSet 类型。对应 Java：`Statement#getResultSetType()`。
    pub fn result_set_type(&self, connection: &DruidPooledConnection) -> Result<i32, DruidError> {
        self.statement_base.result_set_type(connection)
    }

    /// 返回 ResultSet 并发模式。对应 Java：`Statement#getResultSetConcurrency()`。
    pub fn result_set_concurrency(
        &self,
        connection: &DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.statement_base.result_set_concurrency(connection)
    }

    /// 返回 ResultSet 保持性。对应 Java：`Statement#getResultSetHoldability()`。
    pub fn result_set_holdability(
        &self,
        connection: &DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.statement_base.result_set_holdability(connection)
    }

    /// 返回最大字段大小。对应 Java：`Statement#getMaxFieldSize()`。
    pub fn max_field_size(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.statement_base.max_field_size(connection)
    }

    /// 设置最大字段大小，并保存关闭时需要恢复的当前值。
    pub fn set_max_field_size(
        &mut self,
        connection: &mut DruidPooledConnection,
        max: i32,
    ) -> Result<(), DruidError> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_max_field_size = max;
        self.statement_base.set_max_field_size(connection, max)
    }

    /// 返回最大结果行数。对应 Java：`Statement#getMaxRows()`。
    pub fn max_rows(&mut self, connection: &mut DruidPooledConnection) -> Result<i32, DruidError> {
        self.statement_base.max_rows(connection)
    }

    /// 设置最大结果行数，并保存关闭时需要恢复的当前值。
    pub fn set_max_rows(
        &mut self,
        connection: &mut DruidPooledConnection,
        max: i32,
    ) -> Result<(), DruidError> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_max_rows = max;
        self.statement_base.set_max_rows(connection, max)
    }

    /// 设置 JDBC escape 处理开关。
    pub fn set_escape_processing(
        &mut self,
        connection: &mut DruidPooledConnection,
        enabled: bool,
    ) -> Result<(), DruidError> {
        self.statement_base
            .set_escape_processing(connection, enabled)
    }

    /// 返回查询超时秒数。对应 Java：`Statement#getQueryTimeout()`。
    pub fn query_timeout(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.statement_base.query_timeout(connection)
    }

    /// 设置查询超时，并保存关闭时需要恢复的当前值。
    pub fn set_query_timeout(
        &mut self,
        connection: &mut DruidPooledConnection,
        seconds: i32,
    ) -> Result<(), DruidError> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_query_timeout = seconds;
        self.statement_base.set_query_timeout(connection, seconds)
    }

    /// 取消当前执行。对应 Java：`Statement#cancel()`。
    pub fn cancel(&mut self, connection: &mut DruidPooledConnection) -> Result<(), DruidError> {
        self.statement_base.cancel(connection)
    }

    /// 设置游标名称。对应 Java：`Statement#setCursorName(String)`。
    pub fn set_cursor_name(
        &mut self,
        connection: &mut DruidPooledConnection,
        name: &str,
    ) -> Result<(), DruidError> {
        self.statement_base.set_cursor_name(connection, name)
    }

    /// 设置抓取方向，并保存关闭时需要恢复的当前值。
    pub fn set_fetch_direction(
        &mut self,
        connection: &mut DruidPooledConnection,
        direction: i32,
    ) -> Result<(), DruidError> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_fetch_direction = direction;
        self.statement_base
            .set_fetch_direction(connection, direction)
    }

    /// 返回抓取方向。对应 Java：`Statement#getFetchDirection()`。
    pub fn fetch_direction(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.statement_base.fetch_direction(connection)
    }

    /// 设置抓取行数，并保存关闭时需要恢复的当前值。
    pub fn set_fetch_size(
        &mut self,
        connection: &mut DruidPooledConnection,
        rows: i32,
    ) -> Result<(), DruidError> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_fetch_size = rows;
        self.statement_base.set_fetch_size(connection, rows)
    }

    /// 返回抓取行数。对应 Java：`Statement#getFetchSize()`。
    pub fn fetch_size(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.statement_base.fetch_size(connection)
    }

    /// 设置 poolable；Java Druid 仅接受 `true`，且始终报告 `false`。
    pub fn set_poolable(
        &mut self,
        connection: &mut DruidPooledConnection,
        poolable: bool,
    ) -> Result<(), DruidError> {
        self.statement_base.set_poolable(connection, poolable)
    }

    /// Java Druid 的池化 PreparedStatement wrapper 仍返回 `false`。
    pub fn is_poolable(&self) -> bool {
        self.statement_base.is_poolable()
    }

    /// 设置执行完成后自动关闭。
    pub fn close_on_completion(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.statement_base.close_on_completion(connection)
    }

    /// 返回执行完成后自动关闭状态。
    pub fn is_close_on_completion(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.statement_base.is_close_on_completion(connection)
    }

    /// 返回当前已分配的参数槽位数，包括尚未绑定的中间槽位。
    pub fn parameter_slot_count(&self) -> usize {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters
            .len()
    }

    /// 返回指定 Java 1-based 下标的绑定参数。
    pub fn parameter(&self, parameter_index: usize) -> Option<PreparedInputParameter> {
        parameter_index.checked_sub(1).and_then(|index| {
            self.shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .parameters
                .get(index)
                .and_then(Clone::clone)
        })
    }

    /// 执行 `setNull(int, int)`。
    pub fn set_null(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        sql_type: i32,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::null(sql_type),
        )
    }

    /// 执行 `setNull(int, int, String)`；`type_name=None` 对应 Java null。
    pub fn set_null_with_type_name(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        sql_type: i32,
        type_name: Option<String>,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::null_with_type_name(sql_type, type_name),
        )
    }

    prepared_value_setter!(set_boolean, bool, Boolean, "setBoolean");
    prepared_value_setter!(set_byte, i8, Byte, "setByte");
    prepared_value_setter!(set_short, i16, Short, "setShort");
    prepared_value_setter!(set_int, i32, Int, "setInt");
    prepared_value_setter!(set_long, i64, Long, "setLong");
    prepared_value_setter!(set_float, f32, Float, "setFloat");
    prepared_value_setter!(set_double, f64, Double, "setDouble");
    prepared_value_setter!(
        set_big_decimal,
        Option<BigDecimal>,
        BigDecimal,
        "setBigDecimal"
    );
    prepared_value_setter!(set_string, Option<String>, String, "setString");
    prepared_value_setter!(set_n_string, Option<String>, NString, "setNString");
    prepared_value_setter!(set_bytes, Option<Vec<u8>>, Bytes, "setBytes");
    prepared_value_setter!(set_ref, Option<JdbcRef>, Ref, "setRef");
    prepared_value_setter!(set_blob, Option<JdbcBlob>, Blob, "setBlob");
    prepared_value_setter!(set_clob, Option<JdbcClob>, Clob, "setClob");
    prepared_value_setter!(set_n_clob, Option<JdbcNClob>, NClob, "setNClob");
    prepared_value_setter!(set_array, Option<JdbcArray>, Array, "setArray");
    prepared_value_setter!(set_url, Option<JdbcUrl>, Url, "setURL");
    prepared_value_setter!(set_row_id, Option<JdbcRowId>, RowId, "setRowId");
    prepared_value_setter!(set_sql_xml, Option<JdbcSqlXml>, SqlXml, "setSQLXML");

    /// 执行 `setDate(int, Date)`。
    pub fn set_date(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveDate>,
    ) -> Result<(), DruidError> {
        self.set_date_with_calendar_argument(
            connection,
            parameter_index,
            value,
            JdbcCalendarArgument::Unspecified,
        )
    }

    /// 执行 `setDate(int, Date, Calendar)`。
    pub fn set_date_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveDate>,
        calendar: Option<JdbcCalendar>,
    ) -> Result<(), DruidError> {
        self.set_date_with_calendar_argument(
            connection,
            parameter_index,
            value,
            JdbcCalendarArgument::Specified(calendar),
        )
    }

    /// 执行 `setTime(int, Time)`。
    pub fn set_time(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveTime>,
    ) -> Result<(), DruidError> {
        self.set_time_with_calendar_argument(
            connection,
            parameter_index,
            value,
            JdbcCalendarArgument::Unspecified,
        )
    }

    /// 执行 `setTime(int, Time, Calendar)`。
    pub fn set_time_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveTime>,
        calendar: Option<JdbcCalendar>,
    ) -> Result<(), DruidError> {
        self.set_time_with_calendar_argument(
            connection,
            parameter_index,
            value,
            JdbcCalendarArgument::Specified(calendar),
        )
    }

    /// 执行 `setTimestamp(int, Timestamp)`。
    pub fn set_timestamp(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveDateTime>,
    ) -> Result<(), DruidError> {
        self.set_timestamp_with_calendar_argument(
            connection,
            parameter_index,
            value,
            JdbcCalendarArgument::Unspecified,
        )
    }

    /// 执行 `setTimestamp(int, Timestamp, Calendar)`。
    pub fn set_timestamp_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveDateTime>,
        calendar: Option<JdbcCalendar>,
    ) -> Result<(), DruidError> {
        self.set_timestamp_with_calendar_argument(
            connection,
            parameter_index,
            value,
            JdbcCalendarArgument::Specified(calendar),
        )
    }

    /// 执行 `setObject(int, Object)`。
    pub fn set_object(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<JdbcObject>,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::object(value),
        )
    }

    /// 执行 `setObject(int, Object, int)`。
    pub fn set_object_with_sql_type(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<JdbcObject>,
        target_sql_type: i32,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::object_with_sql_type(value, target_sql_type),
        )
    }

    /// 执行 `setObject(int, Object, int, int)`。
    pub fn set_object_with_sql_type_and_scale(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<JdbcObject>,
        target_sql_type: i32,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::object_with_sql_type_and_scale(
                value,
                target_sql_type,
                scale_or_length,
            ),
        )
    }

    /// 执行 `setAsciiStream(int, InputStream)`。
    pub fn set_ascii_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.set_stream_parameter(
            connection,
            parameter_index,
            stream,
            JdbcStreamLength::Unspecified,
            true,
        )
    }

    /// 执行 `setAsciiStream(int, InputStream, int)`。
    pub fn set_ascii_stream_with_int_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.set_stream_parameter(
            connection,
            parameter_index,
            stream,
            JdbcStreamLength::Int(length),
            true,
        )
    }

    /// 执行 `setAsciiStream(int, InputStream, long)`。
    pub fn set_ascii_stream_with_long_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_stream_parameter(
            connection,
            parameter_index,
            stream,
            JdbcStreamLength::Long(length),
            true,
        )
    }

    /// 执行已废弃的 `setUnicodeStream(int, InputStream, int)`。
    pub fn set_unicode_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::UnicodeStream { stream, length },
        )
    }

    /// 执行 `setBinaryStream(int, InputStream)`。
    pub fn set_binary_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.set_stream_parameter(
            connection,
            parameter_index,
            stream,
            JdbcStreamLength::Unspecified,
            false,
        )
    }

    /// 执行 `setBinaryStream(int, InputStream, int)`。
    pub fn set_binary_stream_with_int_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.set_stream_parameter(
            connection,
            parameter_index,
            stream,
            JdbcStreamLength::Int(length),
            false,
        )
    }

    /// 执行 `setBinaryStream(int, InputStream, long)`。
    pub fn set_binary_stream_with_long_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_stream_parameter(
            connection,
            parameter_index,
            stream,
            JdbcStreamLength::Long(length),
            false,
        )
    }

    /// 执行 `setCharacterStream(int, Reader)`。
    pub fn set_character_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
    ) -> Result<(), DruidError> {
        self.set_character_parameter(
            connection,
            parameter_index,
            reader,
            JdbcCharacterLength::Unspecified,
            false,
        )
    }

    /// 执行 `setCharacterStream(int, Reader, int)`。
    pub fn set_character_stream_with_int_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.set_character_parameter(
            connection,
            parameter_index,
            reader,
            JdbcCharacterLength::Int(length),
            false,
        )
    }

    /// 执行 `setCharacterStream(int, Reader, long)`。
    pub fn set_character_stream_with_long_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_character_parameter(
            connection,
            parameter_index,
            reader,
            JdbcCharacterLength::Long(length),
            false,
        )
    }

    /// 执行 `setNCharacterStream(int, Reader)`。
    pub fn set_n_character_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
    ) -> Result<(), DruidError> {
        self.set_character_parameter(
            connection,
            parameter_index,
            reader,
            JdbcCharacterLength::Unspecified,
            true,
        )
    }

    /// 执行 `setNCharacterStream(int, Reader, long)`。
    pub fn set_n_character_stream_with_long_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_character_parameter(
            connection,
            parameter_index,
            reader,
            JdbcCharacterLength::Long(length),
            true,
        )
    }

    /// 执行 `setBlob(int, InputStream)`。
    pub fn set_blob_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::BlobStream {
                stream,
                length: JdbcStreamLength::Unspecified,
            },
        )
    }

    /// 执行 `setBlob(int, InputStream, long)`。
    pub fn set_blob_stream_with_long_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::BlobStream {
                stream,
                length: JdbcStreamLength::Long(length),
            },
        )
    }

    /// 执行 `setClob(int, Reader)`。
    pub fn set_clob_reader(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::ClobReader {
                reader,
                length: JdbcCharacterLength::Unspecified,
            },
        )
    }

    /// 执行 `setClob(int, Reader, long)`。
    pub fn set_clob_reader_with_long_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::ClobReader {
                reader,
                length: JdbcCharacterLength::Long(length),
            },
        )
    }

    /// 执行 `setNClob(int, Reader)`。
    pub fn set_n_clob_reader(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::NClobReader {
                reader,
                length: JdbcCharacterLength::Unspecified,
            },
        )
    }

    /// 执行 `setNClob(int, Reader, long)`。
    pub fn set_n_clob_reader_with_long_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::NClobReader {
                reader,
                length: JdbcCharacterLength::Long(length),
            },
        )
    }

    pub(crate) fn result_set_statement_handle(&self) -> DruidPooledPreparedStatementHandle {
        DruidPooledPreparedStatementHandle {
            shared: Arc::clone(&self.shared),
            statement_base: DruidPooledStatement::from_inner(Arc::clone(
                &self.statement_base.inner,
            )),
        }
    }

    /// 返回 PreparedStatement 警告链。
    ///
    /// 对应 Java：继承的 `DruidPooledStatement#getWarnings()`；使用同一个物理
    /// PreparedStatement handle 和 `statement_getWarnings` Filter around-chain。
    pub async fn warnings(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Option<SqlWarning>, DruidError> {
        self.ensure_open_for(connection)?;
        let filter_chain = self.statement_base.inner.filter_chain.clone();
        let physical = self.shared.holder.statement().clone();
        let result = match filter_chain {
            Some(filter_chain) => {
                filter_chain
                    .prepared_statement_warnings(physical.as_ref())
                    .await
            }
            None => physical.warnings(),
        };
        match connection.classify_result(result) {
            Ok(warning) => Ok(warning),
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    /// 清除 PreparedStatement 警告链。
    ///
    /// 对应 Java：继承的 `DruidPooledStatement#clearWarnings()`。
    pub async fn clear_warnings(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let filter_chain = self.statement_base.inner.filter_chain.clone();
        let physical = self.shared.holder.statement().clone();
        let result = match filter_chain {
            Some(filter_chain) => {
                filter_chain
                    .prepared_statement_clear_warnings(physical.as_ref())
                    .await
            }
            None => physical.clear_warnings(),
        };
        match connection.classify_result(result) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    /// 使用当前 `setXxx` 绑定执行 Java `executeUpdate()` 语义。
    pub async fn execute_update_bound(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<ExecResult, DruidError> {
        let parameters = self.bound_parameters_or_record_error()?;
        self.exec_parameters(connection, parameters).await
    }

    /// 使用当前 `setXxx` 绑定执行 Java `executeQuery()` 语义。
    pub async fn execute_query_bound(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<DruidPooledResultSet, DruidError> {
        let parameters = self.bound_parameters_or_record_error()?;
        self.fetch_parameters_result_set(connection, parameters)
            .await
    }

    /// 使用当前 `setXxx` 绑定执行 Java `execute()` 语义。
    pub async fn execute_bound(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        let parameters = self.bound_parameters_or_record_error()?;
        self.execute_parameters(connection, parameters).await
    }

    /// 使用当前 `setXxx` 绑定执行查询并返回 eager 行集合。
    ///
    /// 这是 Rust 扩展；Java canonical 入口是 `execute_query_bound`。
    pub async fn fetch_bound(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Vec<Row>, DruidError> {
        let parameters = self.bound_parameters_or_record_error()?;
        self.fetch_parameters(connection, parameters).await
    }

    async fn exec_parameters(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<ExecResult, DruidError> {
        self.ensure_open_for(connection)?;
        self.statement_base.begin_external_execution();
        let statement = self.shared.holder.statement().clone();
        let result = connection
            .exec_prepared_parameters_with_filters(statement.as_ref(), parameters)
            .await;
        match &result {
            Ok(execution) => self.statement_base.complete_external_update(execution),
            Err(_) => self.record_exception(),
        }
        result
    }

    async fn fetch_parameters(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Vec<Row>, DruidError> {
        self.ensure_open_for(connection)?;
        self.statement_base.begin_external_execution();
        let statement = self.shared.holder.statement().clone();
        let result = connection
            .fetch_prepared_parameters_with_filters(statement.as_ref(), parameters)
            .await;
        if let Ok(rows) = &result {
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.fetch_row_peak = state
                .fetch_row_peak
                .max(i32::try_from(rows.len()).unwrap_or(i32::MAX));
            drop(state);
            self.statement_base.complete_external_query(rows.clone());
        } else {
            self.record_exception();
        }
        result
    }

    async fn fetch_parameters_result_set(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<DruidPooledResultSet, DruidError> {
        self.ensure_open_for(connection)?;
        self.statement_base.begin_external_execution();
        let statement = self.shared.holder.statement().clone();
        let result = connection
            .fetch_prepared_parameters_result_set_with_filters(statement.as_ref(), parameters)
            .await;
        match result {
            Ok(physical) => self.complete_physical_query_result_set(physical),
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    async fn execute_parameters(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        self.statement_base.begin_external_execution();
        let statement = self.shared.holder.statement().clone();
        let generated_keys = self.shared.holder.key().statement_generated_keys();
        let result = connection
            .execute_prepared_parameters_with_filters(
                statement.as_ref(),
                parameters,
                generated_keys,
            )
            .await;
        match result {
            Ok(results) => Ok(self.statement_base.complete_external_execute(results)),
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    /// 执行更新类 `PreparedStatement`。
    ///
    /// 参数 `params` 与 Java `setXxx` 后的参数快照等价地一次性交给驱动。
    pub async fn exec(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        self.ensure_open_for(connection)?;
        self.statement_base.begin_external_execution();
        let statement = self.shared.holder.statement().clone();
        let result = connection
            .exec_prepared_with_filters(statement.as_ref(), params)
            .await;
        match &result {
            Ok(execution) => self.statement_base.complete_external_update(execution),
            Err(_) => self.record_exception(),
        }
        result
    }

    /// 执行查询类 `PreparedStatement`。
    pub async fn fetch(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        self.ensure_open_for(connection)?;
        self.statement_base.begin_external_execution();
        let statement = self.shared.holder.statement().clone();
        let result = connection
            .fetch_prepared_with_filters(statement.as_ref(), params)
            .await;
        if let Ok(rows) = &result {
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.fetch_row_peak = state
                .fetch_row_peak
                .max(i32::try_from(rows.len()).unwrap_or(i32::MAX));
            drop(state);
            self.statement_base.complete_external_query(rows.clone());
        } else {
            self.record_exception();
        }
        result
    }

    /// 执行查询并返回池化 `ResultSet`。
    ///
    /// 对应 Java：`DruidPooledPreparedStatement#executeQuery()`。结果集进入与
    /// 普通 `Statement` 相同的 trace、`Filter` open/close/fetch 和级联关闭路径。
    pub async fn fetch_result_set(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<DruidPooledResultSet, DruidError> {
        self.ensure_open_for(connection)?;
        self.statement_base.begin_external_execution();
        let statement = self.shared.holder.statement().clone();
        let result = connection
            .fetch_prepared_result_set_with_filters(statement.as_ref(), params)
            .await;
        match result {
            Ok(physical) => self.complete_physical_query_result_set(physical),
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    fn complete_physical_query_result_set(
        &self,
        physical: Arc<dyn PhysicalResultSet>,
    ) -> Result<DruidPooledResultSet, DruidError> {
        self.statement_base
            .complete_external_query_result_set(Arc::clone(&physical));
        self.statement_base
            .wrap_result_set(physical)
            .map(|result_set| {
                result_set.with_prepared_statement(self.result_set_statement_handle())
            })
    }

    /// 执行 `PreparedStatement#execute()` 并返回首结果是否为 `ResultSet`。
    ///
    /// 参数对应 Java 调用前由 `setXxx` 保存的绑定快照；prepare 时选择的
    /// generated-keys 重载由 `PreparedStatementKey` 原样下沉。
    pub async fn execute(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        self.statement_base.begin_external_execution();
        let statement = self.shared.holder.statement().clone();
        let generated_keys = self.shared.holder.key().statement_generated_keys();
        let result = connection
            .execute_prepared_with_filters(statement.as_ref(), params, generated_keys)
            .await;
        match result {
            Ok(results) => Ok(self.statement_base.complete_external_execute(results)),
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    /// 返回 generic execute 的当前 `ResultSet`。
    pub fn result_set(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Option<DruidPooledResultSet>, DruidError> {
        self.ensure_open_for(connection)?;
        let getter = self.shared.holder.statement().get_result_set();
        if let Err(error) = connection.classify_result(getter) {
            self.record_exception();
            return Err(error);
        }
        self.statement_base
            .result_set(connection)
            .map(|result_set| {
                result_set.map(|result_set| {
                    result_set.with_prepared_statement(self.result_set_statement_handle())
                })
            })
    }

    /// 返回最近一次执行的更新计数；查询或无结果时为 `-1`。
    pub fn update_count(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i64, DruidError> {
        self.ensure_open_for(connection)?;
        let getter = self.shared.holder.statement().get_update_count();
        if let Err(error) = connection.classify_result(getter) {
            self.record_exception();
            return Err(error);
        }
        self.statement_base.update_count(connection)
    }

    /// 返回 generated keys 的池化 ResultSet。
    pub fn generated_keys(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<DruidPooledResultSet, DruidError> {
        self.ensure_open_for(connection)?;
        let getter = self.shared.holder.statement().get_generated_keys();
        if let Err(error) = connection.classify_result(getter) {
            self.record_exception();
            return Err(error);
        }
        self.statement_base
            .generated_keys(connection)
            .map(|result_set| {
                result_set.with_prepared_statement(self.result_set_statement_handle())
            })
    }

    /// 推进到下一个 JDBC 结果。
    pub fn more_results(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.more_results_internal(connection, None)
    }

    /// 使用 JDBC current-result 常量推进结果。
    pub fn more_results_with_current(
        &mut self,
        connection: &mut DruidPooledConnection,
        current: i32,
    ) -> Result<bool, DruidError> {
        self.more_results_internal(connection, Some(current))
    }

    /// 把当前参数快照加入 PreparedStatement 批次。
    ///
    /// 对应 Java：`DruidPooledPreparedStatement#addBatch()`。Rust 没有可变的
    /// `setXxx` 绑定槽位，因此调用方显式传入本次快照；后续参数修改和
    /// `clear_parameters` 不会改变已经加入的批次。
    pub fn add_batch(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.shared.holder.statement().add_batch(&params);
        if let Err(error) = connection.classify_result(result) {
            self.record_exception();
            return Err(error);
        }
        let params = params
            .into_iter()
            .map(PreparedInputParameter::RustValue)
            .collect();
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(params);
        Ok(())
    }

    /// 将当前 `setXxx` 绑定快照加入 Java `addBatch()` 参数批次。
    pub fn add_bound_batch(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let params = self.bound_parameters_or_record_error()?;
        let result = self.shared.holder.statement().add_parameter_batch(&params);
        if let Err(error) = connection.classify_result(result) {
            self.record_exception();
            return Err(error);
        }
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(params);
        Ok(())
    }

    /// 清理当前参数绑定，但不影响已经加入的参数批次。
    ///
    /// 对应 Java：`DruidPooledPreparedStatement#clearParameters()`。
    pub fn clear_parameters(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.shared.holder.statement().clear_parameters();
        if let Err(error) = connection.classify_result(result) {
            self.record_exception();
            return Err(error);
        }
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters
            .clear();
        Ok(())
    }

    /// 清空 PreparedStatement 参数批次；关闭后与 Java `clearBatch()` 一样无操作。
    pub fn clear_batch(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        if self.is_closed() {
            return Ok(());
        }
        self.ensure_open_for(connection)?;
        let result = self.shared.holder.statement().clear_batch();
        if let Err(error) = connection.classify_result(result) {
            self.record_exception();
            return Err(error);
        }
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clear();
        Ok(())
    }

    /// 执行当前 PreparedStatement 参数批次。
    ///
    /// 对应 Java：`DruidPooledPreparedStatement#executeBatch()`。参数按
    /// `add_batch` 时的快照顺序执行；整个批次只进入一次 Filter before/after。
    /// 物理执行开始后，无论成功还是驱动失败，批次都按 JDBC 驱动行为被消费；
    /// before Filter 短路则保留批次。
    pub async fn execute_batch(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Vec<i32>, DruidError> {
        self.ensure_open_for(connection)?;
        let statement = self.shared.holder.statement().clone();
        let mut batch_parameter_sets = {
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut state.batch_parameter_sets)
        };
        let result = connection
            .exec_prepared_batch_with_filters(statement.as_ref(), &mut batch_parameter_sets)
            .await;
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets = batch_parameter_sets;
        if result.is_err() {
            self.record_exception();
        }
        result
    }

    /// 返回尚未执行的 PreparedStatement 参数批次数量。
    pub fn batch_size(&self) -> usize {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .len()
    }

    /// 关闭逻辑语句并按 Java 分支放回缓存或删除物理语句。
    pub fn close(&mut self) -> Result<(), DruidError> {
        if self.is_closed() {
            return Ok(());
        }

        if let Err(error) = self.shared.restore_statement_defaults() {
            self.increment_exception_count();
            return Err(error);
        }

        if self.shared.pooled {
            if let Err(error) = self.shared.holder.statement().clear_parameters() {
                self.increment_exception_count();
                return Err(error);
            }
            if let Err(error) = self.shared.holder.statement().clear_batch() {
                self.increment_exception_count();
                return Err(error);
            }
        }

        self.shared.finish();
        Ok(())
    }

    /// 在原池化连接上下文中关闭逻辑语句并执行 fatal 异常分类。
    ///
    /// 对应 Java：
    /// `DruidPooledConnection#closePoolableStatement`。Rust 语句不长期可变借用
    /// 连接，因此由调用方显式传回创建该语句的同一租约；`clearParameters`、
    /// `clearBatch` 的结构化 SQL 错误会先进入连接的 `ExceptionSorter`，再原样
    /// 返回。跨租约关闭会被拒绝，不能污染下一次借出的连接。
    pub fn close_with_connection(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        if self.is_closed() {
            return Ok(());
        }
        self.ensure_open_for(connection)?;

        let restore_result = self.shared.restore_statement_defaults();
        if let Err(error) = connection.classify_result(restore_result) {
            self.record_exception();
            return Err(error);
        }

        if self.shared.pooled {
            let clear_parameters_result = self.shared.holder.statement().clear_parameters();
            if let Err(error) = connection.classify_result(clear_parameters_result) {
                self.record_exception();
                return Err(error);
            }

            let clear_batch_result = self.shared.holder.statement().clear_batch();
            if let Err(error) = connection.classify_result(clear_batch_result) {
                self.record_exception();
                return Err(error);
            }
        }

        self.shared.finish();
        Ok(())
    }

    pub(crate) fn ensure_open(&self) -> Result<(), DruidError> {
        if self.is_closed() || !self.shared.lease_active.load(Ordering::Acquire) {
            Err(DruidError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    pub(crate) fn record_exception(&mut self) {
        self.increment_exception_count();
        self.statement_base.record_exception();
    }

    fn increment_exception_count(&self) {
        self.shared.increment_exception_count();
    }

    fn set_parameter(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        parameter: PreparedInputParameter,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self
            .shared
            .holder
            .statement()
            .set_parameter(parameter_index, &parameter);
        if let Err(error) = connection.classify_result(result) {
            self.record_exception();
            return Err(error);
        }
        let Some(index) = parameter_index.checked_sub(1) else {
            self.record_exception();
            return Err(DruidError::InvalidArgument(
                "parameterIndex must be at least 1".to_string(),
            ));
        };
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.parameters.len() <= index {
            state.parameters.resize_with(index + 1, || None);
        }
        state.parameters[index] = Some(parameter);
        Ok(())
    }

    fn set_date_with_calendar_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveDate>,
        calendar: JdbcCalendarArgument,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::Date { value, calendar },
        )
    }

    fn set_time_with_calendar_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveTime>,
        calendar: JdbcCalendarArgument,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::Time { value, calendar },
        )
    }

    fn set_timestamp_with_calendar_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        value: Option<NaiveDateTime>,
        calendar: JdbcCalendarArgument,
    ) -> Result<(), DruidError> {
        self.set_parameter(
            connection,
            parameter_index,
            PreparedInputParameter::Timestamp { value, calendar },
        )
    }

    fn set_stream_parameter(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        stream: Option<JdbcInputStream>,
        length: JdbcStreamLength,
        ascii: bool,
    ) -> Result<(), DruidError> {
        let parameter = if ascii {
            PreparedInputParameter::AsciiStream { stream, length }
        } else {
            PreparedInputParameter::BinaryStream { stream, length }
        };
        self.set_parameter(connection, parameter_index, parameter)
    }

    fn set_character_parameter(
        &mut self,
        connection: &mut DruidPooledConnection,
        parameter_index: usize,
        reader: Option<JdbcReader>,
        length: JdbcCharacterLength,
        national: bool,
    ) -> Result<(), DruidError> {
        let parameter = if national {
            PreparedInputParameter::NCharacterStream { reader, length }
        } else {
            PreparedInputParameter::CharacterStream { reader, length }
        };
        self.set_parameter(connection, parameter_index, parameter)
    }

    fn bound_parameters_or_record_error(&self) -> Result<Vec<PreparedInputParameter>, DruidError> {
        let parameters = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters
            .clone();
        let parameters = parameters
            .into_iter()
            .enumerate()
            .map(|(index, parameter)| {
                parameter.ok_or_else(|| {
                    DruidError::InvalidArgument(format!(
                        "parameter {} has not been bound",
                        index + 1
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>();
        if parameters.is_err() {
            self.shared.increment_exception_count();
        }
        parameters
    }

    fn ensure_open_for(&self, connection: &DruidPooledConnection) -> Result<(), DruidError> {
        self.ensure_open()?;
        if connection.is_same_open_lease(&self.shared.lease_active) {
            Ok(())
        } else {
            Err(DruidError::ConnectionDiscarded)
        }
    }

    fn more_results_internal(
        &mut self,
        connection: &mut DruidPooledConnection,
        current: Option<i32>,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let getter = self.shared.holder.statement().get_more_results(current);
        if let Err(error) = connection.classify_result(getter) {
            self.record_exception();
            return Err(error);
        }
        match current {
            Some(current) => self
                .statement_base
                .more_results_with_current(connection, current),
            None => self.statement_base.more_results(connection),
        }
    }
}
