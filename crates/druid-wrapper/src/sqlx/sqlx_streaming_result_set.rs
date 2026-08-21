//! `SQLx` 原生行物化 `ResultSet`。
//!
//! 当前实现保留 `SQLx` 原生 Row 到 `next() / value(...)`，避免提前转换为 Druid
//! `Vec<Row>`，但查询侧仍使用 `fetch_all`，因此不是流式游标。真正的流式结果集
//! 需要先把同步 `PhysicalResultSet::next()` 演进为可等待的 C7 生命周期接口。

use druid::core::{
    DruidError, PhysicalResultSet, ResultSetColumnMeta, ResultSetColumnType, ResultSetMetaData,
    Value,
};
use sqlx::mysql::MySqlRow;
use sqlx::postgres::PgRow;
use sqlx::sqlite::SqliteRow;
use sqlx::{any::AnyRow, Column, Row as SqlxRowTrait, TypeInfo, ValueRef};
use std::fmt;
use std::sync::Mutex;

#[derive(Debug)]
struct StreamingState {
    closed: bool,
    cursor: i64,
    was_null: bool,
}

enum StreamingRows {
    Any(Vec<AnyRow>),
    MySql(Vec<MySqlRow>),
    PostgreSql(Vec<PgRow>),
    Sqlite(Vec<SqliteRow>),
}

impl fmt::Debug for StreamingRows {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, len) = match self {
            StreamingRows::Any(rows) => ("any", rows.len()),
            StreamingRows::MySql(rows) => ("mysql", rows.len()),
            StreamingRows::PostgreSql(rows) => ("postgresql", rows.len()),
            StreamingRows::Sqlite(rows) => ("sqlite", rows.len()),
        };
        formatter
            .debug_struct("StreamingRows")
            .field("backend", &name)
            .field("row_count", &len)
            .finish()
    }
}

pub struct SqlxStreamingResultSet {
    rows: StreamingRows,
    column_labels: Vec<String>,
    meta_data: ResultSetMetaData,
    state: Mutex<StreamingState>,
}

impl fmt::Debug for SqlxStreamingResultSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqlxStreamingResultSet")
            .field("rows", &self.rows)
            .field("column_labels", &self.column_labels)
            .finish_non_exhaustive()
    }
}

impl SqlxStreamingResultSet {
    pub fn any(rows: Vec<AnyRow>, column_labels: Vec<String>) -> Self {
        let meta_data = Self::build_meta(&rows, &column_labels, |row, i| {
            let column = row.columns().get(i)?;
            let kind = column.type_info().kind();
            Some(match kind {
                sqlx::any::AnyTypeInfoKind::Null => ResultSetColumnType::Unknown,
                sqlx::any::AnyTypeInfoKind::Bool => ResultSetColumnType::Boolean,
                sqlx::any::AnyTypeInfoKind::SmallInt
                | sqlx::any::AnyTypeInfoKind::Integer
                | sqlx::any::AnyTypeInfoKind::BigInt => ResultSetColumnType::Integer,
                sqlx::any::AnyTypeInfoKind::Real | sqlx::any::AnyTypeInfoKind::Double => {
                    ResultSetColumnType::Float
                }
                sqlx::any::AnyTypeInfoKind::Text => ResultSetColumnType::Text,
                sqlx::any::AnyTypeInfoKind::Blob => ResultSetColumnType::Binary,
            })
        });
        Self {
            rows: StreamingRows::Any(rows),
            column_labels,
            meta_data,
            state: Mutex::new(StreamingState {
                closed: false,
                cursor: -1,
                was_null: false,
            }),
        }
    }

    pub fn mysql(rows: Vec<MySqlRow>, column_labels: Vec<String>) -> Self {
        let meta_data = Self::build_meta(&rows, &column_labels, |row, i| {
            let col = row.columns().get(i)?;
            let name = TypeInfo::name(col.type_info()).to_ascii_uppercase();
            Some(match_type_mysql(&name))
        });
        Self {
            rows: StreamingRows::MySql(rows),
            column_labels,
            meta_data,
            state: Mutex::new(StreamingState {
                closed: false,
                cursor: -1,
                was_null: false,
            }),
        }
    }

    pub fn postgresql(rows: Vec<PgRow>, column_labels: Vec<String>) -> Self {
        let meta_data = Self::build_meta(&rows, &column_labels, |row, i| {
            let col = row.columns().get(i)?;
            let name = TypeInfo::name(col.type_info()).to_ascii_uppercase();
            Some(match_type_postgresql(&name))
        });
        Self {
            rows: StreamingRows::PostgreSql(rows),
            column_labels,
            meta_data,
            state: Mutex::new(StreamingState {
                closed: false,
                cursor: -1,
                was_null: false,
            }),
        }
    }

    pub fn sqlite(rows: Vec<SqliteRow>, column_labels: Vec<String>) -> Self {
        let meta_data = Self::build_meta(&rows, &column_labels, |row, i| {
            let col = row.columns().get(i)?;
            let name = TypeInfo::name(col.type_info()).to_ascii_uppercase();
            Some(match_type_sqlite(&name))
        });
        Self {
            rows: StreamingRows::Sqlite(rows),
            column_labels,
            meta_data,
            state: Mutex::new(StreamingState {
                closed: false,
                cursor: -1,
                was_null: false,
            }),
        }
    }

    fn build_meta<R, F>(rows: &[R], labels: &[String], mut typer: F) -> ResultSetMetaData
    where
        R: SqlxRowTrait,
        F: FnMut(&R, usize) -> Option<ResultSetColumnType>,
    {
        let column_count = rows
            .first()
            .map_or(labels.len(), |row| row.columns().len())
            .max(labels.len());
        let columns = (0..column_count)
            .map(|column_index| {
                let label = labels.get(column_index).cloned().unwrap_or_default();
                let mut column_type = ResultSetColumnType::Unknown;
                for row in rows {
                    if let Some(ty) = typer(row, column_index) {
                        column_type = ty;
                        break;
                    }
                }
                let nullable = rows.is_empty();
                ResultSetColumnMeta::new(label, column_type, nullable)
            })
            .collect::<Vec<_>>();
        ResultSetMetaData::new(columns)
    }

    fn len(&self) -> i64 {
        match &self.rows {
            StreamingRows::Any(rows) => i64::try_from(rows.len()).unwrap_or(i64::MAX),
            StreamingRows::MySql(rows) => i64::try_from(rows.len()).unwrap_or(i64::MAX),
            StreamingRows::PostgreSql(rows) => i64::try_from(rows.len()).unwrap_or(i64::MAX),
            StreamingRows::Sqlite(rows) => i64::try_from(rows.len()).unwrap_or(i64::MAX),
        }
    }

    fn current_row(&self, index: usize) -> Option<ValueAt<'_>> {
        match &self.rows {
            StreamingRows::Any(rows) => rows.get(index).map(ValueAt::Any),
            StreamingRows::MySql(rows) => rows.get(index).map(ValueAt::MySql),
            StreamingRows::PostgreSql(rows) => rows.get(index).map(ValueAt::PostgreSql),
            StreamingRows::Sqlite(rows) => rows.get(index).map(ValueAt::Sqlite),
        }
    }
}

enum ValueAt<'a> {
    Any(&'a AnyRow),
    MySql(&'a MySqlRow),
    PostgreSql(&'a PgRow),
    Sqlite(&'a SqliteRow),
}

impl ValueAt<'_> {
    fn column_len(&self) -> usize {
        match self {
            ValueAt::Any(row) => row.columns().len(),
            ValueAt::MySql(row) => row.columns().len(),
            ValueAt::PostgreSql(row) => row.columns().len(),
            ValueAt::Sqlite(row) => row.columns().len(),
        }
    }

    fn value(&self, column_index: usize) -> Result<Value, DruidError> {
        match self {
            ValueAt::Any(row) => decode_any_value(row, column_index),
            ValueAt::MySql(row) => decode_mysql_value(row, column_index),
            ValueAt::PostgreSql(row) => decode_postgresql_value(row, column_index),
            ValueAt::Sqlite(row) => decode_sqlite_value(row, column_index),
        }
    }
}

fn decode_any_value(row: &AnyRow, index: usize) -> Result<Value, DruidError> {
    let raw = row.try_get_raw(index).map_err(sqlx_driver_error)?;
    if ValueRef::is_null(&raw) {
        return Ok(Value::Null);
    }
    let Some(column) = row.columns().get(index) else {
        return Err(DruidError::DriverError(format!(
            "column {index} is out of range"
        )));
    };
    Ok(match column.type_info().kind() {
        sqlx::any::AnyTypeInfoKind::Null => Value::Null,
        sqlx::any::AnyTypeInfoKind::Bool => {
            Value::Bool(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        sqlx::any::AnyTypeInfoKind::SmallInt => {
            let v: i16 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        sqlx::any::AnyTypeInfoKind::Integer => {
            let v: i32 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        sqlx::any::AnyTypeInfoKind::BigInt => {
            Value::Int(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        sqlx::any::AnyTypeInfoKind::Real => {
            let v: f32 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Float(f64::from(v))
        }
        sqlx::any::AnyTypeInfoKind::Double => {
            Value::Float(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        sqlx::any::AnyTypeInfoKind::Text => {
            Value::String(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        sqlx::any::AnyTypeInfoKind::Blob => {
            Value::Bytes(row.try_get(index).map_err(sqlx_driver_error)?)
        }
    })
}

fn decode_mysql_value(row: &MySqlRow, index: usize) -> Result<Value, DruidError> {
    let raw = row.try_get_raw(index).map_err(sqlx_driver_error)?;
    if ValueRef::is_null(&raw) {
        return Ok(Value::Null);
    }
    let Some(column) = row.columns().get(index) else {
        return Err(DruidError::DriverError(format!(
            "column {index} is out of range"
        )));
    };
    let name = TypeInfo::name(column.type_info()).to_ascii_uppercase();
    Ok(match name.as_str() {
        "BOOLEAN" | "BOOL" => Value::Bool(row.try_get(index).map_err(sqlx_driver_error)?),
        "TINYINT" => {
            let v: i8 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        "SMALLINT" => {
            let v: i16 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        "MEDIUMINT" | "INT" | "INTEGER" => {
            let v: i32 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        "BIGINT" => Value::Int(row.try_get(index).map_err(sqlx_driver_error)?),
        "FLOAT" => {
            let v: f32 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Float(f64::from(v))
        }
        "DOUBLE" | "REAL" => Value::Float(row.try_get(index).map_err(sqlx_driver_error)?),
        "DECIMAL" | "NUMERIC" | "NEWDECIMAL" => {
            Value::Decimal(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        "DATE" => Value::Date(row.try_get(index).map_err(sqlx_driver_error)?),
        "TIME" => Value::Time(row.try_get(index).map_err(sqlx_driver_error)?),
        "DATETIME" | "TIMESTAMP" => {
            Value::Timestamp(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        other if other.contains("BLOB") || other.contains("BINARY") => {
            Value::Bytes(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        _ => Value::String(row.try_get(index).map_err(sqlx_driver_error)?),
    })
}

fn decode_postgresql_value(row: &PgRow, index: usize) -> Result<Value, DruidError> {
    let raw = row.try_get_raw(index).map_err(sqlx_driver_error)?;
    if ValueRef::is_null(&raw) {
        return Ok(Value::Null);
    }
    let Some(column) = row.columns().get(index) else {
        return Err(DruidError::DriverError(format!(
            "column {index} is out of range"
        )));
    };
    let name = TypeInfo::name(column.type_info()).to_ascii_uppercase();
    Ok(match name.as_str() {
        "BOOL" | "BOOLEAN" => Value::Bool(row.try_get(index).map_err(sqlx_driver_error)?),
        "INT2" | "SMALLINT" => {
            let v: i16 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        "INT4" | "INTEGER" => {
            let v: i32 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        "INT8" | "BIGINT" => Value::Int(row.try_get(index).map_err(sqlx_driver_error)?),
        "FLOAT4" | "REAL" => {
            let v: f32 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Float(f64::from(v))
        }
        "FLOAT8" | "DOUBLE PRECISION" => {
            Value::Float(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        "NUMERIC" | "DECIMAL" => Value::Decimal(row.try_get(index).map_err(sqlx_driver_error)?),
        "DATE" => Value::Date(row.try_get(index).map_err(sqlx_driver_error)?),
        "TIME" => Value::Time(row.try_get(index).map_err(sqlx_driver_error)?),
        "TIMESTAMP" => Value::Timestamp(row.try_get(index).map_err(sqlx_driver_error)?),
        "BYTEA" => Value::Bytes(row.try_get(index).map_err(sqlx_driver_error)?),
        _ => Value::String(row.try_get(index).map_err(sqlx_driver_error)?),
    })
}

fn decode_sqlite_value(row: &SqliteRow, index: usize) -> Result<Value, DruidError> {
    let raw = row.try_get_raw(index).map_err(sqlx_driver_error)?;
    if ValueRef::is_null(&raw) {
        return Ok(Value::Null);
    }
    let Some(column) = row.columns().get(index) else {
        return Err(DruidError::DriverError(format!(
            "column {index} is out of range"
        )));
    };
    // SQLite expression columns such as `SELECT ? AS value` report a declared
    // type of NULL even when the current value is BLOB/TEXT/INTEGER. Preserve
    // the runtime storage class instead of turning a non-null value into NULL.
    let runtime_type = raw.type_info();
    let name = match TypeInfo::name(column.type_info()) {
        "NULL" => runtime_type.name(),
        declared => declared,
    }
    .to_ascii_uppercase();
    Ok(match name.as_str() {
        "BOOLEAN" | "BOOL" => Value::Bool(row.try_get(index).map_err(sqlx_driver_error)?),
        "TINYINT" => {
            let v: i8 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        "SMALLINT" => {
            let v: i16 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        "MEDIUMINT" | "INT" | "INTEGER" => {
            let v: i32 = row.try_get(index).map_err(sqlx_driver_error)?;
            Value::Int(i64::from(v))
        }
        "BIGINT" => Value::Int(row.try_get(index).map_err(sqlx_driver_error)?),
        "FLOAT" | "REAL" | "DOUBLE" | "DOUBLE PRECISION" => {
            Value::Float(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        "DECIMAL" | "NUMERIC" => {
            let v: String = row.try_get(index).map_err(sqlx_driver_error)?;
            match v.parse::<bigdecimal::BigDecimal>() {
                Ok(decimal) => Value::Decimal(decimal),
                Err(_) => Value::String(v),
            }
        }
        "DATE" => Value::Date(row.try_get(index).map_err(sqlx_driver_error)?),
        "TIME" => Value::Time(row.try_get(index).map_err(sqlx_driver_error)?),
        "DATETIME" | "TIMESTAMP" => {
            Value::Timestamp(row.try_get(index).map_err(sqlx_driver_error)?)
        }
        "BLOB" | "BINARY" | "BYTEA" => Value::Bytes(row.try_get(index).map_err(sqlx_driver_error)?),
        "NULL" => Value::Null,
        _ => Value::String(row.try_get(index).map_err(sqlx_driver_error)?),
    })
}

fn match_type_mysql(name: &str) -> ResultSetColumnType {
    match name {
        "BOOLEAN" | "BOOL" => ResultSetColumnType::Boolean,
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT" => {
            ResultSetColumnType::Integer
        }
        "FLOAT" | "DOUBLE" | "REAL" => ResultSetColumnType::Float,
        "DECIMAL" | "NUMERIC" | "NEWDECIMAL" => ResultSetColumnType::Decimal,
        "DATE" => ResultSetColumnType::Date,
        "TIME" => ResultSetColumnType::Time,
        "DATETIME" | "TIMESTAMP" => ResultSetColumnType::Timestamp,
        other if other.contains("BLOB") || other.contains("BINARY") => ResultSetColumnType::Binary,
        _ => ResultSetColumnType::Text,
    }
}

fn match_type_postgresql(name: &str) -> ResultSetColumnType {
    match name {
        "BOOL" | "BOOLEAN" => ResultSetColumnType::Boolean,
        "INT2" | "SMALLINT" | "INT4" | "INTEGER" | "INT8" | "BIGINT" => {
            ResultSetColumnType::Integer
        }
        "FLOAT4" | "REAL" | "FLOAT8" | "DOUBLE PRECISION" => ResultSetColumnType::Float,
        "NUMERIC" | "DECIMAL" => ResultSetColumnType::Decimal,
        "DATE" => ResultSetColumnType::Date,
        "TIME" => ResultSetColumnType::Time,
        "TIMESTAMP" => ResultSetColumnType::Timestamp,
        "BYTEA" => ResultSetColumnType::Binary,
        _ => ResultSetColumnType::Text,
    }
}

fn match_type_sqlite(name: &str) -> ResultSetColumnType {
    match name {
        "BOOLEAN" | "BOOL" => ResultSetColumnType::Boolean,
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT" => {
            ResultSetColumnType::Integer
        }
        "FLOAT" | "REAL" | "DOUBLE" | "DOUBLE PRECISION" => ResultSetColumnType::Float,
        "DECIMAL" | "NUMERIC" => ResultSetColumnType::Decimal,
        "DATE" => ResultSetColumnType::Date,
        "TIME" => ResultSetColumnType::Time,
        "DATETIME" | "TIMESTAMP" => ResultSetColumnType::Timestamp,
        "BLOB" | "BINARY" | "BYTEA" => ResultSetColumnType::Binary,
        "NULL" => ResultSetColumnType::Unknown,
        _ => ResultSetColumnType::Text,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn sqlx_driver_error(error: sqlx::Error) -> DruidError {
    DruidError::DriverError(error.to_string())
}

impl PhysicalResultSet for SqlxStreamingResultSet {
    fn close(&self) -> Result<(), DruidError> {
        self.state().closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.state().closed
    }

    fn next(&self) -> Result<bool, DruidError> {
        let mut state = self.state();
        if state.closed {
            return Err(DruidError::Other("result set is closed".to_string()));
        }
        let next = state.cursor.saturating_add(1);
        let len = self.len();
        let present = next < len;
        state.cursor = if present { next } else { len };
        state.was_null = false;
        Ok(present)
    }

    fn first(&self) -> Result<bool, DruidError> {
        let mut state = self.state();
        if state.closed {
            return Err(DruidError::Other("result set is closed".to_string()));
        }
        let present = self.len() > 0;
        state.cursor = if present { 0 } else { -1 };
        state.was_null = false;
        Ok(present)
    }

    fn last(&self) -> Result<bool, DruidError> {
        let mut state = self.state();
        if state.closed {
            return Err(DruidError::Other("result set is closed".to_string()));
        }
        let len = self.len();
        let present = len > 0;
        state.cursor = if present { len.saturating_sub(1) } else { len };
        state.was_null = false;
        Ok(present)
    }

    fn before_first(&self) -> Result<(), DruidError> {
        let mut state = self.state();
        if state.closed {
            return Err(DruidError::Other("result set is closed".to_string()));
        }
        state.cursor = -1;
        state.was_null = false;
        Ok(())
    }

    fn after_last(&self) -> Result<(), DruidError> {
        let mut state = self.state();
        if state.closed {
            return Err(DruidError::Other("result set is closed".to_string()));
        }
        state.cursor = self.len();
        state.was_null = false;
        Ok(())
    }

    fn absolute(&self, row: i32) -> Result<bool, DruidError> {
        let mut state = self.state();
        if state.closed {
            return Err(DruidError::Other("result set is closed".to_string()));
        }
        let len = self.len();
        let cursor = match row.cmp(&0) {
            std::cmp::Ordering::Greater => i64::from(row).saturating_sub(1),
            std::cmp::Ordering::Less => len.saturating_add(i64::from(row)),
            std::cmp::Ordering::Equal => -1,
        };
        let present = cursor >= 0 && cursor < len;
        state.cursor = if present {
            cursor
        } else if row > 0 {
            len
        } else {
            -1
        };
        state.was_null = false;
        Ok(present)
    }

    fn relative(&self, rows: i32) -> Result<bool, DruidError> {
        let mut state = self.state();
        if state.closed {
            return Err(DruidError::Other("result set is closed".to_string()));
        }
        let len = self.len();
        let next = state
            .cursor
            .checked_add(i64::from(rows))
            .unwrap_or(if rows < 0 { -1 } else { len });
        let present = next >= 0 && next < len;
        state.cursor = if next < 0 {
            -1
        } else if next >= len {
            len
        } else {
            next
        };
        state.was_null = false;
        Ok(present)
    }

    fn previous(&self) -> Result<bool, DruidError> {
        let mut state = self.state();
        if state.closed {
            return Err(DruidError::Other("result set is closed".to_string()));
        }
        let len = self.len();
        let previous = if state.cursor >= len {
            len.saturating_sub(1)
        } else {
            state.cursor.saturating_sub(1)
        };
        let present = previous >= 0 && previous < len;
        state.cursor = if present { previous } else { -1 };
        state.was_null = false;
        Ok(present)
    }

    fn row(&self) -> Result<i32, DruidError> {
        let state = self.state();
        if state.cursor < 0 || state.cursor >= self.len() {
            Ok(0)
        } else {
            i32::try_from(state.cursor.saturating_add(1))
                .map_err(|error| DruidError::DriverError(error.to_string()))
        }
    }

    fn was_null(&self) -> Result<bool, DruidError> {
        Ok(self.state().was_null)
    }

    fn find_column(&self, column_label: &str) -> Result<usize, DruidError> {
        self.column_labels
            .iter()
            .position(|label| label.eq_ignore_ascii_case(column_label))
            .map(|index| index + 1)
            .ok_or_else(|| {
                DruidError::DriverError(format!("column label {column_label:?} not found"))
            })
    }

    fn value(&self, column_index: usize) -> Result<Value, DruidError> {
        let zero_index = column_index.checked_sub(1).ok_or_else(|| {
            DruidError::DriverError(format!(
                "column_index {column_index} is not a valid 1-based index"
            ))
        })?;
        let current_index = {
            let state = self.state();
            if state.closed {
                return Err(DruidError::Other("result set is closed".to_string()));
            }
            if state.cursor < 0 || state.cursor >= self.len() {
                return Err(DruidError::DriverError(
                    "result set is not positioned on a row".to_string(),
                ));
            }
            usize::try_from(state.cursor)
                .map_err(|error| DruidError::DriverError(error.to_string()))?
        };
        let row = self.current_row(current_index).ok_or_else(|| {
            DruidError::DriverError("result set cursor is out of range".to_string())
        })?;
        if zero_index >= row.column_len() {
            return Err(DruidError::DriverError(format!(
                "column_index {column_index} is out of range"
            )));
        }
        let value = row.value(zero_index)?;
        if matches!(value, Value::Null) {
            self.state().was_null = true;
        }
        Ok(value)
    }

    fn meta_data(&self) -> Result<ResultSetMetaData, DruidError> {
        Ok(self.meta_data.clone())
    }
}

impl SqlxStreamingResultSet {
    fn state(&self) -> std::sync::MutexGuard<'_, StreamingState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
