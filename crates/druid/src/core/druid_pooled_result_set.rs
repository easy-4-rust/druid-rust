//! 对外池化结果集。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidPooledResultSet`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/DruidPooledResultSet.java`。

use super::druid_pooled_statement::DruidPooledStatementInner;
use super::{
    DruidError, DruidPooledCallableStatementHandle, DruidPooledConnection,
    DruidPooledPreparedStatementHandle, DruidPooledStatement, FilterChain, JdbcArray, JdbcBlob,
    JdbcCalendar, JdbcCalendarArgument, JdbcCharacterLength, JdbcClob, JdbcInputStream, JdbcNClob,
    JdbcObject, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcStreamLength, JdbcTargetType,
    JdbcTypeMap, JdbcUrl, PhysicalResultSet, ResultSetFilterContext, ResultSetMetaData,
    ResultSetUpdate, SqlWarning, Unwrapped, Value, Wrapper,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};

macro_rules! scalar_update_pair {
    ($index_method:ident, $label_method:ident, $value_type:ty, $variant:ident, $java_name:literal) => {
        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, ..)`。")]
        pub fn $index_method(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            value: $value_type,
        ) -> Result<(), DruidError> {
            self.update_argument(connection, column_index, ResultSetUpdate::$variant(value))
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, ..)`。")]
        pub fn $label_method(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            value: $value_type,
        ) -> Result<(), DruidError> {
            self.update_by_label_argument(
                connection,
                column_label,
                ResultSetUpdate::$variant(value),
            )
        }
    };
}

macro_rules! input_stream_update_family {
    (
        $plain_index:ident, $plain_label:ident,
        $int_index:ident, $int_label:ident,
        $long_index:ident, $long_label:ident,
        $variant:ident, $java_name:literal
    ) => {
        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, InputStream)`。")]
        pub fn $plain_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            stream: Option<&JdbcInputStream>,
        ) -> Result<(), DruidError> {
            self.update_argument(
                connection,
                column_index,
                ResultSetUpdate::$variant {
                    stream: stream.cloned(),
                    length: JdbcStreamLength::Unspecified,
                },
            )
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, InputStream)`。")]
        pub fn $plain_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            stream: Option<&JdbcInputStream>,
        ) -> Result<(), DruidError> {
            self.update_by_label_argument(
                connection,
                column_label,
                ResultSetUpdate::$variant {
                    stream: stream.cloned(),
                    length: JdbcStreamLength::Unspecified,
                },
            )
        }

        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, InputStream, int)`。")]
        pub fn $int_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            stream: Option<&JdbcInputStream>,
            length: i32,
        ) -> Result<(), DruidError> {
            self.update_argument(
                connection,
                column_index,
                ResultSetUpdate::$variant {
                    stream: stream.cloned(),
                    length: JdbcStreamLength::Int(length),
                },
            )
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, InputStream, int)`。")]
        pub fn $int_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            stream: Option<&JdbcInputStream>,
            length: i32,
        ) -> Result<(), DruidError> {
            self.update_by_label_argument(
                connection,
                column_label,
                ResultSetUpdate::$variant {
                    stream: stream.cloned(),
                    length: JdbcStreamLength::Int(length),
                },
            )
        }

        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, InputStream, long)`。")]
        pub fn $long_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            stream: Option<&JdbcInputStream>,
            length: i64,
        ) -> Result<(), DruidError> {
            self.update_argument(
                connection,
                column_index,
                ResultSetUpdate::$variant {
                    stream: stream.cloned(),
                    length: JdbcStreamLength::Long(length),
                },
            )
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, InputStream, long)`。")]
        pub fn $long_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            stream: Option<&JdbcInputStream>,
            length: i64,
        ) -> Result<(), DruidError> {
            self.update_by_label_argument(
                connection,
                column_label,
                ResultSetUpdate::$variant {
                    stream: stream.cloned(),
                    length: JdbcStreamLength::Long(length),
                },
            )
        }
    };
}

macro_rules! reader_update_family {
    (
        $plain_index:ident, $plain_label:ident,
        $int_index:ident, $int_label:ident,
        $long_index:ident, $long_label:ident,
        $variant:ident, $java_name:literal
    ) => {
        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, Reader)`。")]
        pub fn $plain_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            reader: Option<&JdbcReader>,
        ) -> Result<(), DruidError> {
            self.update_argument(
                connection,
                column_index,
                ResultSetUpdate::$variant {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Unspecified,
                },
            )
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, Reader)`。")]
        pub fn $plain_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            reader: Option<&JdbcReader>,
        ) -> Result<(), DruidError> {
            self.update_by_label_argument(
                connection,
                column_label,
                ResultSetUpdate::$variant {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Unspecified,
                },
            )
        }

        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, Reader, int)`。")]
        pub fn $int_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            reader: Option<&JdbcReader>,
            length: i32,
        ) -> Result<(), DruidError> {
            self.update_argument(
                connection,
                column_index,
                ResultSetUpdate::$variant {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Int(length),
                },
            )
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, Reader, int)`。")]
        pub fn $int_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            reader: Option<&JdbcReader>,
            length: i32,
        ) -> Result<(), DruidError> {
            self.update_by_label_argument(
                connection,
                column_label,
                ResultSetUpdate::$variant {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Int(length),
                },
            )
        }

        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, Reader, long)`。")]
        pub fn $long_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            reader: Option<&JdbcReader>,
            length: i64,
        ) -> Result<(), DruidError> {
            self.update_argument(
                connection,
                column_index,
                ResultSetUpdate::$variant {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Long(length),
                },
            )
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, Reader, long)`。")]
        pub fn $long_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            reader: Option<&JdbcReader>,
            length: i64,
        ) -> Result<(), DruidError> {
            self.update_by_label_argument(
                connection,
                column_label,
                ResultSetUpdate::$variant {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Long(length),
                },
            )
        }
    };
}

/// Statement 结果集追踪表持有的共享关闭状态。
///
/// Java `DruidPooledStatement#clearResultSet()` 保存的是包装结果集本身。Rust
/// 使用弱物理引用和共享原子状态表达相同生命周期，避免 Statement 与 ResultSet
/// 形成强引用环。
#[derive(Clone)]
pub(crate) struct DruidPooledResultSetTrace {
    physical: Weak<dyn PhysicalResultSet>,
    closed: Arc<AtomicBool>,
    filter_chain: Option<Arc<FilterChain>>,
    filter_context: Arc<ResultSetFilterContext>,
}

impl DruidPooledResultSetTrace {
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
            || self
                .physical
                .upgrade()
                .is_none_or(|result_set| result_set.is_closed())
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Some(result_set) = self.physical.upgrade() {
            let result = self.filter_chain.as_ref().map_or_else(
                || result_set.close(),
                |chain| chain.result_set_close(result_set.as_ref(), &self.filter_context),
            );
            if result.is_ok() {
                self.filter_context.increment_close_count();
            }
        }
    }

    /// 仅把池化 wrapper 标记为关闭，不触发显式 ResultSet close Filter。
    ///
    /// 对应 Java：`DruidPooledStatement#getMoreResults(...)` 在底层 Statement
    /// 成功推进后直接设置最后一个 `DruidPooledResultSet.closed = true`。
    pub(crate) fn mark_closed_by_more_results(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub(crate) fn fetch_row_count(&self) -> i32 {
        self.filter_context.fetch_row_count()
    }
}

/// 池化 Statement 返回的结果集包装。
///
/// 该对象保留 Java 的 Statement 身份、游标计数、抓取峰值回写、异常计数及
/// 关闭生命周期。Rust 需要显式传入原 `DruidPooledConnection` 才能把物理
/// ResultSet 错误交给同一连接的 `ExceptionSorter`。
pub struct DruidPooledResultSet {
    statement: DruidPooledStatement,
    prepared_statement: Option<DruidPooledPreparedStatementHandle>,
    callable_statement: Option<DruidPooledCallableStatementHandle>,
    physical: Arc<dyn PhysicalResultSet>,
    closed: Arc<AtomicBool>,
    cursor_index: AtomicI32,
    filter_chain: Option<Arc<FilterChain>>,
    filter_context: Arc<ResultSetFilterContext>,
}

impl DruidPooledResultSet {
    pub(crate) fn new(
        statement: Arc<DruidPooledStatementInner>,
        physical: Arc<dyn PhysicalResultSet>,
    ) -> Result<Self, DruidError> {
        let filter_chain = statement.filter_chain.clone();
        let filter_context = Arc::new(ResultSetFilterContext::new());
        let result_set = Self {
            statement: DruidPooledStatement::from_inner(statement),
            prepared_statement: None,
            callable_statement: None,
            physical,
            closed: Arc::new(AtomicBool::new(false)),
            cursor_index: AtomicI32::new(0),
            filter_chain,
            filter_context,
        };
        if let Some(filter_chain) = &result_set.filter_chain {
            filter_chain.result_set_open_after(&result_set.filter_context)?;
        }
        Ok(result_set)
    }

    pub(crate) fn with_prepared_statement(
        mut self,
        prepared_statement: DruidPooledPreparedStatementHandle,
    ) -> Self {
        self.prepared_statement = Some(prepared_statement);
        self
    }

    pub(crate) fn with_callable_statement(
        mut self,
        callable_statement: DruidPooledCallableStatementHandle,
    ) -> Self {
        self.callable_statement = Some(callable_statement);
        self
    }

    pub(crate) fn trace(&self) -> DruidPooledResultSetTrace {
        DruidPooledResultSetTrace {
            physical: Arc::downgrade(&self.physical),
            closed: Arc::clone(&self.closed),
            filter_chain: self.filter_chain.clone(),
            filter_context: Arc::clone(&self.filter_context),
        }
    }

    /// 返回创建本结果集的池化 Statement。
    ///
    /// 对应 Java：`DruidPooledResultSet#getPoolableStatement()`。
    pub fn poolable_statement(&self) -> &DruidPooledStatement {
        &self.statement
    }

    /// 返回底层物理结果集 SPI。
    ///
    /// 对应 Java：`DruidPooledResultSet#getRawResultSet()`。
    pub fn raw_result_set(&self) -> &dyn PhysicalResultSet {
        self.physical.as_ref()
    }

    /// 返回创建本结果集的 Statement。
    ///
    /// 对应 Java：`DruidPooledResultSet#getStatement()`；该方法返回池化
    /// Statement 身份，不委托 raw ResultSet。
    pub fn statement(&self) -> &DruidPooledStatement {
        &self.statement
    }

    /// 尝试把 `getStatement()` 的动态身份恢复为 PreparedStatement。
    ///
    /// 对应 Java：当本结果集由 `DruidPooledPreparedStatement` 创建时，
    /// `ResultSet#getStatement()` 返回同一逻辑 PreparedStatement 对象。普通
    /// Statement 结果集返回 `None`。
    pub fn prepared_statement(&self) -> Option<&DruidPooledPreparedStatementHandle> {
        self.prepared_statement.as_ref()
    }

    /// 尝试把 `getStatement()` 的动态身份恢复为 CallableStatement。
    ///
    /// 对应 Java：当结果集由 `DruidPooledCallableStatement` 创建时，返回同一
    /// callable 对象；普通 Statement/PreparedStatement 结果集返回 `None`。
    pub fn callable_statement(&self) -> Option<&DruidPooledCallableStatementHandle> {
        self.callable_statement.as_ref()
    }

    /// 移到下一行。
    ///
    /// 对应 Java：`DruidPooledResultSet#next()`；只有成功移动时才增加
    /// `cursorIndex` 和 `fetchRowCount`。
    pub fn next(&mut self, connection: &mut DruidPooledConnection) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.next(),
            |chain| chain.result_set_next(self.physical.as_ref(), &self.filter_context),
        );
        let more_rows = self.classify(connection, result)?;
        if more_rows {
            let cursor_index = self
                .cursor_index
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |cursor_index| {
                    Some(cursor_index.saturating_add(1))
                })
                .unwrap_or_else(|cursor_index| cursor_index)
                .saturating_add(1);
            self.filter_context.record_fetch_row_count(cursor_index);
        }
        Ok(more_rows)
    }

    /// 移到上一行。
    ///
    /// 对应 Java：`DruidPooledResultSet#previous()`。
    pub fn previous(&mut self, connection: &mut DruidPooledConnection) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.previous(),
            |chain| chain.result_set_previous(self.physical.as_ref(), &self.filter_context),
        );
        let more_rows = self.classify(connection, result)?;
        if more_rows {
            let _ = self.cursor_index.fetch_update(
                Ordering::AcqRel,
                Ordering::Acquire,
                |cursor_index| Some(cursor_index.saturating_sub(1)),
            );
        }
        Ok(more_rows)
    }

    /// 关闭结果集并把抓取行数回写 Statement。
    ///
    /// 对应 Java：`DruidPooledResultSet#close()`。Java 会先把逻辑 closed
    /// 设为 true，再关闭 raw ResultSet；raw close 失败时也保持 closed。
    pub fn close_with_connection(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_same_lease(connection)?;
        self.closed.store(true, Ordering::Release);
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.close(),
            |chain| chain.result_set_close(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)?;
        self.filter_context.increment_close_count();
        self.statement
            .record_fetch_row_count(self.fetch_row_count());
        Ok(())
    }

    /// 返回逻辑结果集是否关闭。
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// 经 Java `ResultSet#isClosed()` FilterChain 查询物理关闭状态。
    ///
    /// `is_closed()` 保留为 Rust 内部无失败生命周期观察器；对外 JDBC 语义入口
    /// 使用本方法，使 Filter 可以短路、改写或返回驱动错误。
    pub fn is_closed_with_connection(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.ensure_same_lease(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || Ok(self.physical.is_closed()),
            |chain| chain.result_set_is_closed(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回成功抓取的历史峰值行号。
    ///
    /// 对应 Java：`DruidPooledResultSet#getFetchRowCount()`。
    pub fn fetch_row_count(&self) -> i32 {
        self.filter_context.fetch_row_count()
    }

    /// 返回最近一次 getter 是否读取 SQL NULL。
    pub fn was_null(&mut self, connection: &mut DruidPooledConnection) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.was_null(),
            |chain| chain.result_set_was_null(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 按 1-based 下标读取通用值。
    pub fn object(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        self.value(connection, column_index)
    }

    /// 按标签读取通用值。
    pub fn object_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.value_by_label(column_label),
            |chain| {
                chain.result_set_get_object_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标和 SQL 类型映射读取对象。
    ///
    /// 对应 Java：`DruidPooledResultSet#getObject(int, Map<String, Class<?>>)`；
    /// `type_map=None` 精确保留 Java `null` 参数并直接交给物理驱动。
    pub fn object_with_type_map(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.object_with_type_map(column_index, type_map),
            |chain| {
                chain.result_set_get_object_with_type_map(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    type_map,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签和 SQL 类型映射读取对象。
    ///
    /// 对应 Java：`DruidPooledResultSet#getObject(String, Map<String, Class<?>>)`。
    pub fn object_by_label_with_type_map(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || {
                self.physical
                    .object_by_label_with_type_map(column_label, type_map)
            },
            |chain| {
                chain.result_set_get_object_by_label_with_type_map(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    type_map,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标和目标类型读取对象。
    ///
    /// 对应 Java：`DruidPooledResultSet#getObject(int, Class<T>)`。Rust 用
    /// `JdbcTargetType` 代替 JVM `Class<T>`，目标类型原样下沉到物理 SPI。
    pub fn object_typed(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.object_as(column_index, target_type),
            |chain| {
                chain.result_set_get_object_typed(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    target_type,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签和目标类型读取对象。
    ///
    /// 对应 Java：`DruidPooledResultSet#getObject(String, Class<T>)`。
    pub fn object_typed_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.object_by_label_as(column_label, target_type),
            |chain| {
                chain.result_set_get_object_typed_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    target_type,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取字符串；SQL NULL 返回 `None`。
    pub fn string(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<String>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.string(column_index),
            |chain| {
                chain.result_set_get_string(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取字符串。
    pub fn string_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<String>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.string_by_label(column_label),
            |chain| {
                chain.result_set_get_string_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取布尔值；SQL NULL 与 JDBC 一致返回 false。
    pub fn boolean(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.boolean(column_index),
            |chain| {
                chain.result_set_get_boolean(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取布尔值。
    pub fn boolean_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.boolean_by_label(column_label),
            |chain| {
                chain.result_set_get_boolean_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 i64；SQL NULL 与 JDBC 一致返回 0。
    pub fn long(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<i64, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.long(column_index),
            |chain| {
                chain.result_set_get_long(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 i64。
    pub fn long_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<i64, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.long_by_label(column_label),
            |chain| {
                chain.result_set_get_long_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 i32。
    pub fn int(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.int(column_index),
            |chain| {
                chain.result_set_get_int(self.physical.as_ref(), &self.filter_context, column_index)
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 i32。
    pub fn int_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.int_by_label(column_label),
            |chain| {
                chain.result_set_get_int_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 i16。
    pub fn short(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<i16, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.short(column_index),
            |chain| {
                chain.result_set_get_short(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 i16。
    pub fn short_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<i16, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.short_by_label(column_label),
            |chain| {
                chain.result_set_get_short_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 i8。
    pub fn byte(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<i8, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.byte(column_index),
            |chain| {
                chain.result_set_get_byte(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 i8。
    pub fn byte_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<i8, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.byte_by_label(column_label),
            |chain| {
                chain.result_set_get_byte_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 f64；SQL NULL 返回 0。
    pub fn double(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<f64, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.double(column_index),
            |chain| {
                chain.result_set_get_double(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 f64。
    pub fn double_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<f64, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.double_by_label(column_label),
            |chain| {
                chain.result_set_get_double_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 f32。
    pub fn float(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<f32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.float(column_index),
            |chain| {
                chain.result_set_get_float(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 f32。
    pub fn float_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<f32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.float_by_label(column_label),
            |chain| {
                chain.result_set_get_float_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取字节；SQL NULL 返回 `None`。
    pub fn bytes(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<Vec<u8>>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.bytes(column_index),
            |chain| {
                chain.result_set_get_bytes(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取字节。
    pub fn bytes_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<Vec<u8>>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.bytes_by_label(column_label),
            |chain| {
                chain.result_set_get_bytes_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取任意精度 Decimal。
    ///
    /// 对应 Java：`DruidPooledResultSet#getBigDecimal(int)`；委托物理
    /// ResultSet 后的任何错误都进入 Statement `checkException` 等价路径。
    pub fn big_decimal(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.big_decimal(column_index, None),
            |chain| {
                chain.result_set_get_big_decimal(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取任意精度 Decimal。
    ///
    /// 对应 Java：`DruidPooledResultSet#getBigDecimal(String)`。
    pub fn big_decimal_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.big_decimal_by_label(column_label, None),
            |chain| {
                chain.result_set_get_big_decimal_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 使用已废弃 JDBC scale 重载按下标读取 Decimal。
    ///
    /// 对应 Java：`DruidPooledResultSet#getBigDecimal(int, int)`。
    pub fn big_decimal_with_scale(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.big_decimal(column_index, Some(scale)),
            |chain| {
                chain.result_set_get_big_decimal_with_scale(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    scale,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 使用已废弃 JDBC scale 重载按标签读取 Decimal。
    ///
    /// 对应 Java：`DruidPooledResultSet#getBigDecimal(String, int)`。
    pub fn big_decimal_by_label_with_scale(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || {
                self.physical
                    .big_decimal_by_label(column_label, Some(scale))
            },
            |chain| {
                chain.result_set_get_big_decimal_by_label_with_scale(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    scale,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 DATE。
    ///
    /// 对应 Java：`DruidPooledResultSet#getDate(int)`。
    pub fn date(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.date_argument(
            connection,
            column_index,
            JdbcCalendarArgument::unspecified(),
        )
    }

    /// 按标签读取 DATE。
    pub fn date_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.date_by_label_argument(
            connection,
            column_label,
            JdbcCalendarArgument::unspecified(),
        )
    }

    /// 按下标和 Calendar 读取 DATE。
    ///
    /// `calendar=None` 对应显式向 Java Calendar 重载传入 null。
    pub fn date_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        calendar: Option<JdbcCalendar>,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.date_argument(
            connection,
            column_index,
            JdbcCalendarArgument::specified(calendar),
        )
    }

    /// 按标签和 Calendar 读取 DATE。
    pub fn date_by_label_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        calendar: Option<JdbcCalendar>,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.date_by_label_argument(
            connection,
            column_label,
            JdbcCalendarArgument::specified(calendar),
        )
    }

    /// 按下标读取 TIME。
    pub fn time(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.time_argument(
            connection,
            column_index,
            JdbcCalendarArgument::unspecified(),
        )
    }

    /// 按标签读取 TIME。
    pub fn time_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.time_by_label_argument(
            connection,
            column_label,
            JdbcCalendarArgument::unspecified(),
        )
    }

    /// 按下标和 Calendar 读取 TIME。
    pub fn time_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        calendar: Option<JdbcCalendar>,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.time_argument(
            connection,
            column_index,
            JdbcCalendarArgument::specified(calendar),
        )
    }

    /// 按标签和 Calendar 读取 TIME。
    pub fn time_by_label_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        calendar: Option<JdbcCalendar>,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.time_by_label_argument(
            connection,
            column_label,
            JdbcCalendarArgument::specified(calendar),
        )
    }

    /// 按下标读取 TIMESTAMP。
    pub fn timestamp(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.timestamp_argument(
            connection,
            column_index,
            JdbcCalendarArgument::unspecified(),
        )
    }

    /// 按标签读取 TIMESTAMP。
    pub fn timestamp_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.timestamp_by_label_argument(
            connection,
            column_label,
            JdbcCalendarArgument::unspecified(),
        )
    }

    /// 按下标和 Calendar 读取 TIMESTAMP。
    pub fn timestamp_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        calendar: Option<JdbcCalendar>,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.timestamp_argument(
            connection,
            column_index,
            JdbcCalendarArgument::specified(calendar),
        )
    }

    /// 按标签和 Calendar 读取 TIMESTAMP。
    pub fn timestamp_by_label_with_calendar(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        calendar: Option<JdbcCalendar>,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.timestamp_by_label_argument(
            connection,
            column_label,
            JdbcCalendarArgument::specified(calendar),
        )
    }

    /// 按下标读取 JDBC `Ref`。
    ///
    /// 对应 Java：`DruidPooledResultSet#getRef(int)`。
    pub fn reference(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcRef>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.reference(column_index),
            |chain| {
                chain.result_set_get_ref(self.physical.as_ref(), &self.filter_context, column_index)
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 JDBC `Ref`。
    pub fn reference_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcRef>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.reference_by_label(column_label),
            |chain| {
                chain.result_set_get_ref_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 JDBC `Blob`。
    pub fn blob(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcBlob>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.blob(column_index),
            |chain| {
                chain.result_set_get_blob(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 JDBC `Blob`。
    pub fn blob_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcBlob>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.blob_by_label(column_label),
            |chain| {
                chain.result_set_get_blob_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 JDBC `Clob`。
    pub fn clob(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcClob>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.clob(column_index),
            |chain| {
                chain.result_set_get_clob(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 JDBC `Clob`。
    pub fn clob_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcClob>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.clob_by_label(column_label),
            |chain| {
                chain.result_set_get_clob_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 JDBC `Array`。
    pub fn array(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcArray>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.array(column_index),
            |chain| {
                chain.result_set_get_array(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 JDBC `Array`。
    pub fn array_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcArray>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.array_by_label(column_label),
            |chain| {
                chain.result_set_get_array_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 JDBC `URL`。
    pub fn url(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcUrl>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.url(column_index),
            |chain| {
                chain.result_set_get_url(self.physical.as_ref(), &self.filter_context, column_index)
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 JDBC `URL`。
    pub fn url_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcUrl>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.url_by_label(column_label),
            |chain| {
                chain.result_set_get_url_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 JDBC `RowId`。
    pub fn row_id(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcRowId>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.row_id(column_index),
            |chain| {
                chain.result_set_get_row_id(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 JDBC `RowId`。
    pub fn row_id_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcRowId>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.row_id_by_label(column_label),
            |chain| {
                chain.result_set_get_row_id_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 JDBC `NClob`。
    pub fn n_clob(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcNClob>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.n_clob(column_index),
            |chain| {
                chain.result_set_get_n_clob(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 JDBC `NClob`。
    pub fn n_clob_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcNClob>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.n_clob_by_label(column_label),
            |chain| {
                chain.result_set_get_n_clob_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 JDBC `SQLXML`。
    pub fn sql_xml(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcSqlXml>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.sql_xml(column_index),
            |chain| {
                chain.result_set_get_sql_xml(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 JDBC `SQLXML`。
    pub fn sql_xml_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcSqlXml>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.sql_xml_by_label(column_label),
            |chain| {
                chain.result_set_get_sql_xml_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标执行 Java `ResultSet#updateNull(int)`。
    pub fn update_null(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<(), DruidError> {
        self.update_argument(connection, column_index, ResultSetUpdate::Null)
    }

    /// 按标签执行 Java `ResultSet#updateNull(String)`。
    pub fn update_null_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<(), DruidError> {
        self.update_by_label_argument(connection, column_label, ResultSetUpdate::Null)
    }

    scalar_update_pair!(
        update_boolean,
        update_boolean_by_label,
        bool,
        Boolean,
        "updateBoolean"
    );
    scalar_update_pair!(update_byte, update_byte_by_label, i8, Byte, "updateByte");
    scalar_update_pair!(
        update_short,
        update_short_by_label,
        i16,
        Short,
        "updateShort"
    );
    scalar_update_pair!(update_int, update_int_by_label, i32, Int, "updateInt");
    scalar_update_pair!(update_long, update_long_by_label, i64, Long, "updateLong");
    scalar_update_pair!(
        update_float,
        update_float_by_label,
        f32,
        Float,
        "updateFloat"
    );
    scalar_update_pair!(
        update_double,
        update_double_by_label,
        f64,
        Double,
        "updateDouble"
    );
    scalar_update_pair!(
        update_big_decimal,
        update_big_decimal_by_label,
        Option<BigDecimal>,
        BigDecimal,
        "updateBigDecimal"
    );
    scalar_update_pair!(
        update_string,
        update_string_by_label,
        Option<String>,
        String,
        "updateString"
    );
    scalar_update_pair!(
        update_bytes,
        update_bytes_by_label,
        Option<Vec<u8>>,
        Bytes,
        "updateBytes"
    );
    scalar_update_pair!(
        update_date,
        update_date_by_label,
        Option<NaiveDate>,
        Date,
        "updateDate"
    );
    scalar_update_pair!(
        update_time,
        update_time_by_label,
        Option<NaiveTime>,
        Time,
        "updateTime"
    );
    scalar_update_pair!(
        update_timestamp,
        update_timestamp_by_label,
        Option<NaiveDateTime>,
        Timestamp,
        "updateTimestamp"
    );

    /// 按下标执行 Java `ResultSet#updateObject(int, Object)`。
    pub fn update_object(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: JdbcObject,
    ) -> Result<(), DruidError> {
        self.update_argument(connection, column_index, ResultSetUpdate::Object(value))
    }

    /// 按标签执行 Java `ResultSet#updateObject(String, Object)`。
    pub fn update_object_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: JdbcObject,
    ) -> Result<(), DruidError> {
        self.update_by_label_argument(connection, column_label, ResultSetUpdate::Object(value))
    }

    /// 按下标执行 Java `ResultSet#updateObject(int, Object, int)`。
    pub fn update_object_with_scale_or_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: JdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        self.update_argument(
            connection,
            column_index,
            ResultSetUpdate::ObjectWithScaleOrLength {
                value,
                scale_or_length,
            },
        )
    }

    /// 按标签执行 Java `ResultSet#updateObject(String, Object, int)`。
    pub fn update_object_by_label_with_scale_or_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: JdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        self.update_by_label_argument(
            connection,
            column_label,
            ResultSetUpdate::ObjectWithScaleOrLength {
                value,
                scale_or_length,
            },
        )
    }

    scalar_update_pair!(
        update_n_string,
        update_n_string_by_label,
        Option<String>,
        NString,
        "updateNString"
    );

    input_stream_update_family!(
        update_ascii_stream,
        update_ascii_stream_by_label,
        update_ascii_stream_with_int_length,
        update_ascii_stream_by_label_with_int_length,
        update_ascii_stream_with_length,
        update_ascii_stream_by_label_with_length,
        AsciiStream,
        "updateAsciiStream"
    );
    input_stream_update_family!(
        update_binary_stream,
        update_binary_stream_by_label,
        update_binary_stream_with_int_length,
        update_binary_stream_by_label_with_int_length,
        update_binary_stream_with_length,
        update_binary_stream_by_label_with_length,
        BinaryStream,
        "updateBinaryStream"
    );
    reader_update_family!(
        update_character_stream,
        update_character_stream_by_label,
        update_character_stream_with_int_length,
        update_character_stream_by_label_with_int_length,
        update_character_stream_with_length,
        update_character_stream_by_label_with_length,
        CharacterStream,
        "updateCharacterStream"
    );

    /// 按下标执行 Java `ResultSet#updateNCharacterStream(int, Reader)`。
    pub fn update_n_character_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
    ) -> Result<(), DruidError> {
        self.update_argument(
            connection,
            column_index,
            ResultSetUpdate::NCharacterStream {
                reader: reader.cloned(),
                length: JdbcCharacterLength::Unspecified,
            },
        )
    }

    /// 按标签执行 Java `ResultSet#updateNCharacterStream(String, Reader)`。
    pub fn update_n_character_stream_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
    ) -> Result<(), DruidError> {
        self.update_by_label_argument(
            connection,
            column_label,
            ResultSetUpdate::NCharacterStream {
                reader: reader.cloned(),
                length: JdbcCharacterLength::Unspecified,
            },
        )
    }

    /// 按下标执行 Java `ResultSet#updateNCharacterStream(int, Reader, long)`。
    pub fn update_n_character_stream_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.update_argument(
            connection,
            column_index,
            ResultSetUpdate::NCharacterStream {
                reader: reader.cloned(),
                length: JdbcCharacterLength::Long(length),
            },
        )
    }

    /// 按标签执行 Java `ResultSet#updateNCharacterStream(String, Reader, long)`。
    pub fn update_n_character_stream_by_label_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.update_by_label_argument(
            connection,
            column_label,
            ResultSetUpdate::NCharacterStream {
                reader: reader.cloned(),
                length: JdbcCharacterLength::Long(length),
            },
        )
    }

    /// 按下标更新 JDBC `Ref`。
    pub fn update_reference(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: Option<&JdbcRef>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_reference(column_index, value)
        })
    }

    /// 按标签更新 JDBC `Ref`。
    pub fn update_reference_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: Option<&JdbcRef>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_reference_by_label(column_label, value)
        })
    }

    /// 按下标更新 JDBC `Blob`。
    pub fn update_blob(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: Option<&JdbcBlob>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_blob(column_index, value)
        })
    }

    /// 按标签更新 JDBC `Blob`。
    pub fn update_blob_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: Option<&JdbcBlob>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_blob_by_label(column_label, value)
        })
    }

    /// 按下标更新 JDBC `Clob`。
    pub fn update_clob(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: Option<&JdbcClob>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_clob(column_index, value)
        })
    }

    /// 按标签更新 JDBC `Clob`。
    pub fn update_clob_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: Option<&JdbcClob>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_clob_by_label(column_label, value)
        })
    }

    /// 按下标更新 JDBC `Array`。
    pub fn update_array(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: Option<&JdbcArray>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_array(column_index, value)
        })
    }

    /// 按标签更新 JDBC `Array`。
    pub fn update_array_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: Option<&JdbcArray>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_array_by_label(column_label, value)
        })
    }

    /// 按下标更新 JDBC `RowId`。
    pub fn update_row_id(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: Option<&JdbcRowId>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_row_id(column_index, value)
        })
    }

    /// 按标签更新 JDBC `RowId`。
    pub fn update_row_id_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: Option<&JdbcRowId>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_row_id_by_label(column_label, value)
        })
    }

    /// 按下标更新 JDBC `NClob`。
    pub fn update_n_clob(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: Option<&JdbcNClob>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_n_clob(column_index, value)
        })
    }

    /// 按标签更新 JDBC `NClob`。
    pub fn update_n_clob_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: Option<&JdbcNClob>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_n_clob_by_label(column_label, value)
        })
    }

    /// 按下标更新 JDBC `SQLXML`。
    pub fn update_sql_xml(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: Option<&JdbcSqlXml>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_sql_xml(column_index, value)
        })
    }

    /// 按标签更新 JDBC `SQLXML`。
    pub fn update_sql_xml_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: Option<&JdbcSqlXml>,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_sql_xml_by_label(column_label, value)
        })
    }

    /// 按下标使用无长度输入流重载更新 `Blob`。
    pub fn update_blob_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        stream: Option<&JdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.update_blob_stream_argument(
            connection,
            column_index,
            stream,
            JdbcStreamLength::Unspecified,
        )
    }

    /// 按标签使用无长度输入流重载更新 `Blob`。
    pub fn update_blob_stream_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        stream: Option<&JdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.update_blob_stream_by_label_argument(
            connection,
            column_label,
            stream,
            JdbcStreamLength::Unspecified,
        )
    }

    /// 按下标使用 long 长度输入流重载更新 `Blob`。
    pub fn update_blob_stream_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        stream: Option<&JdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.update_blob_stream_argument(
            connection,
            column_index,
            stream,
            JdbcStreamLength::Long(length),
        )
    }

    /// 按标签使用 long 长度输入流重载更新 `Blob`。
    pub fn update_blob_stream_by_label_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        stream: Option<&JdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.update_blob_stream_by_label_argument(
            connection,
            column_label,
            stream,
            JdbcStreamLength::Long(length),
        )
    }

    /// 按下标使用无长度 Reader 重载更新 `Clob`。
    pub fn update_clob_reader(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
    ) -> Result<(), DruidError> {
        self.update_clob_reader_argument(
            connection,
            column_index,
            reader,
            JdbcCharacterLength::Unspecified,
        )
    }

    /// 按标签使用无长度 Reader 重载更新 `Clob`。
    pub fn update_clob_reader_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
    ) -> Result<(), DruidError> {
        self.update_clob_reader_by_label_argument(
            connection,
            column_label,
            reader,
            JdbcCharacterLength::Unspecified,
        )
    }

    /// 按下标使用 long 长度 Reader 重载更新 `Clob`。
    pub fn update_clob_reader_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.update_clob_reader_argument(
            connection,
            column_index,
            reader,
            JdbcCharacterLength::Long(length),
        )
    }

    /// 按标签使用 long 长度 Reader 重载更新 `Clob`。
    pub fn update_clob_reader_by_label_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.update_clob_reader_by_label_argument(
            connection,
            column_label,
            reader,
            JdbcCharacterLength::Long(length),
        )
    }

    /// 按下标使用无长度 Reader 重载更新 `NClob`。
    pub fn update_n_clob_reader(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
    ) -> Result<(), DruidError> {
        self.update_n_clob_reader_argument(
            connection,
            column_index,
            reader,
            JdbcCharacterLength::Unspecified,
        )
    }

    /// 按标签使用无长度 Reader 重载更新 `NClob`。
    pub fn update_n_clob_reader_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
    ) -> Result<(), DruidError> {
        self.update_n_clob_reader_by_label_argument(
            connection,
            column_label,
            reader,
            JdbcCharacterLength::Unspecified,
        )
    }

    /// 按下标使用 long 长度 Reader 重载更新 `NClob`。
    pub fn update_n_clob_reader_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.update_n_clob_reader_argument(
            connection,
            column_index,
            reader,
            JdbcCharacterLength::Long(length),
        )
    }

    /// 按标签使用 long 长度 Reader 重载更新 `NClob`。
    pub fn update_n_clob_reader_by_label_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.update_n_clob_reader_by_label_argument(
            connection,
            column_label,
            reader,
            JdbcCharacterLength::Long(length),
        )
    }

    /// 按下标读取 NString。
    ///
    /// 对应 Java：`DruidPooledResultSet#getNString(int)`；Rust `String`
    /// 本身为 Unicode 值，eager Adapter 与普通字符串共享转换规则。
    pub fn n_string(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<String>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.n_string(column_index),
            |chain| {
                chain.result_set_get_n_string(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 NString。
    pub fn n_string_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<String>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.n_string_by_label(column_label),
            |chain| {
                chain.result_set_get_n_string_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 ASCII 输入流。
    pub fn ascii_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.ascii_stream(column_index),
            |chain| {
                chain.result_set_get_ascii_stream(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 ASCII 输入流。
    pub fn ascii_stream_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.ascii_stream_by_label(column_label),
            |chain| {
                chain.result_set_get_ascii_stream_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取已废弃的 Unicode 输入流。
    pub fn unicode_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.unicode_stream(column_index),
            |chain| {
                chain.result_set_get_unicode_stream(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取已废弃的 Unicode 输入流。
    pub fn unicode_stream_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.unicode_stream_by_label(column_label),
            |chain| {
                chain.result_set_get_unicode_stream_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取二进制输入流。
    pub fn binary_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.binary_stream(column_index),
            |chain| {
                chain.result_set_get_binary_stream(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取二进制输入流。
    pub fn binary_stream_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.binary_stream_by_label(column_label),
            |chain| {
                chain.result_set_get_binary_stream_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取字符 Reader。
    pub fn character_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcReader>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.character_stream(column_index),
            |chain| {
                chain.result_set_get_character_stream(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取字符 Reader。
    pub fn character_stream_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.character_stream_by_label(column_label),
            |chain| {
                chain.result_set_get_character_stream_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按下标读取 NCharacterStream。
    pub fn n_character_stream(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Option<JdbcReader>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.n_character_stream(column_index),
            |chain| {
                chain.result_set_get_n_character_stream(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签读取 NCharacterStream。
    pub fn n_character_stream_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.n_character_stream_by_label(column_label),
            |chain| {
                chain.result_set_get_n_character_stream_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签查找 1-based 列下标。
    pub fn find_column(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<usize, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.find_column(column_label),
            |chain| {
                chain.result_set_find_column(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 返回是否位于第一行之前。
    pub fn is_before_first(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.is_before_first(),
            |chain| chain.result_set_is_before_first(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回是否位于最后一行之后。
    pub fn is_after_last(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.is_after_last(),
            |chain| chain.result_set_is_after_last(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回是否位于第一行。
    pub fn is_first(&mut self, connection: &mut DruidPooledConnection) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.is_first(),
            |chain| chain.result_set_is_first(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回是否位于最后一行。
    pub fn is_last(&mut self, connection: &mut DruidPooledConnection) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.is_last(),
            |chain| chain.result_set_is_last(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 移到第一行之前。
    pub fn before_first(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.before_first(),
            |chain| chain.result_set_before_first(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 移到最后一行之后。
    pub fn after_last(&mut self, connection: &mut DruidPooledConnection) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.after_last(),
            |chain| chain.result_set_after_last(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 移到第一行。
    pub fn first(&mut self, connection: &mut DruidPooledConnection) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.first(),
            |chain| chain.result_set_first(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 移到最后一行。
    pub fn last(&mut self, connection: &mut DruidPooledConnection) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.last(),
            |chain| chain.result_set_last(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回当前 JDBC 行号。
    pub fn row(&mut self, connection: &mut DruidPooledConnection) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.row(),
            |chain| chain.result_set_get_row(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 按绝对行号定位。
    pub fn absolute(
        &mut self,
        connection: &mut DruidPooledConnection,
        row: i32,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.absolute(row),
            |chain| chain.result_set_absolute(self.physical.as_ref(), &self.filter_context, row),
        );
        self.classify(connection, result)
    }

    /// 相对当前游标定位。
    pub fn relative(
        &mut self,
        connection: &mut DruidPooledConnection,
        rows: i32,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.relative(rows),
            |chain| chain.result_set_relative(self.physical.as_ref(), &self.filter_context, rows),
        );
        self.classify(connection, result)
    }

    /// 设置抓取方向。
    pub fn set_fetch_direction(
        &mut self,
        connection: &mut DruidPooledConnection,
        direction: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.set_fetch_direction(direction),
            |chain| {
                chain.result_set_set_fetch_direction(
                    self.physical.as_ref(),
                    &self.filter_context,
                    direction,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 返回抓取方向。
    pub fn fetch_direction(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.fetch_direction(),
            |chain| {
                chain.result_set_get_fetch_direction(self.physical.as_ref(), &self.filter_context)
            },
        );
        self.classify(connection, result)
    }

    /// 设置抓取大小。
    pub fn set_fetch_size(
        &mut self,
        connection: &mut DruidPooledConnection,
        rows: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.set_fetch_size(rows),
            |chain| {
                chain.result_set_set_fetch_size(self.physical.as_ref(), &self.filter_context, rows)
            },
        );
        self.classify(connection, result)
    }

    /// 返回抓取大小。
    pub fn fetch_size(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.fetch_size(),
            |chain| chain.result_set_get_fetch_size(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回结果集类型。
    pub fn result_set_type(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.result_set_type(),
            |chain| chain.result_set_get_type(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回并发模式。
    pub fn concurrency(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.concurrency(),
            |chain| chain.result_set_get_concurrency(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回保持性。
    pub fn holdability(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.holdability(),
            |chain| chain.result_set_get_holdability(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回结果集警告链。
    ///
    /// 对应 Java：`DruidPooledResultSet#getWarnings()`。
    pub fn warnings(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Option<SqlWarning>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.warnings(),
            |chain| chain.result_set_warnings(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 清除结果集警告链。
    ///
    /// 对应 Java：`DruidPooledResultSet#clearWarnings()`。
    pub fn clear_warnings(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.clear_warnings(),
            |chain| chain.result_set_clear_warnings(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回可空游标名称。
    ///
    /// 对应 Java：`DruidPooledResultSet#getCursorName()`。
    pub fn cursor_name(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Option<String>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.cursor_name(),
            |chain| chain.result_set_get_cursor_name(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回结果列 metadata。
    ///
    /// 对应 Java：`DruidPooledResultSet#getMetaData()`。
    pub fn meta_data(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<ResultSetMetaData, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.physical.meta_data();
        self.classify(connection, result)
    }

    /// 返回当前行是否被更新。
    pub fn row_updated(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.row_updated(),
            |chain| chain.result_set_row_updated(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回当前行是否为插入行。
    pub fn row_inserted(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.row_inserted(),
            |chain| chain.result_set_row_inserted(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回当前行是否被删除。
    pub fn row_deleted(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.row_deleted(),
            |chain| chain.result_set_row_deleted(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 提交插入行。
    pub fn insert_row(&mut self, connection: &mut DruidPooledConnection) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| result_set.insert_row())
    }

    /// 提交当前行更新。
    pub fn update_row(&mut self, connection: &mut DruidPooledConnection) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| result_set.update_row())
    }

    /// 删除当前行。
    pub fn delete_row(&mut self, connection: &mut DruidPooledConnection) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| result_set.delete_row())
    }

    /// 刷新当前行。
    pub fn refresh_row(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| result_set.refresh_row())
    }

    /// 取消当前行尚未提交的更新。
    pub fn cancel_row_updates(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| result_set.cancel_row_updates())
    }

    /// 移到插入行。
    pub fn move_to_insert_row(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| result_set.move_to_insert_row())
    }

    /// 从插入行返回当前行。
    pub fn move_to_current_row(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| result_set.move_to_current_row())
    }

    fn value(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.value(column_index),
            |chain| {
                chain.result_set_get_object(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    fn date_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        calendar: JdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.date(column_index, &calendar),
            |chain| match &calendar {
                JdbcCalendarArgument::Unspecified => chain.result_set_get_date(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                ),
                JdbcCalendarArgument::Specified(_) => chain.result_set_get_date_with_calendar(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    &calendar,
                ),
            },
        );
        self.classify(connection, result)
    }

    fn date_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        calendar: JdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.date_by_label(column_label, &calendar),
            |chain| match &calendar {
                JdbcCalendarArgument::Unspecified => chain.result_set_get_date_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                ),
                JdbcCalendarArgument::Specified(_) => chain
                    .result_set_get_date_by_label_with_calendar(
                        self.physical.as_ref(),
                        &self.filter_context,
                        column_label,
                        &calendar,
                    ),
            },
        );
        self.classify(connection, result)
    }

    fn time_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        calendar: JdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.time(column_index, &calendar),
            |chain| match &calendar {
                JdbcCalendarArgument::Unspecified => chain.result_set_get_time(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                ),
                JdbcCalendarArgument::Specified(_) => chain.result_set_get_time_with_calendar(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    &calendar,
                ),
            },
        );
        self.classify(connection, result)
    }

    fn time_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        calendar: JdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.time_by_label(column_label, &calendar),
            |chain| match &calendar {
                JdbcCalendarArgument::Unspecified => chain.result_set_get_time_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                ),
                JdbcCalendarArgument::Specified(_) => chain
                    .result_set_get_time_by_label_with_calendar(
                        self.physical.as_ref(),
                        &self.filter_context,
                        column_label,
                        &calendar,
                    ),
            },
        );
        self.classify(connection, result)
    }

    fn timestamp_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        calendar: JdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.timestamp(column_index, &calendar),
            |chain| match &calendar {
                JdbcCalendarArgument::Unspecified => chain.result_set_get_timestamp(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                ),
                JdbcCalendarArgument::Specified(_) => chain.result_set_get_timestamp_with_calendar(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    &calendar,
                ),
            },
        );
        self.classify(connection, result)
    }

    fn timestamp_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        calendar: JdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.timestamp_by_label(column_label, &calendar),
            |chain| match &calendar {
                JdbcCalendarArgument::Unspecified => chain.result_set_get_timestamp_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                ),
                JdbcCalendarArgument::Specified(_) => chain
                    .result_set_get_timestamp_by_label_with_calendar(
                        self.physical.as_ref(),
                        &self.filter_context,
                        column_label,
                        &calendar,
                    ),
            },
        );
        self.classify(connection, result)
    }

    fn update_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_value(column_index, &update)
        })
    }

    fn update_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_value_by_label(column_label, &update)
        })
    }

    fn update_blob_stream_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        stream: Option<&JdbcInputStream>,
        length: JdbcStreamLength,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_blob_stream(column_index, stream, length)
        })
    }

    fn update_blob_stream_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        stream: Option<&JdbcInputStream>,
        length: JdbcStreamLength,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_blob_stream_by_label(column_label, stream, length)
        })
    }

    fn update_clob_reader_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
        length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_clob_reader(column_index, reader, length)
        })
    }

    fn update_clob_reader_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
        length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_clob_reader_by_label(column_label, reader, length)
        })
    }

    fn update_n_clob_reader_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
        length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_n_clob_reader(column_index, reader, length)
        })
    }

    fn update_n_clob_reader_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
        length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.delegate_unit(connection, |result_set| {
            result_set.update_n_clob_reader_by_label(column_label, reader, length)
        })
    }

    fn delegate_unit(
        &mut self,
        connection: &mut DruidPooledConnection,
        operation: impl FnOnce(&dyn PhysicalResultSet) -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = operation(self.physical.as_ref());
        self.classify(connection, result)
    }

    fn ensure_same_lease(&self, connection: &DruidPooledConnection) -> Result<(), DruidError> {
        if connection.is_same_open_lease(&self.statement.inner.lease_active) {
            Ok(())
        } else {
            Err(DruidError::ConnectionDiscarded)
        }
    }

    fn ensure_open_for(&self, connection: &DruidPooledConnection) -> Result<(), DruidError> {
        self.ensure_same_lease(connection)?;
        if self.is_closed() || self.physical.is_closed() || self.statement.is_closed() {
            Err(DruidError::Other("result set is closed".to_string()))
        } else {
            Ok(())
        }
    }

    fn classify<T>(
        &mut self,
        connection: &mut DruidPooledConnection,
        result: Result<T, DruidError>,
    ) -> Result<T, DruidError> {
        let result = connection.classify_result(result);
        if result.is_err() {
            self.statement.record_result_set_exception();
        }
        result
    }
}

impl Wrapper for DruidPooledResultSet {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        iface.is_some_and(|iface| {
            iface == TypeId::of::<Self>() || iface == TypeId::of::<dyn PhysicalResultSet>()
        })
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if iface == TypeId::of::<Self>() {
            return Some(Unwrapped::Object(self));
        }
        (iface == TypeId::of::<dyn PhysicalResultSet>())
            .then(|| Unwrapped::ResultSet(self.physical.as_ref()))
    }
}

impl std::fmt::Debug for DruidPooledResultSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledResultSet")
            .field("physical", &self.physical)
            .field("closed", &self.is_closed())
            .field("cursor_index", &self.cursor_index.load(Ordering::Acquire))
            .field("fetch_row_count", &self.fetch_row_count())
            .finish()
    }
}
