use rusqlite::{
    Row,
    types::{ToSql, ToSqlOutput, Value as SqlValue, ValueRef},
};
use std::error::Error as StdError;
use std::fmt;
use std::str::FromStr;
use toasty_core::stmt::{self, Value as CoreValue};

#[derive(Debug)]
struct SqliteValueConversionError(String);

impl fmt::Display for SqliteValueConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for SqliteValueConversionError {}

fn conversion_error(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteValueConversionError(message.into())))
}

#[derive(Debug)]
pub(crate) struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl Value {
    /// Converts this SQLite driver value into the core Toasty value.
    pub(crate) fn into_inner(self) -> CoreValue {
        self.0
    }

    /// Converts a SQLite value within a row to a Toasty value.
    pub(crate) fn from_sql(
        row: &Row,
        index: usize,
        ty: &stmt::Type,
    ) -> rusqlite::Result<Self> {
        let value: Option<SqlValue> = row.get(index)?;

        let core_value = match value {
            Some(SqlValue::Null) | None => stmt::Value::Null,
            Some(SqlValue::Integer(value)) => match ty {
                stmt::Type::Bool => stmt::Value::Bool(value != 0),
                stmt::Type::I8 => stmt::Value::I8(value as i8),
                stmt::Type::I16 => stmt::Value::I16(value as i16),
                stmt::Type::I32 => stmt::Value::I32(value as i32),
                stmt::Type::I64 => stmt::Value::I64(value),
                stmt::Type::U8 => stmt::Value::U8(value as u8),
                stmt::Type::U16 => stmt::Value::U16(value as u16),
                stmt::Type::U32 => stmt::Value::U32(value as u32),
                stmt::Type::U64 => stmt::Value::U64(value as u64),
                _ => {
                    return Err(conversion_error(format!(
                        "SQLite INTEGER cannot decode as {ty:#?}"
                    )));
                }
            },
            Some(SqlValue::Real(value)) => match ty {
                stmt::Type::F32 => stmt::Value::F32(value as f32),
                stmt::Type::F64 => stmt::Value::F64(value),
                _ => {
                    return Err(conversion_error(format!(
                        "SQLite REAL cannot decode as {ty:#?}"
                    )));
                }
            },
            Some(SqlValue::Text(value)) => match ty {
                stmt::Type::Uuid => stmt::Value::Uuid(value.parse().map_err(|error| {
                    conversion_error(format!("invalid SQLite UUID result: {error}"))
                })?),
                stmt::Type::List(elem) => json_text_to_value_list(&value, elem)?,
                // A bare `#[document]` column (`Type::Object`) decodes
                // shape-directed to the named `Value::Object` wire form; the
                // engine raises it to the embed's positional record.
                stmt::Type::Object => json_text_to_value(&value, ty)?,
                #[cfg(feature = "bigdecimal")]
                stmt::Type::BigDecimal => stmt::Value::BigDecimal(
                    bigdecimal::BigDecimal::from_str(&value).map_err(|error| {
                        conversion_error(format!("invalid SQLite BigDecimal result: {error}"))
                    })?,
                ),
                #[cfg(feature = "jiff")]
                stmt::Type::Date => stmt::Value::Date(
                    value.parse().map_err(|error| {
                        conversion_error(format!("invalid SQLite Date result: {error}"))
                    })?,
                ),
                #[cfg(feature = "jiff")]
                stmt::Type::Time => stmt::Value::Time(
                    value.parse().map_err(|error| {
                        conversion_error(format!("invalid SQLite Time result: {error}"))
                    })?,
                ),
                #[cfg(feature = "jiff")]
                stmt::Type::DateTime => stmt::Value::DateTime(
                    value.replace(' ', "T").parse().map_err(|error| {
                        conversion_error(format!("invalid SQLite DateTime result: {error}"))
                    })?,
                ),
                #[cfg(feature = "jiff")]
                stmt::Type::Timestamp => stmt::Value::Timestamp(
                    value.parse().map_err(|error| {
                        conversion_error(format!("invalid SQLite Timestamp result: {error}"))
                    })?,
                ),
                _ => stmt::Value::String(value),
            },
            Some(SqlValue::Blob(value)) => match ty {
                stmt::Type::Bytes => stmt::Value::Bytes(value),
                _ => {
                    return Err(conversion_error(format!(
                        "SQLite BLOB cannot decode as {ty:#?}"
                    )));
                }
            },
        };

        Ok(Value(core_value))
    }

    /// Converts a SQLite value within a row using SQLite's runtime storage class.
    pub(crate) fn from_sql_infer(
        row: &Row,
        index: usize,
        declared_type: Option<&str>,
    ) -> rusqlite::Result<Self> {
        let value: Option<SqlValue> = row.get(index)?;

        #[cfg(feature = "bigdecimal")]
        if declared_type.is_some_and(is_decimal_type) {
            let value = match value {
                Some(SqlValue::Integer(value)) => bigdecimal::BigDecimal::from(value),
                Some(SqlValue::Real(value)) => {
                    bigdecimal::BigDecimal::from_str(&value.to_string()).map_err(|error| {
                        conversion_error(format!("invalid SQLite decimal result: {error}"))
                    })?
                }
                Some(SqlValue::Text(value)) => {
                    bigdecimal::BigDecimal::from_str(&value).map_err(|error| {
                        conversion_error(format!("invalid SQLite decimal result: {error}"))
                    })?
                }
                Some(SqlValue::Null) | None => return Ok(Value(stmt::Value::Null)),
                Some(actual) => {
                    return Err(conversion_error(format!(
                        "SQLite decimal result used incompatible storage value {actual:?}"
                    )));
                }
            };
            return Ok(Value(stmt::Value::BigDecimal(value)));
        }

        #[cfg(feature = "jiff")]
        if let Some(declared_type) = declared_type {
            let declared_type = declared_type.to_ascii_uppercase();
            if is_date_type(&declared_type) {
                return parse_jiff_text(value, "Date", |value| {
                    value.parse::<jiff::civil::Date>().map(stmt::Value::Date)
                });
            }
            if is_time_type(&declared_type) {
                return parse_jiff_text(value, "Time", |value| {
                    value.parse::<jiff::civil::Time>().map(stmt::Value::Time)
                });
            }
            if is_datetime_type(&declared_type) {
                return parse_jiff_text(value, "DateTime", |value| {
                    value
                        .replace(' ', "T")
                        .parse::<jiff::civil::DateTime>()
                        .map(stmt::Value::DateTime)
                });
            }
        }

        let core_value = match value {
            Some(SqlValue::Null) | None => stmt::Value::Null,
            Some(SqlValue::Integer(value)) => stmt::Value::I64(value),
            Some(SqlValue::Real(value)) => stmt::Value::F64(value),
            Some(SqlValue::Text(value)) => stmt::Value::String(value),
            Some(SqlValue::Blob(value)) => stmt::Value::Bytes(value),
        };

        Ok(Value(core_value))
    }
}

impl ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        use stmt::Value;

        match &self.0 {
            Value::Bool(true) => Ok(ToSqlOutput::Owned(SqlValue::Integer(1))),
            Value::Bool(false) => Ok(ToSqlOutput::Owned(SqlValue::Integer(0))),
            Value::I8(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::I16(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::I32(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::I64(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v))),
            Value::U8(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::U16(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::U32(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::U64(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::F32(v) => Ok(ToSqlOutput::Owned(SqlValue::Real(*v as f64))),
            Value::F64(v) => Ok(ToSqlOutput::Owned(SqlValue::Real(*v))),
            #[cfg(feature = "bigdecimal")]
            Value::BigDecimal(v) => Ok(ToSqlOutput::Owned(SqlValue::Text(v.to_string()))),
            #[cfg(feature = "jiff")]
            Value::Timestamp(v) => Ok(ToSqlOutput::Owned(SqlValue::Text(format!("{v:.9}")))),
            #[cfg(feature = "jiff")]
            Value::Zoned(v) => Ok(ToSqlOutput::Owned(SqlValue::Text(v.to_string()))),
            #[cfg(feature = "jiff")]
            Value::Date(v) => Ok(ToSqlOutput::Owned(SqlValue::Text(v.to_string()))),
            #[cfg(feature = "jiff")]
            Value::Time(v) => Ok(ToSqlOutput::Owned(SqlValue::Text(format!("{v:.9}")))),
            #[cfg(feature = "jiff")]
            Value::DateTime(v) => Ok(ToSqlOutput::Owned(SqlValue::Text(format!("{v:.9}")))),
            Value::String(v) => Ok(ToSqlOutput::Borrowed(ValueRef::Text(v.as_bytes()))),
            Value::Bytes(v) => Ok(ToSqlOutput::Borrowed(ValueRef::Blob(&v[..]))),
            Value::Null => Ok(ToSqlOutput::Owned(SqlValue::Null)),
            // A `Vec<scalar>` / document collection (`List`) or a bare
            // `#[document]` embed (`Object`) is stored as JSON text.
            Value::List(_) | Value::Object(_) => Ok(ToSqlOutput::Owned(SqlValue::Text(
                value_to_json_text(&self.0)?,
            ))),
            unsupported => Err(conversion_error(format!(
                "Toasty value cannot be bound to SQLite: {unsupported:#?}"
            ))),
        }
    }
}

#[cfg(feature = "bigdecimal")]
fn is_decimal_type(declared_type: &str) -> bool {
    let declared_type = declared_type.to_ascii_uppercase();
    declared_type.contains("DECIMAL") || declared_type.contains("NUMERIC")
}

#[cfg(feature = "jiff")]
fn is_date_type(declared_type: &str) -> bool {
    declared_type.trim() == "DATE"
}

#[cfg(feature = "jiff")]
fn is_time_type(declared_type: &str) -> bool {
    let declared_type = declared_type.trim();
    declared_type == "TIME" || declared_type.starts_with("TIME(")
}

#[cfg(feature = "jiff")]
fn is_datetime_type(declared_type: &str) -> bool {
    declared_type.contains("DATETIME") || declared_type.contains("TIMESTAMP")
}

#[cfg(feature = "jiff")]
fn parse_jiff_text<T, E>(
    value: Option<SqlValue>,
    type_name: &str,
    parse: impl FnOnce(&str) -> Result<T, E>,
) -> rusqlite::Result<Value>
where
    T: Into<stmt::Value>,
    E: fmt::Display,
{
    match value {
        Some(SqlValue::Text(value)) => parse(&value)
            .map(Into::into)
            .map(Value)
            .map_err(|error| {
                conversion_error(format!("invalid SQLite {type_name} result: {error}"))
            }),
        Some(SqlValue::Null) | None => Ok(Value(stmt::Value::Null)),
        Some(actual) => Err(conversion_error(format!(
            "SQLite {type_name} result used incompatible storage value {actual:?}"
        ))),
    }
}

fn value_to_json_text(value: &CoreValue) -> rusqlite::Result<String> {
    toasty_sql::json::to_string(value)
        .map_err(|error| conversion_error(format!("cannot encode SQLite JSON value: {error}")))
}

fn json_text_to_value_list(
    text: &str,
    elem_ty: &stmt::Type,
) -> rusqlite::Result<CoreValue> {
    toasty_sql::json::list_from_str(text, elem_ty)
        .map_err(|error| conversion_error(format!("invalid SQLite collection JSON: {error}")))
}

fn json_text_to_value(text: &str, ty: &stmt::Type) -> rusqlite::Result<CoreValue> {
    toasty_sql::json::from_str(text, ty)
        .map_err(|error| conversion_error(format!("invalid SQLite document JSON: {error}")))
}
