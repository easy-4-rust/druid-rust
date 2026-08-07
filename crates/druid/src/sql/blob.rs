//! Standard Rust mapping for Java `java.sql.Blob`.

use crate::core::{DruidError, RdbcInputStream, RdbcOutputStream};
use crate::spi::{
    RdbcBlobAccess, RdbcResourceCapabilities, RdbcResourceContext, RdbcResourceId,
    RdbcResourceState,
};
use std::fmt;
use std::sync::Arc;

/// Driver-neutral RDBC `Blob` handle.
///
/// Clone preserves resource identity. Equality never reads large-object content.
#[derive(Clone)]
pub struct RdbcBlob {
    access: Arc<dyn RdbcBlobAccess>,
    context: Arc<RdbcResourceContext>,
}

impl RdbcBlob {
    pub(crate) fn from_parts(
        access: Arc<dyn RdbcBlobAccess>,
        context: Arc<RdbcResourceContext>,
    ) -> Self {
        Self { access, context }
    }

    /// Returns the Blob length. Corresponds to `Blob#length()`.
    pub async fn length(&self) -> Result<i64, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "Blob#length")?;
        self.context.observe(self.access.length().await)
    }

    /// Reads a 1-based byte range. Corresponds to `Blob#getBytes(long,int)`.
    pub async fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::RANGE,
            "Blob#getBytes",
        )?;
        self.context
            .observe(self.access.get_bytes(position, length).await)
    }

    /// Opens the full binary stream. Corresponds to `Blob#getBinaryStream()`.
    pub async fn get_binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::STREAM,
            "Blob#getBinaryStream",
        )?;
        self.context.observe(self.access.get_binary_stream().await)
    }

    /// Finds a byte pattern. Corresponds to `Blob#position(byte[],long)`.
    pub async fn position_bytes(
        &self,
        pattern: &[u8],
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::SEARCH,
            "Blob#position(byte[],long)",
        )?;
        self.context
            .observe(self.access.position_bytes(pattern, start).await)
    }

    /// Finds another Blob. Corresponds to `Blob#position(Blob,long)`.
    pub async fn position_blob(
        &self,
        pattern: &RdbcBlob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::SEARCH,
            "Blob#position(Blob,long)",
        )?;
        self.context
            .observe(self.access.position_blob(pattern, start).await)
    }

    /// Writes all bytes. Corresponds to `Blob#setBytes(long,byte[])`.
    pub async fn set_bytes(&self, position: i64, bytes: &[u8]) -> Result<i32, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::WRITE, "Blob#setBytes")?;
        self.context
            .observe(self.access.set_bytes(position, bytes).await)
    }

    /// Writes a byte subrange. Corresponds to `Blob#setBytes(long,byte[],int,int)`.
    pub async fn set_bytes_range(
        &self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::RANGE,
            "Blob#setBytes(long,byte[],int,int)",
        )?;
        self.context.observe(
            self.access
                .set_bytes_range(position, bytes, offset, length)
                .await,
        )
    }

    /// Opens a positioned output stream. Corresponds to `Blob#setBinaryStream(long)`.
    pub async fn set_binary_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::STREAM,
            "Blob#setBinaryStream",
        )?;
        self.context
            .observe(self.access.set_binary_stream(position).await)
    }

    /// Truncates the Blob. Corresponds to `Blob#truncate(long)`.
    pub async fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::TRUNCATE,
            "Blob#truncate",
        )?;
        self.context.observe(self.access.truncate(length).await)
    }

    /// Releases the Blob. Corresponds to `Blob#free()`.
    pub async fn free(&self) -> Result<(), DruidError> {
        self.context
            .require_capability(RdbcResourceCapabilities::FREE, "Blob#free")?;
        self.context.free(self.access.as_ref()).await
    }

    /// Returns whether the Blob was released.
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.context.is_freed()
    }

    /// Opens a binary stream over a range. Corresponds to `Blob#getBinaryStream(long,long)`.
    pub async fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcInputStream, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ
                | RdbcResourceCapabilities::STREAM
                | RdbcResourceCapabilities::RANGE,
            "Blob#getBinaryStream(long,long)",
        )?;
        self.context
            .observe(self.access.get_binary_stream_range(position, length).await)
    }

    /// Returns the driver-neutral resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &RdbcResourceId {
        self.context.resource_id()
    }

    /// Returns the current shared resource state.
    #[must_use]
    pub fn state(&self) -> RdbcResourceState {
        self.context.state()
    }

    /// Returns the operations enabled for this Blob instance.
    #[must_use]
    pub fn capabilities(&self) -> RdbcResourceCapabilities {
        self.context.capabilities()
    }
}

impl fmt::Debug for RdbcBlob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcBlob")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RdbcBlob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.context, &other.context)
    }
}

impl Eq for RdbcBlob {}

/// Mapping of an SQL `BLOB` binary large object.
pub type Blob = RdbcBlob;
