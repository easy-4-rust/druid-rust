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
    ResultSetStatement, ResultSetUpdate, SqlWarning, Unwrapped, Value, Wrapper,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};

macro_rules! scalar_update_pair {
    ($index_method:ident, $label_method:ident, $value_type:ty, $variant:ident, $chain_index:ident, $chain_label:ident, $java_name:literal) => {
        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, ..)`。")]
        pub fn $index_method(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            value: $value_type,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_index(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    value,
                )
            } else {
                self.physical
                    .update_value(column_index, &ResultSetUpdate::$variant(value))
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, ..)`。")]
        pub fn $label_method(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            value: $value_type,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    value,
                )
            } else {
                self.physical
                    .update_value_by_label(column_label, &ResultSetUpdate::$variant(value))
            };
            self.classify(connection, result)
        }
    };
}

macro_rules! resource_update_pair {
    (
        $index_method:ident,
        $label_method:ident,
        $chain_index:ident,
        $chain_label:ident,
        $value_type:ty,
        $java_name:literal
    ) => {
        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, ..)`，保留 nullable 资源句柄。")]
        pub fn $index_method(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            value: Option<&$value_type>,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_index(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    value.cloned(),
                )
            } else {
                self.physical.$index_method(column_index, value)
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, ..)`，保留 nullable 资源句柄。")]
        pub fn $label_method(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            value: Option<&$value_type>,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    value.cloned(),
                )
            } else {
                self.physical.$label_method(column_label, value)
            };
            self.classify(connection, result)
        }
    };
}

macro_rules! input_stream_update_family {
    (
        $plain_index:ident, $plain_label:ident,
        $int_index:ident, $int_label:ident,
        $long_index:ident, $long_label:ident,
        $chain_plain_index:ident, $chain_plain_label:ident,
        $chain_int_index:ident, $chain_int_label:ident,
        $chain_long_index:ident, $chain_long_label:ident,
        $variant:ident, $java_name:literal
    ) => {
        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, InputStream)`。")]
        pub fn $plain_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            stream: Option<&JdbcInputStream>,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_plain_index(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    stream.cloned(),
                )
            } else {
                self.physical.update_value(
                    column_index,
                    &ResultSetUpdate::$variant {
                        stream: stream.cloned(),
                        length: JdbcStreamLength::Unspecified,
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, InputStream)`。")]
        pub fn $plain_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            stream: Option<&JdbcInputStream>,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_plain_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    stream.cloned(),
                )
            } else {
                self.physical.update_value_by_label(
                    column_label,
                    &ResultSetUpdate::$variant {
                        stream: stream.cloned(),
                        length: JdbcStreamLength::Unspecified,
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, InputStream, int)`。")]
        pub fn $int_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            stream: Option<&JdbcInputStream>,
            length: i32,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_int_index(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    stream.cloned(),
                    length,
                )
            } else {
                self.physical.update_value(
                    column_index,
                    &ResultSetUpdate::$variant {
                        stream: stream.cloned(),
                        length: JdbcStreamLength::Int(length),
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, InputStream, int)`。")]
        pub fn $int_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            stream: Option<&JdbcInputStream>,
            length: i32,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_int_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    stream.cloned(),
                    length,
                )
            } else {
                self.physical.update_value_by_label(
                    column_label,
                    &ResultSetUpdate::$variant {
                        stream: stream.cloned(),
                        length: JdbcStreamLength::Int(length),
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, InputStream, long)`。")]
        pub fn $long_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            stream: Option<&JdbcInputStream>,
            length: i64,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_long_index(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    stream.cloned(),
                    length,
                )
            } else {
                self.physical.update_value(
                    column_index,
                    &ResultSetUpdate::$variant {
                        stream: stream.cloned(),
                        length: JdbcStreamLength::Long(length),
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, InputStream, long)`。")]
        pub fn $long_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            stream: Option<&JdbcInputStream>,
            length: i64,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_long_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    stream.cloned(),
                    length,
                )
            } else {
                self.physical.update_value_by_label(
                    column_label,
                    &ResultSetUpdate::$variant {
                        stream: stream.cloned(),
                        length: JdbcStreamLength::Long(length),
                    },
                )
            };
            self.classify(connection, result)
        }
    };
}

macro_rules! reader_update_family {
    (
        $plain_index:ident, $plain_label:ident,
        $int_index:ident, $int_label:ident,
        $long_index:ident, $long_label:ident,
        $chain_plain_index:ident, $chain_plain_label:ident,
        $chain_int_index:ident, $chain_int_label:ident,
        $chain_long_index:ident, $chain_long_label:ident,
        $variant:ident, $java_name:literal
    ) => {
        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, Reader)`。")]
        pub fn $plain_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            reader: Option<&JdbcReader>,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_plain_index(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    reader.cloned(),
                )
            } else {
                self.physical.update_value(
                    column_index,
                    &ResultSetUpdate::$variant {
                        reader: reader.cloned(),
                        length: JdbcCharacterLength::Unspecified,
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, Reader)`。")]
        pub fn $plain_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            reader: Option<&JdbcReader>,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_plain_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    reader.cloned(),
                )
            } else {
                self.physical.update_value_by_label(
                    column_label,
                    &ResultSetUpdate::$variant {
                        reader: reader.cloned(),
                        length: JdbcCharacterLength::Unspecified,
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, Reader, int)`。")]
        pub fn $int_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            reader: Option<&JdbcReader>,
            length: i32,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_int_index(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    reader.cloned(),
                    length,
                )
            } else {
                self.physical.update_value(
                    column_index,
                    &ResultSetUpdate::$variant {
                        reader: reader.cloned(),
                        length: JdbcCharacterLength::Int(length),
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, Reader, int)`。")]
        pub fn $int_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            reader: Option<&JdbcReader>,
            length: i32,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_int_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    reader.cloned(),
                    length,
                )
            } else {
                self.physical.update_value_by_label(
                    column_label,
                    &ResultSetUpdate::$variant {
                        reader: reader.cloned(),
                        length: JdbcCharacterLength::Int(length),
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按下标执行 Java `ResultSet#", $java_name, "(int, Reader, long)`。")]
        pub fn $long_index(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_index: usize,
            reader: Option<&JdbcReader>,
            length: i64,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_long_index(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    reader.cloned(),
                    length,
                )
            } else {
                self.physical.update_value(
                    column_index,
                    &ResultSetUpdate::$variant {
                        reader: reader.cloned(),
                        length: JdbcCharacterLength::Long(length),
                    },
                )
            };
            self.classify(connection, result)
        }

        #[doc = concat!("按标签执行 Java `ResultSet#", $java_name, "(String, Reader, long)`。")]
        pub fn $long_label(
            &mut self,
            connection: &mut DruidPooledConnection,
            column_label: &str,
            reader: Option<&JdbcReader>,
            length: i64,
        ) -> Result<(), DruidError> {
            self.ensure_open_for(connection)?;
            let result = if let Some(chain) = self.filter_chain.as_ref() {
                chain.$chain_long_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    reader.cloned(),
                    length,
                )
            } else {
                self.physical.update_value_by_label(
                    column_label,
                    &ResultSetUpdate::$variant {
                        reader: reader.cloned(),
                        length: JdbcCharacterLength::Long(length),
                    },
                )
            };
            self.classify(connection, result)
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
    id: u64,
    attributes: super::ProxyAttributes,
    statement: DruidPooledStatement,
    prepared_statement: Option<DruidPooledPreparedStatementHandle>,
    callable_statement: Option<DruidPooledCallableStatementHandle>,
    physical: Arc<dyn PhysicalResultSet>,
    closed: Arc<AtomicBool>,
    cursor_index: AtomicI32,
    filter_chain: Option<Arc<FilterChain>>,
    filter_context: Arc<ResultSetFilterContext>,
    logic_column_map: Option<HashMap<i32, i32>>,
    physical_column_map: Option<HashMap<i32, i32>>,
    hidden_columns: Option<Vec<i32>>,
}

impl DruidPooledResultSet {
    pub(crate) fn new(
        statement: Arc<DruidPooledStatementInner>,
        physical: Arc<dyn PhysicalResultSet>,
    ) -> Result<Self, DruidError> {
        let filter_chain = statement.filter_chain.clone();
        let statement_handle = DruidPooledStatement::from_inner(Arc::clone(&statement));
        let result_set_id = statement.result_set_id_seed.fetch_add(1, Ordering::AcqRel);
        let filter_context = Arc::new(
            ResultSetFilterContext::with_identity_sql_and_execute_elapsed(
                statement.connection_id,
                statement.id,
                result_set_id,
                statement_handle.last_sql(),
                statement_handle.last_execute_elapsed(),
            ),
        );
        let mut result_set = Self {
            id: result_set_id,
            attributes: super::ProxyAttributes::default(),
            statement: DruidPooledStatement::from_inner(statement),
            prepared_statement: None,
            callable_statement: None,
            physical,
            closed: Arc::new(AtomicBool::new(false)),
            cursor_index: AtomicI32::new(0),
            filter_chain,
            filter_context,
            logic_column_map: None,
            physical_column_map: None,
            hidden_columns: None,
        };
        if let Some(filter_chain) = &result_set.filter_chain {
            let mut open_context = super::ResultSetOpenContext::new(
                &result_set.filter_context,
                result_set.physical.as_ref(),
            );
            filter_chain.result_set_open_after_with_proxy(&mut open_context)?;
            let (logic_column_map, physical_column_map, hidden_columns) =
                open_context.into_column_mappings();
            result_set.logic_column_map = logic_column_map;
            result_set.physical_column_map = physical_column_map;
            result_set.hidden_columns = hidden_columns;
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

    /// 返回 Druid 数据源分配的 ResultSet proxy ID。
    ///
    /// 对应 Java：`WrapperProxy#getId()`；每个数据源从 50000 开始递增。
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// 返回 ResultSet proxy attribute 数量。
    #[must_use]
    pub fn attributes_size(&self) -> usize {
        self.attributes.len()
    }

    /// 清空 ResultSet proxy attributes。
    pub fn clear_attributes(&self) {
        self.attributes.clear();
    }

    /// 返回 ResultSet proxy attributes 快照。
    #[must_use]
    pub fn attributes(&self) -> std::collections::HashMap<String, super::ProxyAttributeValue> {
        self.attributes.snapshot()
    }

    /// 返回指定 ResultSet proxy attribute。
    #[must_use]
    pub fn attribute(&self, key: &str) -> Option<super::ProxyAttributeValue> {
        self.attributes.get(key)
    }

    /// 保存或覆盖 ResultSet proxy attribute。
    pub fn put_attribute(
        &self,
        key: impl Into<String>,
        value: super::ProxyAttributeValue,
    ) -> Option<super::ProxyAttributeValue> {
        self.attributes.put(key, value)
    }

    /// 返回当前逻辑游标位置。
    ///
    /// 对应 Java：`ResultSetProxy#getCursorIndex()`。
    #[must_use]
    pub fn cursor_index(&self) -> i32 {
        self.cursor_index.load(Ordering::Acquire)
    }

    /// 返回成功完成的 ResultSet close Filter 链次数。
    #[must_use]
    pub fn close_count(&self) -> u64 {
        self.filter_context.close_count()
    }

    /// 返回从 ResultSet open Filter 时点到当前的耗时。
    ///
    /// Rust `Instant` 不伪造 Java `System.nanoTime()` 的绝对数值，只保留其可比较
    /// 的单调耗时语义。
    #[must_use]
    pub fn construct_elapsed(&self) -> Option<std::time::Duration> {
        self.filter_context.elapsed()
    }

    /// 返回产生本 ResultSet 的 SQL。
    #[must_use]
    pub fn sql(&self) -> Option<&str> {
        self.filter_context.sql()
    }

    /// 返回累计读取的 Java UTF-16 字符数。
    #[must_use]
    pub fn read_string_length(&self) -> u64 {
        self.filter_context.read_string_length()
    }

    /// 返回累计读取的字节数。
    #[must_use]
    pub fn read_bytes_length(&self) -> u64 {
        self.filter_context.read_bytes_length()
    }

    /// 返回打开 InputStream 的次数。
    #[must_use]
    pub fn open_input_stream_count(&self) -> u64 {
        self.filter_context.open_input_stream_count()
    }

    /// 返回打开 Reader 的次数。
    #[must_use]
    pub fn open_reader_count(&self) -> u64 {
        self.filter_context.open_reader_count()
    }

    /// 把逻辑列号映射为物理列号。
    ///
    /// Java map 存在但 key 缺失时会因拆箱抛出 NPE；Rust 用 `None` 显式表达该
    /// 非法映射。未设置 map 时返回原列号。
    #[must_use]
    pub fn physical_column(&self, logic_column: i32) -> Option<i32> {
        self.logic_column_map
            .as_ref()
            .map_or(Some(logic_column), |columns| {
                columns.get(&logic_column).copied()
            })
    }

    /// 把物理列号映射为逻辑列号；未设置 map 时返回原列号。
    #[must_use]
    pub fn logic_column(&self, physical_column: i32) -> Option<i32> {
        self.physical_column_map
            .as_ref()
            .map_or(Some(physical_column), |columns| {
                columns.get(&physical_column).copied()
            })
    }

    /// 返回隐藏列数量。
    #[must_use]
    pub fn hidden_column_count(&self) -> usize {
        self.hidden_columns.as_ref().map_or(0, Vec::len)
    }

    /// 返回隐藏列快照借用；尚未配置时保留 Java null 语义。
    #[must_use]
    pub fn hidden_columns(&self) -> Option<&[i32]> {
        self.hidden_columns.as_deref()
    }

    /// 替换逻辑列到物理列的映射；`None` 恢复恒等映射。
    pub fn set_logic_column_map(&mut self, logic_column_map: Option<HashMap<i32, i32>>) {
        self.logic_column_map = logic_column_map;
    }

    /// 替换物理列到逻辑列的映射；`None` 恢复恒等映射。
    pub fn set_physical_column_map(&mut self, physical_column_map: Option<HashMap<i32, i32>>) {
        self.physical_column_map = physical_column_map;
    }

    /// 替换隐藏列列表；`None` 保留 Java null。
    pub fn set_hidden_columns(&mut self, hidden_columns: Option<Vec<i32>>) {
        self.hidden_columns = hidden_columns;
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

    /// 通过 FilterChain 返回创建本结果集的动态 Statement 平台对象。
    ///
    /// 对应 Java `ResultSet#getStatement()`：普通、Prepared、Callable 三种运行时
    /// 身份均保留为共享句柄，Filter 可以继续调用、短路替换或返回驱动错误。
    pub fn statement_object(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<ResultSetStatement, DruidError> {
        self.ensure_open_for(connection)?;
        let statement = if let Some(callable) = &self.callable_statement {
            ResultSetStatement::Callable(callable.clone())
        } else if let Some(prepared) = &self.prepared_statement {
            ResultSetStatement::Prepared(prepared.clone())
        } else {
            ResultSetStatement::Statement(self.statement.clone())
        };
        let result = self.filter_chain.as_ref().map_or_else(
            || Ok(statement.clone()),
            |chain| {
                chain.result_set_get_statement(
                    self.physical.as_ref(),
                    &self.filter_context,
                    &statement,
                )
            },
        );
        self.classify(connection, result)
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
        let result = self.classify(connection, result);
        if let Ok(value) = result.as_ref() {
            self.record_object_lob_open(value);
        }
        result
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
        let result = self.classify(connection, result);
        if let Ok(value) = result.as_ref() {
            self.record_object_lob_open(value);
        }
        result
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
        let result = self.classify(connection, result);
        if let Ok(value) = result.as_ref() {
            self.record_object_lob_open(value);
        }
        result
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
        let result = self.classify(connection, result);
        if let Ok(value) = result.as_ref() {
            self.record_object_lob_open(value);
        }
        result
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
        let result = self.classify(connection, result);
        if let Ok(Some(value)) = result.as_ref() {
            self.filter_context.add_read_string_length(value);
        }
        result
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
        let result = self.classify(connection, result);
        if let Ok(Some(value)) = result.as_ref() {
            self.filter_context.add_read_string_length(value);
        }
        result
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
        let result = self.classify(connection, result);
        if let Ok(Some(value)) = result.as_ref() {
            self.filter_context.add_read_bytes_length(value.len());
        }
        result
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
        let result = self.classify(connection, result);
        if let Ok(Some(value)) = result.as_ref() {
            self.filter_context.add_read_bytes_length(value.len());
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.record_blob_open();
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.record_blob_open();
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.record_clob_open();
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.record_clob_open();
        }
        result
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
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || {
                self.physical
                    .update_value(column_index, &ResultSetUpdate::Null)
            },
            |chain| {
                chain.result_set_update_null(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                )
            },
        );
        self.classify(connection, result)
    }

    /// 按标签执行 Java `ResultSet#updateNull(String)`。
    pub fn update_null_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || {
                self.physical
                    .update_value_by_label(column_label, &ResultSetUpdate::Null)
            },
            |chain| {
                chain.result_set_update_null_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                )
            },
        );
        self.classify(connection, result)
    }

    scalar_update_pair!(
        update_boolean,
        update_boolean_by_label,
        bool,
        Boolean,
        result_set_update_boolean,
        result_set_update_boolean_by_label,
        "updateBoolean"
    );
    scalar_update_pair!(
        update_byte,
        update_byte_by_label,
        i8,
        Byte,
        result_set_update_byte,
        result_set_update_byte_by_label,
        "updateByte"
    );
    scalar_update_pair!(
        update_short,
        update_short_by_label,
        i16,
        Short,
        result_set_update_short,
        result_set_update_short_by_label,
        "updateShort"
    );
    scalar_update_pair!(
        update_int,
        update_int_by_label,
        i32,
        Int,
        result_set_update_int,
        result_set_update_int_by_label,
        "updateInt"
    );
    scalar_update_pair!(
        update_long,
        update_long_by_label,
        i64,
        Long,
        result_set_update_long,
        result_set_update_long_by_label,
        "updateLong"
    );
    scalar_update_pair!(
        update_float,
        update_float_by_label,
        f32,
        Float,
        result_set_update_float,
        result_set_update_float_by_label,
        "updateFloat"
    );
    scalar_update_pair!(
        update_double,
        update_double_by_label,
        f64,
        Double,
        result_set_update_double,
        result_set_update_double_by_label,
        "updateDouble"
    );
    scalar_update_pair!(
        update_big_decimal,
        update_big_decimal_by_label,
        Option<BigDecimal>,
        BigDecimal,
        result_set_update_big_decimal,
        result_set_update_big_decimal_by_label,
        "updateBigDecimal"
    );
    scalar_update_pair!(
        update_string,
        update_string_by_label,
        Option<String>,
        String,
        result_set_update_string,
        result_set_update_string_by_label,
        "updateString"
    );
    scalar_update_pair!(
        update_bytes,
        update_bytes_by_label,
        Option<Vec<u8>>,
        Bytes,
        result_set_update_bytes,
        result_set_update_bytes_by_label,
        "updateBytes"
    );
    scalar_update_pair!(
        update_date,
        update_date_by_label,
        Option<NaiveDate>,
        Date,
        result_set_update_date,
        result_set_update_date_by_label,
        "updateDate"
    );
    scalar_update_pair!(
        update_time,
        update_time_by_label,
        Option<NaiveTime>,
        Time,
        result_set_update_time,
        result_set_update_time_by_label,
        "updateTime"
    );
    scalar_update_pair!(
        update_timestamp,
        update_timestamp_by_label,
        Option<NaiveDateTime>,
        Timestamp,
        result_set_update_timestamp,
        result_set_update_timestamp_by_label,
        "updateTimestamp"
    );

    /// 按下标执行 Java `ResultSet#updateObject(int, Object)`。
    pub fn update_object(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: JdbcObject,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            chain.result_set_update_object(
                self.physical.as_ref(),
                &self.filter_context,
                column_index,
                value,
            )
        } else {
            self.physical
                .update_value(column_index, &ResultSetUpdate::Object(value))
        };
        self.classify(connection, result)
    }

    /// 按标签执行 Java `ResultSet#updateObject(String, Object)`。
    pub fn update_object_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: JdbcObject,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            chain.result_set_update_object_by_label(
                self.physical.as_ref(),
                &self.filter_context,
                column_label,
                value,
            )
        } else {
            self.physical
                .update_value_by_label(column_label, &ResultSetUpdate::Object(value))
        };
        self.classify(connection, result)
    }

    /// 按下标执行 Java `ResultSet#updateObject(int, Object, int)`。
    pub fn update_object_with_scale_or_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        value: JdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            chain.result_set_update_object_with_scale_or_length(
                self.physical.as_ref(),
                &self.filter_context,
                column_index,
                value,
                scale_or_length,
            )
        } else {
            self.physical.update_value(
                column_index,
                &ResultSetUpdate::ObjectWithScaleOrLength {
                    value,
                    scale_or_length,
                },
            )
        };
        self.classify(connection, result)
    }

    /// 按标签执行 Java `ResultSet#updateObject(String, Object, int)`。
    pub fn update_object_by_label_with_scale_or_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        value: JdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            chain.result_set_update_object_by_label_with_scale_or_length(
                self.physical.as_ref(),
                &self.filter_context,
                column_label,
                value,
                scale_or_length,
            )
        } else {
            self.physical.update_value_by_label(
                column_label,
                &ResultSetUpdate::ObjectWithScaleOrLength {
                    value,
                    scale_or_length,
                },
            )
        };
        self.classify(connection, result)
    }

    scalar_update_pair!(
        update_n_string,
        update_n_string_by_label,
        Option<String>,
        NString,
        result_set_update_n_string,
        result_set_update_n_string_by_label,
        "updateNString"
    );

    input_stream_update_family!(
        update_ascii_stream,
        update_ascii_stream_by_label,
        update_ascii_stream_with_int_length,
        update_ascii_stream_by_label_with_int_length,
        update_ascii_stream_with_length,
        update_ascii_stream_by_label_with_length,
        result_set_update_ascii_stream,
        result_set_update_ascii_stream_by_label,
        result_set_update_ascii_stream_with_int_length,
        result_set_update_ascii_stream_by_label_with_int_length,
        result_set_update_ascii_stream_with_length,
        result_set_update_ascii_stream_by_label_with_length,
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
        result_set_update_binary_stream,
        result_set_update_binary_stream_by_label,
        result_set_update_binary_stream_with_int_length,
        result_set_update_binary_stream_by_label_with_int_length,
        result_set_update_binary_stream_with_length,
        result_set_update_binary_stream_by_label_with_length,
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
        result_set_update_character_stream,
        result_set_update_character_stream_by_label,
        result_set_update_character_stream_with_int_length,
        result_set_update_character_stream_by_label_with_int_length,
        result_set_update_character_stream_with_length,
        result_set_update_character_stream_by_label_with_length,
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
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            chain.result_set_update_n_character_stream(
                self.physical.as_ref(),
                &self.filter_context,
                column_index,
                reader.cloned(),
            )
        } else {
            self.physical.update_value(
                column_index,
                &ResultSetUpdate::NCharacterStream {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Unspecified,
                },
            )
        };
        self.classify(connection, result)
    }

    /// 按标签执行 Java `ResultSet#updateNCharacterStream(String, Reader)`。
    pub fn update_n_character_stream_by_label(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            chain.result_set_update_n_character_stream_by_label(
                self.physical.as_ref(),
                &self.filter_context,
                column_label,
                reader.cloned(),
            )
        } else {
            self.physical.update_value_by_label(
                column_label,
                &ResultSetUpdate::NCharacterStream {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Unspecified,
                },
            )
        };
        self.classify(connection, result)
    }

    /// 按下标执行 Java `ResultSet#updateNCharacterStream(int, Reader, long)`。
    pub fn update_n_character_stream_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            chain.result_set_update_n_character_stream_with_length(
                self.physical.as_ref(),
                &self.filter_context,
                column_index,
                reader.cloned(),
                length,
            )
        } else {
            self.physical.update_value(
                column_index,
                &ResultSetUpdate::NCharacterStream {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Long(length),
                },
            )
        };
        self.classify(connection, result)
    }

    /// 按标签执行 Java `ResultSet#updateNCharacterStream(String, Reader, long)`。
    pub fn update_n_character_stream_by_label_with_length(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            chain.result_set_update_n_character_stream_by_label_with_length(
                self.physical.as_ref(),
                &self.filter_context,
                column_label,
                reader.cloned(),
                length,
            )
        } else {
            self.physical.update_value_by_label(
                column_label,
                &ResultSetUpdate::NCharacterStream {
                    reader: reader.cloned(),
                    length: JdbcCharacterLength::Long(length),
                },
            )
        };
        self.classify(connection, result)
    }

    resource_update_pair!(
        update_reference,
        update_reference_by_label,
        result_set_update_reference,
        result_set_update_reference_by_label,
        JdbcRef,
        "updateRef"
    );
    resource_update_pair!(
        update_blob,
        update_blob_by_label,
        result_set_update_blob,
        result_set_update_blob_by_label,
        JdbcBlob,
        "updateBlob"
    );
    resource_update_pair!(
        update_clob,
        update_clob_by_label,
        result_set_update_clob,
        result_set_update_clob_by_label,
        JdbcClob,
        "updateClob"
    );
    resource_update_pair!(
        update_array,
        update_array_by_label,
        result_set_update_array,
        result_set_update_array_by_label,
        JdbcArray,
        "updateArray"
    );
    resource_update_pair!(
        update_row_id,
        update_row_id_by_label,
        result_set_update_row_id,
        result_set_update_row_id_by_label,
        JdbcRowId,
        "updateRowId"
    );
    resource_update_pair!(
        update_n_clob,
        update_n_clob_by_label,
        result_set_update_n_clob,
        result_set_update_n_clob_by_label,
        JdbcNClob,
        "updateNClob"
    );
    resource_update_pair!(
        update_sql_xml,
        update_sql_xml_by_label,
        result_set_update_sql_xml,
        result_set_update_sql_xml_by_label,
        JdbcSqlXml,
        "updateSQLXML"
    );

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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.filter_context.increment_open_input_stream_count();
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.filter_context.increment_open_input_stream_count();
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.filter_context.increment_open_input_stream_count();
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.filter_context.increment_open_input_stream_count();
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.filter_context.increment_open_reader_count();
        }
        result
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
        let result = self.classify(connection, result);
        if result.as_ref().is_ok_and(Option::is_some) {
            self.filter_context.increment_open_reader_count();
        }
        result
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
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.meta_data(),
            |chain| chain.result_set_get_meta_data(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 返回带 Druid Proxy 身份的结果列 metadata。
    ///
    /// 对应 Java：`ResultSetMetaDataProxyImpl`。底层 21 个列描述方法仍由
    /// `ResultSetMetaData` 精确委托；代理层增加 metadata ID、所属 ResultSet
    /// ID、attributes 与 raw unwrap。
    pub fn meta_data_proxy(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<super::ResultSetMetaDataProxyImpl, DruidError> {
        let raw = self.meta_data(connection)?;
        let metadata_id = self
            .statement
            .inner
            .metadata_id_seed
            .fetch_add(1, Ordering::AcqRel);
        Ok(super::ResultSetMetaDataProxyImpl::new(
            raw,
            metadata_id,
            self.id,
        ))
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
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.insert_row(),
            |chain| chain.result_set_insert_row(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 提交当前行更新。
    pub fn update_row(&mut self, connection: &mut DruidPooledConnection) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.update_row(),
            |chain| chain.result_set_update_row(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 删除当前行。
    pub fn delete_row(&mut self, connection: &mut DruidPooledConnection) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.delete_row(),
            |chain| chain.result_set_delete_row(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 刷新当前行。
    pub fn refresh_row(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.refresh_row(),
            |chain| chain.result_set_refresh_row(self.physical.as_ref(), &self.filter_context),
        );
        self.classify(connection, result)
    }

    /// 取消当前行尚未提交的更新。
    pub fn cancel_row_updates(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.cancel_row_updates(),
            |chain| {
                chain.result_set_cancel_row_updates(self.physical.as_ref(), &self.filter_context)
            },
        );
        self.classify(connection, result)
    }

    /// 移到插入行。
    pub fn move_to_insert_row(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.move_to_insert_row(),
            |chain| {
                chain.result_set_move_to_insert_row(self.physical.as_ref(), &self.filter_context)
            },
        );
        self.classify(connection, result)
    }

    /// 从插入行返回当前行。
    pub fn move_to_current_row(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.filter_chain.as_ref().map_or_else(
            || self.physical.move_to_current_row(),
            |chain| {
                chain.result_set_move_to_current_row(self.physical.as_ref(), &self.filter_context)
            },
        );
        self.classify(connection, result)
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

    fn update_blob_stream_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        stream: Option<&JdbcInputStream>,
        length: JdbcStreamLength,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            match length {
                JdbcStreamLength::Unspecified => chain.result_set_update_blob_stream(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    stream.cloned(),
                ),
                JdbcStreamLength::Long(length) => chain.result_set_update_blob_stream_with_length(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    stream.cloned(),
                    length,
                ),
                JdbcStreamLength::Int(_) => unreachable!("Blob 不存在 int 长度重载"),
            }
        } else {
            self.physical
                .update_blob_stream(column_index, stream, length)
        };
        self.classify(connection, result)
    }

    fn update_blob_stream_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        stream: Option<&JdbcInputStream>,
        length: JdbcStreamLength,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            match length {
                JdbcStreamLength::Unspecified => chain.result_set_update_blob_stream_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    stream.cloned(),
                ),
                JdbcStreamLength::Long(length) => chain
                    .result_set_update_blob_stream_by_label_with_length(
                        self.physical.as_ref(),
                        &self.filter_context,
                        column_label,
                        stream.cloned(),
                        length,
                    ),
                JdbcStreamLength::Int(_) => unreachable!("Blob 不存在 int 长度重载"),
            }
        } else {
            self.physical
                .update_blob_stream_by_label(column_label, stream, length)
        };
        self.classify(connection, result)
    }

    fn update_clob_reader_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
        length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            match length {
                JdbcCharacterLength::Unspecified => chain.result_set_update_clob_reader(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    reader.cloned(),
                ),
                JdbcCharacterLength::Long(length) => chain
                    .result_set_update_clob_reader_with_length(
                        self.physical.as_ref(),
                        &self.filter_context,
                        column_index,
                        reader.cloned(),
                        length,
                    ),
                JdbcCharacterLength::Int(_) => unreachable!("Clob 不存在 int 长度重载"),
            }
        } else {
            self.physical
                .update_clob_reader(column_index, reader, length)
        };
        self.classify(connection, result)
    }

    fn update_clob_reader_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
        length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            match length {
                JdbcCharacterLength::Unspecified => chain.result_set_update_clob_reader_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    reader.cloned(),
                ),
                JdbcCharacterLength::Long(length) => chain
                    .result_set_update_clob_reader_by_label_with_length(
                        self.physical.as_ref(),
                        &self.filter_context,
                        column_label,
                        reader.cloned(),
                        length,
                    ),
                JdbcCharacterLength::Int(_) => unreachable!("Clob 不存在 int 长度重载"),
            }
        } else {
            self.physical
                .update_clob_reader_by_label(column_label, reader, length)
        };
        self.classify(connection, result)
    }

    fn update_n_clob_reader_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_index: usize,
        reader: Option<&JdbcReader>,
        length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            match length {
                JdbcCharacterLength::Unspecified => chain.result_set_update_n_clob_reader(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_index,
                    reader.cloned(),
                ),
                JdbcCharacterLength::Long(length) => chain
                    .result_set_update_n_clob_reader_with_length(
                        self.physical.as_ref(),
                        &self.filter_context,
                        column_index,
                        reader.cloned(),
                        length,
                    ),
                JdbcCharacterLength::Int(_) => unreachable!("NClob 不存在 int 长度重载"),
            }
        } else {
            self.physical
                .update_n_clob_reader(column_index, reader, length)
        };
        self.classify(connection, result)
    }

    fn update_n_clob_reader_by_label_argument(
        &mut self,
        connection: &mut DruidPooledConnection,
        column_label: &str,
        reader: Option<&JdbcReader>,
        length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = if let Some(chain) = self.filter_chain.as_ref() {
            match length {
                JdbcCharacterLength::Unspecified => chain.result_set_update_n_clob_reader_by_label(
                    self.physical.as_ref(),
                    &self.filter_context,
                    column_label,
                    reader.cloned(),
                ),
                JdbcCharacterLength::Long(length) => chain
                    .result_set_update_n_clob_reader_by_label_with_length(
                        self.physical.as_ref(),
                        &self.filter_context,
                        column_label,
                        reader.cloned(),
                        length,
                    ),
                JdbcCharacterLength::Int(_) => unreachable!("NClob 不存在 int 长度重载"),
            }
        } else {
            self.physical
                .update_n_clob_reader_by_label(column_label, reader, length)
        };
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
        // Java `DruidPooledStatement#checkException` 只为 Prepared/Callable
        // Statement 传入固定 SQL；普通 Statement 的 ResultSet 错误传 null。
        let sql = (self.prepared_statement.is_some() || self.callable_statement.is_some())
            .then(|| self.filter_context.sql())
            .flatten();
        let result = connection.classify_result_with_sql(result, sql);
        if result.is_err() {
            self.statement.record_result_set_exception();
        }
        result
    }

    fn record_blob_open(&self) {
        self.statement.record_blob_open();
    }

    fn record_object_lob_open(&self, value: &JdbcObject) {
        match value {
            JdbcObject::Blob(_) => self.record_blob_open(),
            JdbcObject::Clob(_) | JdbcObject::NClob(_) => self.record_clob_open(),
            _ => {}
        }
    }

    fn record_clob_open(&self) {
        self.statement.record_clob_open();
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
