use crate::error::{RdbcError, SqlWarning};
use crate::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot,
};
use crate::result_set::{
    default_object_as, default_read_object, ColumnIndex, ResultSet, ResultSetNavigation,
    ResultSetRead, ResultSetStreamKind, ResultSetUpdate,
};
use crate::statement::{FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType};
use crate::types::{RdbcInputStream, RdbcObject, RdbcReader, RdbcTargetType, Row, Value};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct BufferedRowSetState {
    closed: bool,
    cursor: i64,
    was_null: bool,
    fetch_direction: FetchDirection,
    fetch_size: i32,
    warnings: Option<SqlWarning>,
    row_updated: bool,
}

/// 基于 owned rows 的可滚动、默认只读 canonical buffered ResultSet。
///
/// 本类型取代旧 concrete identity，但不宣称实现 `javax.sql.RowSet` 的 property/event
/// contract。所有 column index 保持 1-based，重复 label 使用 first-match。
#[derive(Debug)]
pub struct BufferedRowSet {
    rows: Vec<Row>,
    column_labels: Vec<String>,
    meta_data: ResultSetMetaDataSnapshot,
    state: Mutex<BufferedRowSetState>,
}

impl BufferedRowSet {
    /// 从没有显式 labels 的 rows 创建 buffered cursor。
    pub fn new(rows: Vec<Row>) -> Self {
        Self::with_column_labels(rows, Vec::new())
    }

    /// 从 rows 和顺序 labels 创建 buffered cursor。
    pub fn with_column_labels(rows: Vec<Row>, column_labels: Vec<String>) -> Self {
        let width = rows
            .iter()
            .map(Row::len)
            .max()
            .unwrap_or_default()
            .max(column_labels.len());
        let columns = (0..width)
            .map(|offset| {
                let type_name = first_column_type_name(&rows, offset);
                let nullable = rows.iter().any(|row| {
                    row.get(offset)
                        .is_none_or(|value| std::matches!(value, Value::Null))
                });
                ColumnDescriptor::new(
                    column_labels.get(offset).cloned().unwrap_or_default(),
                    type_name,
                    if nullable {
                        ColumnNullability::Nullable
                    } else {
                        ColumnNullability::NoNulls
                    },
                )
            })
            .collect();
        Self {
            rows,
            column_labels,
            meta_data: ResultSetMetaDataSnapshot::new(columns),
            state: Mutex::new(BufferedRowSetState {
                closed: false,
                cursor: -1,
                was_null: false,
                fetch_direction: FetchDirection::Forward,
                fetch_size: 0,
                warnings: None,
                row_updated: false,
            }),
        }
    }

    // 所有缓冲标量转换只读取一次当前列，复用已有 typed object 转换规则。
    fn converted_scalar(
        &self,
        column: ColumnIndex,
        target: &RdbcTargetType,
    ) -> Result<Value, RdbcError> {
        match default_object_as(self, column, target)? {
            RdbcObject::Scalar(value) => Ok(value),
            RdbcObject::String(value) => Ok(Value::String(value)),
            RdbcObject::Boolean(value) => Ok(Value::Bool(value)),
            RdbcObject::Long(value) => Ok(Value::Int(value)),
            RdbcObject::Double(value) => Ok(Value::Float(value)),
            RdbcObject::Bytes(value) => Ok(Value::Bytes(value)),
            other => Err(RdbcError::DriverError(format!(
                "buffered scalar conversion returned unexpected object: {other}"
            ))),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, BufferedRowSetState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn ensure_open(state: &BufferedRowSetState) -> Result<(), RdbcError> {
        if state.closed {
            Err(RdbcError::Other("result set is closed".to_owned()))
        } else {
            Ok(())
        }
    }

    fn len(&self) -> i64 {
        i64::try_from(self.rows.len()).unwrap_or(i64::MAX)
    }

    fn has_current(&self, cursor: i64) -> bool {
        cursor >= 0 && cursor < self.len()
    }

    fn move_to(&self, state: &mut BufferedRowSetState, cursor: i64) -> bool {
        state.was_null = false;
        state.row_updated = false;
        if self.has_current(cursor) {
            state.cursor = cursor;
            true
        } else {
            state.cursor = if cursor < 0 { -1 } else { self.len() };
            false
        }
    }
}

crate::impl_standard_wrapper!(BufferedRowSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for BufferedRowSet {
    /// 返回已缓冲的完整行数；关闭后返回 None，读取不推进游标。
    /// 对应标准扩展: ResultSet#known_row_count。
    fn known_row_count(&self) -> Option<u64> {
        Self::ensure_open(&self.state()).ok()?;
        u64::try_from(self.rows.len()).ok()
    }

    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.navigate(ResultSetNavigation::Next).await
    }

    async fn navigate(&mut self, navigation: ResultSetNavigation) -> Result<bool, RdbcError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        let cursor = match navigation {
            ResultSetNavigation::Next => state.cursor.saturating_add(1),
            ResultSetNavigation::Previous if state.cursor >= self.len() => {
                self.len().saturating_sub(1)
            }
            ResultSetNavigation::Previous => state.cursor.saturating_sub(1),
            ResultSetNavigation::First => 0,
            ResultSetNavigation::Last => self.len().saturating_sub(1),
            ResultSetNavigation::BeforeFirst | ResultSetNavigation::Absolute(0) => -1,
            ResultSetNavigation::AfterLast => self.len(),
            ResultSetNavigation::Absolute(row) if row > 0 => i64::from(row) - 1,
            ResultSetNavigation::Absolute(row) => self.len().saturating_add(i64::from(row)),
            ResultSetNavigation::Relative(rows) => state.cursor.saturating_add(i64::from(rows)),
        };
        Ok(self.move_to(&mut state, cursor))
    }

    fn row(&self) -> Result<i32, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        if self.has_current(state.cursor) {
            i32::try_from(state.cursor + 1)
                .map_err(|_| RdbcError::InvalidArgument("row exceeds JDBC i32".to_owned()))
        } else {
            Ok(0)
        }
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        if !self.has_current(state.cursor) {
            return Err(RdbcError::InvalidArgument(
                "result set is not on a row".to_owned(),
            ));
        }
        let cursor = usize::try_from(state.cursor)
            .map_err(|_| RdbcError::InvalidArgument("invalid cursor position".to_owned()))?;
        let row = self
            .rows
            .get(cursor)
            .ok_or_else(|| RdbcError::InvalidArgument("result set is not on a row".to_owned()))?;
        let value = row.get(column.get() - 1).cloned().ok_or_else(|| {
            RdbcError::InvalidArgument(format!("column {} exceeds row width", column.get()))
        })?;
        state.was_null = std::matches!(value, Value::Null);
        Ok(value)
    }

    /// 按列读取缓冲结果的字符串表示，SQL NULL 保持 NULL，非法 UTF-8 返回驱动错误。
    /// 参数为 1-based 列下标；返回转换后的字符串值。
    /// 对应 Java: java.sql.ResultSet#getString；原 RowSetResultSet 标量合同。
    fn string_value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.converted_scalar(column, &RdbcTargetType::String)
    }

    /// 按原读取描述符转换缓冲标量，保留 NULL、整数范围和单次读取语义。
    /// 参数为 1-based 列和 getter 请求；返回标量或默认日期、资源读取结果。
    /// 对应 Java: java.sql.ResultSet#getLong/getDouble/getBytes 等。
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        let target = match request {
            ResultSetRead::Boolean => RdbcTargetType::Boolean,
            ResultSetRead::Long
            | ResultSetRead::Int
            | ResultSetRead::Short
            | ResultSetRead::Byte => RdbcTargetType::Long,
            ResultSetRead::Double | ResultSetRead::Float => RdbcTargetType::Double,
            ResultSetRead::Bytes => RdbcTargetType::Bytes,
            // String 通过 string_value；其余请求沿用唯一标准默认分派。
            _ => return default_read_object(self, column, request),
        };
        self.converted_scalar(column, &target)
            .map(RdbcObject::Scalar)
    }

    fn find_column(&self, column_label: &str) -> Result<ColumnIndex, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        self.column_labels
            .iter()
            .position(|label| label.eq_ignore_ascii_case(column_label))
            .and_then(|offset| ColumnIndex::new(offset + 1).ok())
            .ok_or_else(|| RdbcError::InvalidArgument(format!("unknown column: {column_label}")))
    }

    fn input_stream(
        &self,
        column: ColumnIndex,
        _kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.value(column).map(|value| match value {
            Value::Null => None,
            Value::Bytes(value) => Some(RdbcInputStream::from_bytes(value)),
            // SQL 字符串保持原始内容，不能使用附带 SQL 引号的 Value 展示格式。
            Value::String(value) => Some(RdbcInputStream::from_bytes(value.into_bytes())),
            other => Some(RdbcInputStream::from_bytes(other.to_string().into_bytes())),
        })
    }

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

    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        if !self.has_current(state.cursor) {
            return Err(RdbcError::InvalidArgument(
                "result set is not on a row".to_owned(),
            ));
        }
        let cursor = usize::try_from(state.cursor)
            .map_err(|_| RdbcError::InvalidArgument("invalid cursor position".to_owned()))?;
        let value = scalar_update(update)?;
        state.was_null = std::matches!(value, Value::Null);
        state.row_updated = true;
        drop(state);
        let row = self
            .rows
            .get_mut(cursor)
            .ok_or_else(|| RdbcError::InvalidArgument("result set row disappeared".to_owned()))?;
        let target = row.values.get_mut(column.get() - 1).ok_or_else(|| {
            RdbcError::InvalidArgument(format!("column {} exceeds row width", column.get()))
        })?;
        *target = value;
        Ok(())
    }

    /// 按列名更新缓冲行，先验证游标和列，再保留缓冲驱动的更新错误。
    /// 参数为原列名及更新描述符；不支持的资源更新返回含 0A000 的 SQL 异常。
    /// 对应 Java: java.sql.ResultSet#updateBlob/updateClob/updateNClob。
    fn update_value_by_label(
        &mut self,
        column_label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        // 缓冲驱动有实际更新能力，不能使用只读默认入口提前拒绝 LOB 流。
        let column = self.find_column(column_label)?;
        self.update_value(column, update)
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Self::ensure_open(&self.state())?;
        Ok(Arc::new(self.meta_data.clone()))
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.was_null)
    }

    fn is_before_first(&self) -> Result<bool, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(!self.rows.is_empty() && state.cursor < 0)
    }

    fn is_after_last(&self) -> Result<bool, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(!self.rows.is_empty() && state.cursor >= self.len())
    }

    fn is_first(&self) -> Result<bool, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(!self.rows.is_empty() && state.cursor == 0)
    }

    fn is_last(&self) -> Result<bool, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(!self.rows.is_empty() && state.cursor == self.len() - 1)
    }

    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        state.fetch_direction = direction;
        Ok(())
    }

    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.fetch_direction)
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        if rows < 0 {
            return Err(RdbcError::InvalidArgument(
                "fetch size must not be negative".to_owned(),
            ));
        }
        let mut state = self.state();
        Self::ensure_open(&state)?;
        state.fetch_size = rows;
        Ok(())
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.fetch_size)
    }

    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        Ok(ResultSetType::ScrollInsensitive)
    }

    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        Ok(ResultSetConcurrency::ReadOnly)
    }

    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Ok(ResultSetHoldability::HoldOverCommit)
    }

    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.warnings.clone())
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        state.warnings = None;
        Ok(())
    }

    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        Self::ensure_open(&self.state())?;
        Ok(None)
    }

    fn row_updated(&self) -> Result<bool, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.row_updated)
    }

    /// 返回缓冲行是否为插入行；关闭后保留原错误。
    /// 对应 Java: RowSetResultSet#rowInserted。
    fn row_inserted(&self) -> Result<bool, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(false)
    }

    /// 返回缓冲行是否已删除；关闭后保留原错误。
    /// 对应 Java: RowSetResultSet#rowDeleted。
    fn row_deleted(&self) -> Result<bool, RdbcError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(false)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.state().closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.state().closed)
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NULL",
        Value::Bool(_) => "BOOLEAN",
        Value::Int(_) => "BIGINT",
        Value::Float(_) => "DOUBLE",
        Value::Decimal(_) => "DECIMAL",
        Value::Date(_) => "DATE",
        Value::Time(_) => "TIME",
        Value::Timestamp(_) => "TIMESTAMP",
        Value::String(_) => "VARCHAR",
        Value::Bytes(_) => "VARBINARY",
    }
}

fn first_column_type_name(rows: &[Row], offset: usize) -> &'static str {
    for row in rows {
        if let Some(value) = row.get(offset) {
            if !std::matches!(value, Value::Null) {
                return value_type_name(value);
            }
        }
    }
    "NULL"
}

fn scalar_update(update: ResultSetUpdate) -> Result<Value, RdbcError> {
    match update {
        ResultSetUpdate::Boolean(value) => Ok(Value::Bool(value)),
        ResultSetUpdate::Byte(value) => Ok(Value::Int(i64::from(value))),
        ResultSetUpdate::Short(value) => Ok(Value::Int(i64::from(value))),
        ResultSetUpdate::Int(value) => Ok(Value::Int(i64::from(value))),
        ResultSetUpdate::Long(value) => Ok(Value::Int(value)),
        ResultSetUpdate::Float(value) => Ok(Value::Float(f64::from(value))),
        ResultSetUpdate::Double(value) => Ok(Value::Float(value)),
        ResultSetUpdate::BigDecimal(value) => Ok(value.map_or(Value::Null, Value::Decimal)),
        ResultSetUpdate::String(value) | ResultSetUpdate::NString(value) => {
            Ok(value.map_or(Value::Null, Value::String))
        }
        ResultSetUpdate::Bytes(value) => Ok(value.map_or(Value::Null, Value::Bytes)),
        ResultSetUpdate::Date(value) => Ok(value.map_or(Value::Null, Value::Date)),
        ResultSetUpdate::Time(value) => Ok(value.map_or(Value::Null, Value::Time)),
        ResultSetUpdate::Timestamp(value) => Ok(value.map_or(Value::Null, Value::Timestamp)),
        ResultSetUpdate::Object(crate::types::RdbcObject::Scalar(value)) => Ok(value),
        ResultSetUpdate::Null
        | ResultSetUpdate::Ref(None)
        | ResultSetUpdate::Blob(None)
        | ResultSetUpdate::Clob(None)
        | ResultSetUpdate::Array(None)
        | ResultSetUpdate::RowId(None)
        | ResultSetUpdate::NClob(None)
        | ResultSetUpdate::SqlXml(None) => Ok(Value::Null),
        ResultSetUpdate::Object(_)
        | ResultSetUpdate::Ref(Some(_))
        | ResultSetUpdate::Blob(Some(_))
        | ResultSetUpdate::Clob(Some(_))
        | ResultSetUpdate::Array(Some(_))
        | ResultSetUpdate::RowId(Some(_))
        | ResultSetUpdate::NClob(Some(_))
        | ResultSetUpdate::SqlXml(Some(_))
        | ResultSetUpdate::BlobStream { .. }
        | ResultSetUpdate::ClobReader { .. }
        | ResultSetUpdate::NClobReader { .. }
        | ResultSetUpdate::ObjectWithScaleOrLength { .. }
        | ResultSetUpdate::AsciiStream { .. }
        | ResultSetUpdate::BinaryStream { .. }
        | ResultSetUpdate::CharacterStream { .. }
        | ResultSetUpdate::NCharacterStream { .. } => Err(RdbcError::feature_not_supported(
            "buffered_row_set_resource_update",
        )),
    }
}
