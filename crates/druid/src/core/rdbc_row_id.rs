//! RDBC `RowId` platform value.
//!
//! Corresponds to Java: `java.sql.RowId`. Equality and hashing use the bytes returned by
//! `getBytes`; Rust stores those bytes directly and derives the same value semantics.

/// Database row identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RdbcRowId {
    bytes: Vec<u8>,
}

impl RdbcRowId {
    /// Creates a value from bytes returned by the driver.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    /// Borrows the row identifier bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns a copy of the bytes. Corresponds to Java: `RowId#getBytes()`.
    #[must_use]
    pub fn get_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}
