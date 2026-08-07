//! Standard Rust mapping for Java `java.sql.Clob`.

use crate::core::{
    DruidError, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcString, RdbcWriter,
};
use crate::spi::{
    RdbcClobAccess, RdbcResourceCapabilities, RdbcResourceContext, RdbcResourceId,
    RdbcResourceState,
};
use std::fmt;
use std::sync::Arc;

/// Driver-neutral RDBC `Clob` handle.
///
/// Clone preserves resource identity and never reads character content implicitly.
#[derive(Clone)]
pub struct RdbcClob {
    access: Arc<dyn RdbcClobAccess>,
    context: Arc<RdbcResourceContext>,
}

impl RdbcClob {
    pub(crate) fn from_parts(
        access: Arc<dyn RdbcClobAccess>,
        context: Arc<RdbcResourceContext>,
    ) -> Self {
        Self { access, context }
    }

    /// Returns the character length. Corresponds to `Clob#length()`.
    pub async fn length(&self) -> Result<i64, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "Clob#length")?;
        self.context.observe(self.access.length().await)
    }

    /// Returns a 1-based substring. Corresponds to `Clob#getSubString(long,int)`.
    pub async fn get_sub_string(
        &self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::RANGE,
            "Clob#getSubString",
        )?;
        self.context
            .observe(self.access.get_sub_string(position, length).await)
    }

    /// Opens the full character stream. Corresponds to `Clob#getCharacterStream()`.
    pub async fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::STREAM,
            "Clob#getCharacterStream",
        )?;
        self.context
            .observe(self.access.get_character_stream().await)
    }

    /// Opens the ASCII stream. Corresponds to `Clob#getAsciiStream()`.
    pub async fn get_ascii_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::STREAM,
            "Clob#getAsciiStream",
        )?;
        self.context.observe(self.access.get_ascii_stream().await)
    }

    /// Finds a string. Corresponds to `Clob#position(String,long)`.
    pub async fn position_string(
        &self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::SEARCH,
            "Clob#position(String,long)",
        )?;
        self.context
            .observe(self.access.position_string(pattern, start).await)
    }

    /// Finds another Clob. Corresponds to `Clob#position(Clob,long)`.
    pub async fn position_clob(
        &self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::SEARCH,
            "Clob#position(Clob,long)",
        )?;
        self.context
            .observe(self.access.position_clob(pattern, start).await)
    }

    /// Writes a string. Corresponds to `Clob#setString(long,String)`.
    pub async fn set_string(&self, position: i64, value: &RdbcString) -> Result<i32, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::WRITE, "Clob#setString")?;
        self.context
            .observe(self.access.set_string(position, value).await)
    }

    /// Writes a string range. Corresponds to `Clob#setString(long,String,int,int)`.
    pub async fn set_string_range(
        &self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::RANGE,
            "Clob#setString(long,String,int,int)",
        )?;
        self.context.observe(
            self.access
                .set_string_range(position, value, offset, length)
                .await,
        )
    }

    /// Opens a positioned ASCII output stream. Corresponds to `Clob#setAsciiStream(long)`.
    pub async fn set_ascii_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::STREAM,
            "Clob#setAsciiStream",
        )?;
        self.context
            .observe(self.access.set_ascii_stream(position).await)
    }

    /// Opens a positioned character writer. Corresponds to `Clob#setCharacterStream(long)`.
    pub async fn set_character_stream(&self, position: i64) -> Result<RdbcWriter, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::STREAM,
            "Clob#setCharacterStream",
        )?;
        self.context
            .observe(self.access.set_character_stream(position).await)
    }

    /// Truncates the Clob. Corresponds to `Clob#truncate(long)`.
    pub async fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::TRUNCATE,
            "Clob#truncate",
        )?;
        self.context.observe(self.access.truncate(length).await)
    }

    /// Releases the Clob. Corresponds to `Clob#free()`.
    pub async fn free(&self) -> Result<(), DruidError> {
        self.context
            .require_capability(RdbcResourceCapabilities::FREE, "Clob#free")?;
        self.context.free(self.access.as_ref()).await
    }

    /// Returns whether the Clob was released.
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.context.is_freed()
    }

    /// Opens a character stream over a range. Corresponds to `Clob#getCharacterStream(long,long)`.
    pub async fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ
                | RdbcResourceCapabilities::STREAM
                | RdbcResourceCapabilities::RANGE,
            "Clob#getCharacterStream(long,long)",
        )?;
        self.context.observe(
            self.access
                .get_character_stream_range(position, length)
                .await,
        )
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

    /// Returns the operations enabled for this Clob instance.
    #[must_use]
    pub fn capabilities(&self) -> RdbcResourceCapabilities {
        self.context.capabilities()
    }
}

impl fmt::Debug for RdbcClob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcClob")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RdbcClob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.context, &other.context)
    }
}

impl Eq for RdbcClob {}

/// Mapping of an SQL `CLOB` character large object.
pub type Clob = RdbcClob;
