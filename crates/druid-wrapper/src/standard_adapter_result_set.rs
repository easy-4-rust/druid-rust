//! Wrapper-private canonical eager ResultSet for non-SQLx adapter cores.

use crate::standard_adapter_executor::StandardExecutor;
use crate::standard_query_result::StandardQueryResult;
use crate::standard_result_rows::StandardResultRows;
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlWarning;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{
    default_object_as, ColumnIndex, ResultSet, ResultSetRead, ResultSetStreamKind, ResultSetUpdate,
};
use druid_rdbc::types::{RdbcInputStream, RdbcObject, RdbcReader, RdbcTargetType, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub(crate) struct StandardAdapterResultSet {
    rows: StandardResultRows,
    cursor: usize,
    positioned: bool,
    before_first: bool,
    was_null: AtomicBool,
    closed: bool,
    lease: Option<Arc<dyn StandardExecutor>>,
    metadata: Arc<ResultSetMetaDataSnapshot>,
    column_labels: Vec<String>,
    warnings: Option<SqlWarning>,
    fetch_size: i32,
}

impl StandardAdapterResultSet {
    pub(crate) fn new(result: StandardQueryResult, lease: Arc<dyn StandardExecutor>) -> Self {
        Self {
            rows: StandardResultRows::from_rows(result.rows),
            cursor: 0,
            positioned: false,
            before_first: true,
            was_null: AtomicBool::new(false),
            closed: false,
            lease: Some(lease),
            metadata: Arc::new(ResultSetMetaDataSnapshot::new(result.columns)),
            column_labels: result.column_labels,
            warnings: None,
            fetch_size: 0,
        }
    }

    #[cfg(feature = "jdbc-agent")]
    pub(crate) fn new_spooled(
        labels: Vec<String>,
        rows: StandardResultRows,
        lease: Arc<dyn StandardExecutor>,
    ) -> Self {
        let mut result = StandardQueryResult::with_labels(Vec::new(), labels);
        result.rows = Vec::new();
        let mut result_set = Self::new(result, lease);
        result_set.rows = rows;
        result_set
    }

    fn ensure_open_core(&self) -> Result<(), RdbcError> {
        if self.closed
            || self
                .lease
                .as_ref()
                .is_some_and(|executor| executor.is_closed())
        {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }
    /// 保留原对象入口的定位错误；关闭或失效租约优先于未定位。
    fn ensure_object_positioned(&self) -> Result<(), RdbcError> {
        self.ensure_open_core()?;
        if !self.positioned {
            return Err(RdbcError::Other(
                "result set cursor is not positioned on a row".to_owned(),
            ));
        }
        Ok(())
    }

    /// 将逻辑一基位置夹到首行前或末行后；返回是否位于实际行。
    /// 对应 Java: ResultSet 的缓冲游标定位，边界不会重新执行查询。
    fn position_at(&mut self, position: i64) -> bool {
        let length = i64::try_from(self.rows.len()).unwrap_or(i64::MAX - 1);
        self.before_first = position <= 0;
        self.positioned = position > 0 && position <= length;
        self.cursor = if self.before_first {
            0
        } else if self.positioned {
            usize::try_from(position - 1).expect("有效位置来自已有 Vec 行数")
        } else {
            self.rows.len()
        };
        self.was_null.store(false, Ordering::Release);
        self.positioned
    }

    fn logical_position(&self) -> i64 {
        if self.before_first {
            0
        } else {
            i64::try_from(self.cursor).unwrap_or(i64::MAX - 1) + 1
        }
    }
}

druid_rdbc::impl_standard_wrapper!(StandardAdapterResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for StandardAdapterResultSet {
    fn known_row_count(&self) -> Option<u64> {
        self.ensure_open_core().ok()?;
        u64::try_from(self.rows.len()).ok()
    }

    async fn next(&mut self) -> Result<bool, RdbcError> {
        let present = self
            .navigate(druid_rdbc::result_set::ResultSetNavigation::Next)
            .await?;
        if present {
            if let Some(executor) = &self.lease {
                executor.task4_trace_result_next();
            }
        }
        Ok(present)
    }

    /// 在同一已缓冲结果中导航；参数为完整标准定位请求，返回实际行是否存在。
    /// 对应 Java: ResultSet#previous/first/last/absolute/relative。
    async fn navigate(
        &mut self,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
    ) -> Result<bool, RdbcError> {
        use druid_rdbc::result_set::ResultSetNavigation;
        self.ensure_open_core()?;
        let length = i64::try_from(self.rows.len()).unwrap_or(i64::MAX - 1);
        let position = match navigation {
            ResultSetNavigation::Next => self.logical_position().saturating_add(1),
            ResultSetNavigation::Previous => self.logical_position().saturating_sub(1),
            ResultSetNavigation::First => 1,
            ResultSetNavigation::Last => length,
            ResultSetNavigation::BeforeFirst => 0,
            ResultSetNavigation::AfterLast => length + 1,
            ResultSetNavigation::Absolute(row) if row < 0 => length + 1 + i64::from(row),
            ResultSetNavigation::Absolute(row) => i64::from(row),
            ResultSetNavigation::Relative(rows) => {
                self.logical_position().saturating_add(i64::from(rows))
            }
            _ => {
                return Err(RdbcError::UnsupportedOperation {
                    operation: "result_set_navigation",
                });
            }
        };
        Ok(self.position_at(position))
    }

    /// 返回当前一基行号；未定位时为零，关闭后保留原错误。
    /// 对应 Java: ResultSet#getRow。
    fn row(&self) -> Result<i32, RdbcError> {
        self.ensure_open_core()?;
        if !self.positioned {
            return Ok(0);
        }
        i32::try_from(self.cursor + 1)
            .map_err(|_| RdbcError::InvalidArgument("row number exceeds i32".to_owned()))
    }

    /// 返回非空结果是否位于首行前。对应 Java: ResultSet#isBeforeFirst。
    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(!self.rows.is_empty() && self.before_first)
    }

    /// 返回非空结果是否位于末行后。对应 Java: ResultSet#isAfterLast。
    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(!self.rows.is_empty() && !self.before_first && !self.positioned)
    }

    /// 返回当前是否为第一行。对应 Java: ResultSet#isFirst。
    fn is_first(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(self.positioned && self.cursor == 0)
    }

    /// 返回当前是否为最后一行。对应 Java: ResultSet#isLast。
    fn is_last(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(self.positioned && self.cursor + 1 == self.rows.len())
    }

    /// 缓冲结果没有命名游标；返回 None 或关闭错误。
    /// 对应 Java: ResultSet#getCursorName。
    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open_core()?;
        Ok(None)
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open_core()?;
        if !self.positioned {
            return Err(RdbcError::InvalidArgument(
                "result set has no current row".to_owned(),
            ));
        }
        let value = self
            .rows
            .value(self.cursor, column.get() - 1)?
            .ok_or_else(|| {
                RdbcError::InvalidArgument("column index exceeds row width".to_owned())
            })?;
        self.was_null
            .store(matches!(value, Value::Null), Ordering::Release);
        Ok(value)
    }

    /// 读取缓冲列为独立字节流，保留原字符串内容与SQL NULL。
    /// 参数为1-based列和ASCII/Unicode/Binary入口，返回流或原定位/关闭错误。
    /// 对应 Java: java.sql.ResultSet#getAsciiStream/getUnicodeStream/getBinaryStream。
    fn input_stream(
        &self,
        column: ColumnIndex,
        _kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.value(column).map(|value| match value {
            Value::Null => None,
            Value::Bytes(value) => Some(RdbcInputStream::from_bytes(value)),
            // 与原缓冲结果一致：字符串不经过带SQL引号的展示格式。
            Value::String(value) => Some(RdbcInputStream::from_bytes(value.into_bytes())),
            other => Some(RdbcInputStream::from_bytes(other.to_string().into_bytes())),
        })
    }

    /// 读取缓冲列为字符Reader，二进制值使用严格UTF-8转换。
    /// 参数为1-based列和national标志，返回Reader、SQL NULL或原状态/编码错误。
    /// 对应 Java: java.sql.ResultSet#getCharacterStream/getNCharacterStream。
    fn reader(
        &self,
        column: ColumnIndex,
        _national: bool,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(None),
            Value::String(value) => Ok(Some(RdbcReader::from_string(value))),
            Value::Bytes(value) => String::from_utf8(value)
                .map(RdbcReader::from_string)
                .map(Some)
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            other => Ok(Some(RdbcReader::from_string(other.to_string()))),
        }
    }

    /// 读取原缓冲对象；参数为列，返回无损标量或原对象游标错误。
    /// 对应 Java: java.sql.ResultSet#getObject(int)。
    fn object(&self, column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        self.ensure_object_positioned()?;
        self.value(column).map(RdbcObject::from)
    }

    /// 按目标类型读取对象；参数为列及目标，状态检查后复用唯一标准转换。
    /// 对应 Java: java.sql.ResultSet#getObject(int, Class)。
    fn object_as(
        &self,
        column: ColumnIndex,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.ensure_object_positioned()?;
        default_object_as(self, column, target_type)
    }

    /// 按实际列名读取对象；返回原标量、NULL或保留列名重载身份的资源错误。
    /// 参数为列名和读取请求，不改变缺失列、游标与关闭错误的优先级。
    /// 对应 Java: java.sql.ResultSet#getRef/getBlob/getClob/getArray/getURL/getRowId/getNClob/getSQLXML(String)。
    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        self.read_object(self.find_column(label)?, request)
            .map_err(|error| {
                // 唯一标准默认以列下标校验资源家族；这里只恢复实际列名入口的错误名。
                let operation = match (&error, request) {
                    (
                        RdbcError::UnsupportedOperation {
                            operation: "result_set_ref",
                        },
                        ResultSetRead::Ref,
                    ) => "result_set_ref_by_label",
                    (
                        RdbcError::UnsupportedOperation {
                            operation: "result_set_blob",
                        },
                        ResultSetRead::Blob,
                    ) => "result_set_blob_by_label",
                    (
                        RdbcError::UnsupportedOperation {
                            operation: "result_set_clob",
                        },
                        ResultSetRead::Clob,
                    ) => "result_set_clob_by_label",
                    (
                        RdbcError::UnsupportedOperation {
                            operation: "result_set_array",
                        },
                        ResultSetRead::Array,
                    ) => "result_set_array_by_label",
                    (
                        RdbcError::UnsupportedOperation {
                            operation: "result_set_url",
                        },
                        ResultSetRead::Url,
                    ) => "result_set_url_by_label",
                    (
                        RdbcError::UnsupportedOperation {
                            operation: "result_set_row_id",
                        },
                        ResultSetRead::RowId,
                    ) => "result_set_row_id_by_label",
                    (
                        RdbcError::UnsupportedOperation {
                            operation: "result_set_n_clob",
                        },
                        ResultSetRead::NClob,
                    ) => "result_set_n_clob_by_label",
                    (
                        RdbcError::UnsupportedOperation {
                            operation: "result_set_sql_xml",
                        },
                        ResultSetRead::SqlXml,
                    ) => "result_set_sql_xml_by_label",
                    _ => return error,
                };
                RdbcError::UnsupportedOperation { operation }
            })
    }

    /// 拒绝只读缓冲列更新；参数为下标和更新值，返回原重载的不支持错误。
    /// 对应 Java: java.sql.ResultSet#updateXxx(int, ...)；不消费调用者资源。
    fn update_value(
        &mut self,
        _column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        Err(buffered_update_rejection(&update, false))
    }

    /// 拒绝只读缓冲列名更新；参数为列名和更新值，返回原列名重载错误。
    /// 对应 Java: java.sql.ResultSet#updateXxx(String, ...)；不解析列或消费流。
    fn update_value_by_label(
        &mut self,
        _label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        Err(buffered_update_rejection(&update, true))
    }

    /// 按驱动实际标签查找首个匹配列；返回 1-based 下标或准确的缺失列错误。
    /// 对应 Java: ResultSet#findColumn；保留原缓冲游标的大小写无关、首匹配及关闭优先语义。
    fn find_column(&self, column_label: &str) -> Result<ColumnIndex, RdbcError> {
        self.ensure_open_core()?;
        self.column_labels
            .iter()
            .position(|label| label.eq_ignore_ascii_case(column_label))
            .and_then(|index| ColumnIndex::new(index + 1).ok())
            .ok_or_else(|| RdbcError::InvalidArgument(format!("unknown column: {column_label}")))
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.ensure_open_core()?;
        if let Some(executor) = &self.lease {
            executor.task4_trace_result_set_metadata();
        }
        Ok(self.metadata.clone())
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(self.was_null.load(Ordering::Acquire))
    }

    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.ensure_open_core()?;
        Ok(self.warnings.clone())
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open_core()?;
        self.warnings = None;
        Ok(())
    }

    /// 设置缓冲结果的抓取大小提示。对应 Java: ResultSet#setFetchSize。
    /// 参数为非负行数；返回设置结果，不重新执行或截断已缓冲的查询。
    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        if rows < 0 {
            return Err(RdbcError::InvalidArgument(
                "fetch_size must not be negative".to_owned(),
            ));
        }
        self.ensure_open_core()?;
        self.fetch_size = rows;
        Ok(())
    }

    /// 返回抓取大小提示；关闭后返回错误。对应 Java: ResultSet#getFetchSize。
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.ensure_open_core()?;
        Ok(self.fetch_size)
    }

    /// 查询当前只读缓冲行是否被更新；返回false，关闭或租约失效时返回原错误。
    /// 对应 Java: java.sql.ResultSet#rowUpdated。
    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(false)
    }

    /// 查询当前只读缓冲行是否被插入；返回false，关闭或租约失效时返回原错误。
    /// 对应 Java: java.sql.ResultSet#rowInserted。
    fn row_inserted(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(false)
    }

    /// 查询当前只读缓冲行是否被删除；返回false，关闭或租约失效时返回原错误。
    /// 对应 Java: java.sql.ResultSet#rowDeleted。
    fn row_deleted(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(false)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        self.lease.take();
        self.rows = StandardResultRows::from_rows(Vec::new());
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

/// 保留只读缓冲资源更新的重载身份，标量更新沿用唯一标准的通用拒绝。
/// 参数为更新描述符和列名标志；返回错误，不访问或关闭调用者资源。
fn buffered_update_rejection(update: &ResultSetUpdate, by_label: bool) -> RdbcError {
    let operation = match (update, by_label) {
        (ResultSetUpdate::Ref(_), false) => "result_set_update_ref",
        (ResultSetUpdate::Ref(_), true) => "result_set_update_ref_by_label",
        (ResultSetUpdate::Blob(_), false) => "result_set_update_blob",
        (ResultSetUpdate::Blob(_), true) => "result_set_update_blob_by_label",
        (ResultSetUpdate::Clob(_), false) => "result_set_update_clob",
        (ResultSetUpdate::Clob(_), true) => "result_set_update_clob_by_label",
        (ResultSetUpdate::Array(_), false) => "result_set_update_array",
        (ResultSetUpdate::Array(_), true) => "result_set_update_array_by_label",
        (ResultSetUpdate::RowId(_), false) => "result_set_update_row_id",
        (ResultSetUpdate::RowId(_), true) => "result_set_update_row_id_by_label",
        (ResultSetUpdate::NClob(_), false) => "result_set_update_n_clob",
        (ResultSetUpdate::NClob(_), true) => "result_set_update_n_clob_by_label",
        (ResultSetUpdate::SqlXml(_), false) => "result_set_update_sql_xml",
        (ResultSetUpdate::SqlXml(_), true) => "result_set_update_sql_xml_by_label",
        (ResultSetUpdate::BlobStream { .. }, false) => "result_set_update_blob_stream",
        (ResultSetUpdate::BlobStream { .. }, true) => "result_set_update_blob_stream_by_label",
        (ResultSetUpdate::ClobReader { .. }, false) => "result_set_update_clob_reader",
        (ResultSetUpdate::ClobReader { .. }, true) => "result_set_update_clob_reader_by_label",
        (ResultSetUpdate::NClobReader { .. }, false) => "result_set_update_n_clob_reader",
        (ResultSetUpdate::NClobReader { .. }, true) => "result_set_update_n_clob_reader_by_label",
        (_, false) => "result_set_update_value",
        (_, true) => "result_set_update_value_by_label",
    };
    RdbcError::UnsupportedOperation { operation }
}
