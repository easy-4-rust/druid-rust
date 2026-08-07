//! RDBC `Blob` platform object and physical SPI.
//!
//! Corresponds to Java: `java.sql.Blob`. Druid forwards this resource without owning its byte
//! storage. SQLx, RBDC, and other adapters implement driver semantics through `PhysicalBlob`.

use super::{DruidError, RdbcInputStream, RdbcOutputStream};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// Complete RDBC operation contract for a physical `Blob`.
///
/// Signed position, length, and offset values are retained so the physical driver reports invalid
/// arguments under RDBC rules instead of the pool silently normalizing them.
pub trait PhysicalBlob: fmt::Debug + Send + Sync {
    /// Returns the concrete driver object for safe adapter downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns the Blob length. Corresponds to Java: `Blob#length()`.
    fn length(&self) -> Result<i64, DruidError>;

    /// Reads a byte range. Corresponds to Java: `Blob#getBytes(long, int)`.
    fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, DruidError>;

    /// Opens the full binary stream. Corresponds to Java: `Blob#getBinaryStream()`.
    fn get_binary_stream(&self) -> Result<RdbcInputStream, DruidError>;

    /// Finds a byte pattern. Corresponds to Java: `Blob#position(byte[], long)`.
    fn position_bytes(&self, pattern: &[u8], start: i64) -> Result<Option<i64>, DruidError>;

    /// Finds another Blob. Corresponds to Java: `Blob#position(Blob, long)`.
    fn position_blob(&self, pattern: &RdbcBlob, start: i64) -> Result<Option<i64>, DruidError>;

    /// Writes all bytes. Corresponds to Java: `Blob#setBytes(long, byte[])`.
    fn set_bytes(&self, position: i64, bytes: &[u8]) -> Result<i32, DruidError>;

    /// Writes a byte subrange. Corresponds to Java: `Blob#setBytes(long, byte[], int, int)`.
    fn set_bytes_range(
        &self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError>;

    /// Opens a positioned output stream. Corresponds to Java: `Blob#setBinaryStream(long)`.
    fn set_binary_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError>;

    /// Truncates the Blob. Corresponds to Java: `Blob#truncate(long)`.
    fn truncate(&self, length: i64) -> Result<(), DruidError>;

    /// Releases the Blob. Corresponds to Java: `Blob#free()`.
    fn free(&self) -> Result<(), DruidError>;

    /// Returns whether the Blob has been released, for Druid lifecycle handling and tests.
    fn is_freed(&self) -> bool;

    /// Opens a binary stream over a range. Corresponds to Java: `Blob#getBinaryStream(long, long)`.
    fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcInputStream, DruidError>;
}

/// Public RDBC Blob handle.
///
/// Clone preserves reference identity. Equality compares the physical Blob identity and never
/// reads large-object content implicitly.
#[derive(Clone)]
pub struct RdbcBlob {
    physical: Arc<dyn PhysicalBlob>,
}

impl RdbcBlob {
    /// Wraps a physical Blob adapter.
    pub fn new(physical: Arc<dyn PhysicalBlob>) -> Self {
        Self { physical }
    }

    /// Returns the physical Blob SPI.
    pub fn physical(&self) -> &dyn PhysicalBlob {
        self.physical.as_ref()
    }

    /// Returns the Blob length. Corresponds to Java: `Blob#length()`.
    pub fn length(&self) -> Result<i64, DruidError> {
        self.physical.length()
    }

    /// Reads a byte range. Corresponds to Java: `Blob#getBytes(long, int)`.
    pub fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, DruidError> {
        self.physical.get_bytes(position, length)
    }

    /// Opens the full binary stream. Corresponds to Java: `Blob#getBinaryStream()`.
    pub fn get_binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.physical.get_binary_stream()
    }

    /// Finds a byte pattern. Corresponds to Java: `Blob#position(byte[], long)`.
    pub fn position_bytes(&self, pattern: &[u8], start: i64) -> Result<Option<i64>, DruidError> {
        self.physical.position_bytes(pattern, start)
    }

    /// Finds another Blob. Corresponds to Java: `Blob#position(Blob, long)`.
    pub fn position_blob(&self, pattern: &RdbcBlob, start: i64) -> Result<Option<i64>, DruidError> {
        self.physical.position_blob(pattern, start)
    }

    /// Writes all bytes. Corresponds to Java: `Blob#setBytes(long, byte[])`.
    pub fn set_bytes(&self, position: i64, bytes: &[u8]) -> Result<i32, DruidError> {
        self.physical.set_bytes(position, bytes)
    }

    /// Writes a byte subrange. Corresponds to Java: `Blob#setBytes(long, byte[], int, int)`.
    pub fn set_bytes_range(
        &self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.physical
            .set_bytes_range(position, bytes, offset, length)
    }

    /// Opens a positioned output stream. Corresponds to Java: `Blob#setBinaryStream(long)`.
    pub fn set_binary_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError> {
        self.physical.set_binary_stream(position)
    }

    /// Truncates the Blob. Corresponds to Java: `Blob#truncate(long)`.
    pub fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.physical.truncate(length)
    }

    /// Releases the Blob. Corresponds to Java: `Blob#free()`.
    pub fn free(&self) -> Result<(), DruidError> {
        self.physical.free()
    }

    /// Returns whether the Blob has been released.
    pub fn is_freed(&self) -> bool {
        self.physical.is_freed()
    }

    /// Opens a binary stream over a range. Corresponds to Java: `Blob#getBinaryStream(long, long)`.
    pub fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcInputStream, DruidError> {
        self.physical.get_binary_stream_range(position, length)
    }
}

impl fmt::Debug for RdbcBlob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcBlob")
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for RdbcBlob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcBlob {}
