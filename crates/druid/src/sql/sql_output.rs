use crate::core::Value;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// Output stream used to write user-defined type attributes back to a database.
///
/// Corresponds to Java: `java.sql.SQLOutput`. `SQLData#writeSQL` writes attributes in SQL type
/// declaration order. `None` maps to SQL `NULL`, never to an empty string or numeric zero.
#[derive(Default)]
pub struct SqlOutput {
    values: Vec<Value>,
}

impl SqlOutput {
    /// Creates an output stream with no attributes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Writes one mapped attribute in UDT declaration order.
    pub fn write_value(&mut self, value: Value) {
        self.values.push(value);
    }
    /// Writes SQL NULL.
    pub fn write_null(&mut self) {
        self.write_value(Value::Null);
    }
    /// Writes BOOLEAN.
    pub fn write_boolean(&mut self, value: bool) {
        self.write_value(Value::Bool(value));
    }
    /// Writes BIGINT.
    pub fn write_long(&mut self, value: i64) {
        self.write_value(Value::Int(value));
    }
    /// Writes TINYINT.
    pub fn write_byte(&mut self, value: i8) {
        self.write_long(i64::from(value));
    }
    /// Writes SMALLINT.
    pub fn write_short(&mut self, value: i16) {
        self.write_long(i64::from(value));
    }
    /// Writes INTEGER.
    pub fn write_int(&mut self, value: i32) {
        self.write_long(i64::from(value));
    }
    /// Writes FLOAT.
    pub fn write_float(&mut self, value: f32) {
        self.write_value(Value::Float(f64::from(value)));
    }
    /// Writes DOUBLE.
    pub fn write_double(&mut self, value: f64) {
        self.write_value(Value::Float(value));
    }
    /// Writes DECIMAL or NUMERIC; `None` maps to SQL NULL.
    pub fn write_big_decimal(&mut self, value: Option<BigDecimal>) {
        self.write_value(value.map_or(Value::Null, Value::Decimal));
    }
    /// Writes VARCHAR; `None` maps to SQL NULL.
    pub fn write_string(&mut self, value: Option<String>) {
        self.write_value(value.map_or(Value::Null, Value::String));
    }
    /// Writes VARBINARY; `None` maps to SQL NULL.
    pub fn write_bytes(&mut self, value: Option<Vec<u8>>) {
        self.write_value(value.map_or(Value::Null, Value::Bytes));
    }
    /// Writes SQL DATE.
    pub fn write_date(&mut self, value: Option<NaiveDate>) {
        self.write_value(value.map_or(Value::Null, Value::Date));
    }
    /// Writes SQL TIME.
    pub fn write_time(&mut self, value: Option<NaiveTime>) {
        self.write_value(value.map_or(Value::Null, Value::Time));
    }
    /// Writes SQL TIMESTAMP.
    pub fn write_timestamp(&mut self, value: Option<NaiveDateTime>) {
        self.write_value(value.map_or(Value::Null, Value::Timestamp));
    }
    /// Writes a general mapped object. Corresponds to Java: `writeObject`.
    pub fn write_object(&mut self, value: Value) {
        self.write_value(value);
    }
    /// Writes a DATALINK URL; `None` maps to SQL NULL.
    pub fn write_url(&mut self, value: Option<url::Url>) {
        self.write_string(value.map(|url| url.to_string()));
    }
    /// Consumes the stream and returns all attributes in SQL declaration order.
    #[must_use]
    pub fn into_values(self) -> Vec<Value> {
        self.values
    }
}
