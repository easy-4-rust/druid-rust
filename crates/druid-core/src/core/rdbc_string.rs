//! RDBC string value object with Java-compatible UTF-16 semantics.
//!
//! Corresponds to the `java.lang.String` value representation used by the Java SQL API. A Java
//! string is a sequence of UTF-16 code units and can contain unpaired surrogates, which a Rust
//! UTF-8 `String` cannot represent without loss.

use super::DruidError;
use std::fmt;

/// Lossless RDBC string represented as Java-compatible UTF-16 code units.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RdbcString {
    code_units: Vec<u16>,
}

impl RdbcString {
    /// Creates a value from UTF-16 code units without normalizing surrogate pairs.
    pub fn from_utf16(code_units: impl Into<Vec<u16>>) -> Self {
        Self {
            code_units: code_units.into(),
        }
    }

    /// Creates a value from a Rust UTF-8 string.
    pub fn from_rust_str(value: &str) -> Self {
        Self::from_utf16(value.encode_utf16().collect::<Vec<_>>())
    }

    /// Returns the underlying UTF-16 code units.
    pub fn as_utf16(&self) -> &[u16] {
        &self.code_units
    }

    /// Returns the UTF-16 code-unit count corresponding to Java `String#length()`.
    pub fn len(&self) -> usize {
        self.code_units.len()
    }

    /// Returns whether the value contains no UTF-16 code units.
    pub fn is_empty(&self) -> bool {
        self.code_units.is_empty()
    }

    /// Converts the value to a Rust UTF-8 `String` without replacement characters.
    ///
    /// Returns an error when an unpaired surrogate prevents a lossless conversion.
    pub fn to_rust_string(&self) -> Result<String, DruidError> {
        String::from_utf16(&self.code_units).map_err(|error| {
            DruidError::DriverError(format!("RDBC string contains invalid UTF-16: {error}"))
        })
    }
}

impl From<String> for RdbcString {
    fn from(value: String) -> Self {
        Self::from_rust_str(&value)
    }
}

impl From<&str> for RdbcString {
    fn from(value: &str) -> Self {
        Self::from_rust_str(value)
    }
}

impl fmt::Debug for RdbcString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcString")
            .field("utf16_length", &self.len())
            .field("valid_utf8", &String::from_utf16(&self.code_units).ok())
            .finish()
    }
}
