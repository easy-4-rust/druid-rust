use crate::core::{DruidError, Value};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// Input stream for attributes of an SQL structured or `DISTINCT` type.
///
/// Corresponds to Java: `java.sql.SQLInput`. Drivers provide attributes in database declaration
/// order and `SQLData#readSQL` consumes them in the same order. Invalid conversion or exhaustion
/// returns an error. SQL NULL maps to `None` for objects or a primitive zero value, followed by
/// `was_null() == true`.
pub struct SqlInput {
    values: std::vec::IntoIter<Value>,
    was_null: bool,
}

impl SqlInput {
    /// Creates an input stream from `values` in database declaration order.
    #[must_use]
    pub fn new(values: Vec<Value>) -> Self {
        Self {
            values: values.into_iter(),
            was_null: false,
        }
    }
    /// Reads the next attribute and updates `was_null`.
    ///
    /// Returns `InvalidArgument` when no attributes remain.
    pub fn read_value(&mut self) -> Result<Value, DruidError> {
        let value = self.values.next().ok_or_else(|| {
            DruidError::InvalidArgument("SQLInput has no remaining attributes".to_owned())
        })?;
        self.was_null = matches!(value, Value::Null);
        Ok(value)
    }
    /// Returns whether the last attribute read was SQL `NULL`.
    ///
    /// Call after a getter. Corresponds to Java: `SQLInput#wasNull`.
    #[must_use]
    pub fn was_null(&self) -> bool {
        self.was_null
    }

    /// Reads BOOLEAN; SQL NULL returns `false` and is distinguished by `was_null`.
    pub fn read_boolean(&mut self) -> Result<bool, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(false),
            Value::Bool(value) => Ok(value),
            value => Err(Self::conversion_error("BOOLEAN", &value)),
        }
    }

    /// Reads BIGINT; SQL NULL returns zero.
    pub fn read_long(&mut self) -> Result<i64, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(0),
            Value::Int(value) => Ok(value),
            value => Err(Self::conversion_error("BIGINT", &value)),
        }
    }

    /// Reads TINYINT; SQL NULL returns zero.
    pub fn read_byte(&mut self) -> Result<i8, DruidError> {
        let value = self.read_long()?;
        i8::try_from(value).map_err(|_| Self::conversion_error("TINYINT", &Value::Int(value)))
    }

    /// Reads SMALLINT; SQL NULL returns zero.
    pub fn read_short(&mut self) -> Result<i16, DruidError> {
        let value = self.read_long()?;
        i16::try_from(value).map_err(|_| Self::conversion_error("SMALLINT", &Value::Int(value)))
    }

    /// Reads INTEGER; SQL NULL returns zero.
    pub fn read_int(&mut self) -> Result<i32, DruidError> {
        let value = self.read_long()?;
        i32::try_from(value).map_err(|_| Self::conversion_error("INTEGER", &Value::Int(value)))
    }

    /// Reads DOUBLE, applying the standard numeric conversion to integers.
    #[allow(clippy::cast_precision_loss)]
    pub fn read_double(&mut self) -> Result<f64, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(0.0),
            Value::Float(value) => Ok(value),
            Value::Int(value) => Ok(value as f64),
            value => Err(Self::conversion_error("DOUBLE", &value)),
        }
    }

    /// Reads FLOAT.
    #[allow(clippy::cast_possible_truncation)]
    pub fn read_float(&mut self) -> Result<f32, DruidError> {
        self.read_double().map(|value| value as f32)
    }

    /// Reads DECIMAL or NUMERIC.
    pub fn read_big_decimal(&mut self) -> Result<Option<BigDecimal>, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(None),
            Value::Decimal(value) => Ok(Some(value)),
            Value::Int(value) => Ok(Some(BigDecimal::from(value))),
            value => Err(Self::conversion_error("DECIMAL", &value)),
        }
    }

    /// Reads a string; SQL NULL returns `None`.
    pub fn read_string(&mut self) -> Result<Option<String>, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(None),
            Value::String(value) => Ok(Some(value)),
            value => Err(Self::conversion_error("VARCHAR", &value)),
        }
    }

    /// Reads bytes; SQL NULL returns `None`.
    pub fn read_bytes(&mut self) -> Result<Option<Vec<u8>>, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(None),
            Value::Bytes(value) => Ok(Some(value)),
            value => Err(Self::conversion_error("VARBINARY", &value)),
        }
    }

    /// Reads SQL DATE.
    pub fn read_date(&mut self) -> Result<Option<NaiveDate>, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(None),
            Value::Date(value) => Ok(Some(value)),
            value => Err(Self::conversion_error("DATE", &value)),
        }
    }

    /// Reads SQL TIME.
    pub fn read_time(&mut self) -> Result<Option<NaiveTime>, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(None),
            Value::Time(value) => Ok(Some(value)),
            value => Err(Self::conversion_error("TIME", &value)),
        }
    }

    /// Reads SQL TIMESTAMP.
    pub fn read_timestamp(&mut self) -> Result<Option<NaiveDateTime>, DruidError> {
        match self.read_value()? {
            Value::Null => Ok(None),
            Value::Timestamp(value) => Ok(Some(value)),
            value => Err(Self::conversion_error("TIMESTAMP", &value)),
        }
    }

    /// Reads a general mapped object. Corresponds to Java: `readObject`.
    pub fn read_object(&mut self) -> Result<Value, DruidError> {
        self.read_value()
    }

    /// Reads a DATALINK URL; SQL NULL returns `None`.
    pub fn read_url(&mut self) -> Result<Option<url::Url>, DruidError> {
        self.read_string()?
            .map(|value| {
                url::Url::parse(&value).map_err(|error| {
                    Self::conversion_error("DATALINK", &Value::String(format!("{value}: {error}")))
                })
            })
            .transpose()
    }

    fn conversion_error(target: &str, value: &Value) -> DruidError {
        DruidError::SqlException(Box::new(
            crate::core::SqlException::new(
                0,
                Some("22005".to_owned()),
                Some(format!("cannot convert {value:?} to {target}")),
            )
            .with_class_name("java.sql.SQLDataException")
            .with_assignable_type("java.sql.SQLNonTransientException"),
        ))
    }
}
