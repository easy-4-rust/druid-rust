//! RDBC `Clob` platform object and physical SPI.
//!
//! Corresponds to Java: `java.sql.Clob`. A Clob is a driver-owned character resource and cannot
//! be reduced to a Rust `String` by the pooling layer.

use super::{DruidError, JavaString, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// Complete RDBC operation contract for a physical `Clob`.
///
/// Signed positions, lengths, and offsets are retained. The driver enforces 1-based positions,
/// character counting, and access errors after release.
pub trait PhysicalClob: fmt::Debug + Send + Sync {
    /// Returns the concrete driver object.
    fn as_any(&self) -> &dyn Any;

    /// Returns the character length. Corresponds to Java: `Clob#length()`.
    fn length(&self) -> Result<i64, DruidError>;

    /// Returns a substring. Corresponds to Java: `Clob#getSubString(long, int)`.
    fn get_sub_string(&self, position: i64, length: i32) -> Result<JavaString, DruidError>;

    /// Opens the character stream. Corresponds to Java: `Clob#getCharacterStream()`.
    fn get_character_stream(&self) -> Result<RdbcReader, DruidError>;

    /// Opens the ASCII stream. Corresponds to Java: `Clob#getAsciiStream()`.
    fn get_ascii_stream(&self) -> Result<RdbcInputStream, DruidError>;

    /// Finds a string. Corresponds to Java: `Clob#position(String, long)`.
    fn position_string(&self, pattern: &JavaString, start: i64) -> Result<Option<i64>, DruidError>;

    /// Finds another Clob. Corresponds to Java: `Clob#position(Clob, long)`.
    fn position_clob(&self, pattern: &RdbcClob, start: i64) -> Result<Option<i64>, DruidError>;

    /// Writes a string. Corresponds to Java: `Clob#setString(long, String)`.
    fn set_string(&self, position: i64, value: &JavaString) -> Result<i32, DruidError>;

    /// Writes a string range. Corresponds to Java: `Clob#setString(long, String, int, int)`.
    fn set_string_range(
        &self,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError>;

    /// Opens a positioned ASCII output stream. Corresponds to Java: `Clob#setAsciiStream(long)`.
    fn set_ascii_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError>;

    /// Opens a positioned character writer. Corresponds to Java: `Clob#setCharacterStream(long)`.
    fn set_character_stream(&self, position: i64) -> Result<RdbcWriter, DruidError>;

    /// Truncates the Clob. Corresponds to Java: `Clob#truncate(long)`.
    fn truncate(&self, length: i64) -> Result<(), DruidError>;

    /// Releases the Clob. Corresponds to Java: `Clob#free()`.
    fn free(&self) -> Result<(), DruidError>;

    /// Returns whether the Clob has been released.
    fn is_freed(&self) -> bool;

    /// Opens a character stream over a range. Corresponds to Java:
    /// `Clob#getCharacterStream(long, long)`.
    fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError>;
}

/// Public RDBC Clob handle.
///
/// Clone preserves physical identity and never reads or compares character content implicitly.
#[derive(Clone)]
pub struct RdbcClob {
    physical: Arc<dyn PhysicalClob>,
}

impl RdbcClob {
    /// Wraps a physical Clob adapter.
    pub fn new(physical: Arc<dyn PhysicalClob>) -> Self {
        Self { physical }
    }

    /// Returns the physical Clob SPI.
    pub fn physical(&self) -> &dyn PhysicalClob {
        self.physical.as_ref()
    }

    /// Returns the character length. Corresponds to Java: `Clob#length()`.
    pub fn length(&self) -> Result<i64, DruidError> {
        self.physical.length()
    }

    /// Returns a substring. Corresponds to Java: `Clob#getSubString(long, int)`.
    pub fn get_sub_string(&self, position: i64, length: i32) -> Result<JavaString, DruidError> {
        self.physical.get_sub_string(position, length)
    }

    /// Opens the character stream. Corresponds to Java: `Clob#getCharacterStream()`.
    pub fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.physical.get_character_stream()
    }

    /// Opens the ASCII stream. Corresponds to Java: `Clob#getAsciiStream()`.
    pub fn get_ascii_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.physical.get_ascii_stream()
    }

    /// Finds a string. Corresponds to Java: `Clob#position(String, long)`.
    pub fn position_string(
        &self,
        pattern: &JavaString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.physical.position_string(pattern, start)
    }

    /// Finds another Clob. Corresponds to Java: `Clob#position(Clob, long)`.
    pub fn position_clob(&self, pattern: &RdbcClob, start: i64) -> Result<Option<i64>, DruidError> {
        self.physical.position_clob(pattern, start)
    }

    /// Writes a string. Corresponds to Java: `Clob#setString(long, String)`.
    pub fn set_string(&self, position: i64, value: &JavaString) -> Result<i32, DruidError> {
        self.physical.set_string(position, value)
    }

    /// Writes a string range. Corresponds to Java: `Clob#setString(long, String, int, int)`.
    pub fn set_string_range(
        &self,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.physical
            .set_string_range(position, value, offset, length)
    }

    /// Opens a positioned ASCII output stream. Corresponds to Java: `Clob#setAsciiStream(long)`.
    pub fn set_ascii_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError> {
        self.physical.set_ascii_stream(position)
    }

    /// Opens a positioned character writer. Corresponds to Java: `Clob#setCharacterStream(long)`.
    pub fn set_character_stream(&self, position: i64) -> Result<RdbcWriter, DruidError> {
        self.physical.set_character_stream(position)
    }

    /// Truncates the Clob. Corresponds to Java: `Clob#truncate(long)`.
    pub fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.physical.truncate(length)
    }

    /// Releases the Clob. Corresponds to Java: `Clob#free()`.
    pub fn free(&self) -> Result<(), DruidError> {
        self.physical.free()
    }

    /// Returns whether the Clob has been released.
    pub fn is_freed(&self) -> bool {
        self.physical.is_freed()
    }

    /// Opens a character stream over a range. Corresponds to Java:
    /// `Clob#getCharacterStream(long, long)`.
    pub fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        self.physical.get_character_stream_range(position, length)
    }
}

impl fmt::Debug for RdbcClob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcClob")
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for RdbcClob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcClob {}
