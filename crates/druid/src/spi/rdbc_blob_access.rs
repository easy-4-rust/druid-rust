use super::RdbcResourceAccess;
use crate::core::{DruidError, RdbcInputStream, RdbcOutputStream};
use crate::rdbc::RdbcBlob;

/// Driver access contract for the operations defined by Java `java.sql.Blob`.
#[async_trait::async_trait]
pub trait RdbcBlobAccess: RdbcResourceAccess {
    /// Returns the Blob length.
    async fn length(&self) -> Result<i64, DruidError>;
    /// Reads a byte range.
    async fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, DruidError>;
    /// Opens the full binary stream.
    async fn get_binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        Err(DruidError::feature_not_supported("Blob#getBinaryStream"))
    }
    /// Finds a byte pattern.
    async fn position_bytes(
        &self,
        _pattern: &[u8],
        _start: i64,
    ) -> Result<Option<i64>, DruidError> {
        Err(DruidError::feature_not_supported(
            "Blob#position(byte[],long)",
        ))
    }
    /// Finds another Blob.
    async fn position_blob(
        &self,
        _pattern: &RdbcBlob,
        _start: i64,
    ) -> Result<Option<i64>, DruidError> {
        Err(DruidError::feature_not_supported(
            "Blob#position(Blob,long)",
        ))
    }
    /// Writes all bytes.
    async fn set_bytes(&self, _position: i64, _bytes: &[u8]) -> Result<i32, DruidError> {
        Err(DruidError::feature_not_supported("Blob#setBytes"))
    }
    /// Writes a byte subrange.
    async fn set_bytes_range(
        &self,
        _position: i64,
        _bytes: &[u8],
        _offset: i32,
        _length: i32,
    ) -> Result<i32, DruidError> {
        Err(DruidError::feature_not_supported(
            "Blob#setBytes(long,byte[],int,int)",
        ))
    }
    /// Opens a positioned output stream.
    async fn set_binary_stream(&self, _position: i64) -> Result<RdbcOutputStream, DruidError> {
        Err(DruidError::feature_not_supported("Blob#setBinaryStream"))
    }
    /// Truncates the Blob.
    async fn truncate(&self, _length: i64) -> Result<(), DruidError> {
        Err(DruidError::feature_not_supported("Blob#truncate"))
    }
    /// Opens a binary stream over a range.
    async fn get_binary_stream_range(
        &self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcInputStream, DruidError> {
        Err(DruidError::feature_not_supported(
            "Blob#getBinaryStream(long,long)",
        ))
    }
}
