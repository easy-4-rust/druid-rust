//! Standard Rust mapping for Java `java.sql.NClob`.

use super::clob::RdbcClob;
use crate::core::{
    DruidError, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcString, RdbcWriter,
};
use crate::spi::{
    RdbcNClobAccess, RdbcResourceCapabilities, RdbcResourceContext, RdbcResourceId,
    RdbcResourceState,
};
use std::fmt;
use std::sync::Arc;

/// Driver-neutral RDBC `NClob` handle.
///
/// The type retains national-character identity and explicitly delegates the operations inherited
/// from Java `NClob extends Clob`; it does not use `Deref` as inheritance.
#[derive(Clone)]
pub struct RdbcNClob {
    clob: RdbcClob,
}

impl RdbcNClob {
    pub(crate) fn from_parts(
        access: Arc<dyn RdbcNClobAccess>,
        context: Arc<RdbcResourceContext>,
    ) -> Self {
        let clob_access: Arc<dyn crate::spi::RdbcClobAccess> = access;
        Self {
            clob: RdbcClob::from_parts(clob_access, context),
        }
    }

    /// Returns the inherited Clob view without exposing its access implementation.
    #[must_use]
    pub const fn as_clob(&self) -> &RdbcClob {
        &self.clob
    }

    /// Returns the character length. Corresponds to `Clob#length()`.
    pub async fn length(&self) -> Result<i64, DruidError> {
        self.clob.length().await
    }

    /// Returns a 1-based substring. Corresponds to `Clob#getSubString(long,int)`.
    pub async fn get_sub_string(
        &self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, DruidError> {
        self.clob.get_sub_string(position, length).await
    }

    /// Opens the full character stream. Corresponds to `Clob#getCharacterStream()`.
    pub async fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.clob.get_character_stream().await
    }

    /// Opens the ASCII stream. Corresponds to `Clob#getAsciiStream()`.
    pub async fn get_ascii_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.clob.get_ascii_stream().await
    }

    /// Finds a string. Corresponds to `Clob#position(String,long)`.
    pub async fn position_string(
        &self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.clob.position_string(pattern, start).await
    }

    /// Finds another Clob. Corresponds to `Clob#position(Clob,long)`.
    pub async fn position_clob(
        &self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.clob.position_clob(pattern, start).await
    }

    /// Writes a string. Corresponds to `Clob#setString(long,String)`.
    pub async fn set_string(&self, position: i64, value: &RdbcString) -> Result<i32, DruidError> {
        self.clob.set_string(position, value).await
    }

    /// Writes a string range. Corresponds to `Clob#setString(long,String,int,int)`.
    pub async fn set_string_range(
        &self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.clob
            .set_string_range(position, value, offset, length)
            .await
    }

    /// Opens a positioned ASCII output stream. Corresponds to `Clob#setAsciiStream(long)`.
    pub async fn set_ascii_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError> {
        self.clob.set_ascii_stream(position).await
    }

    /// Opens a positioned character writer. Corresponds to `Clob#setCharacterStream(long)`.
    pub async fn set_character_stream(&self, position: i64) -> Result<RdbcWriter, DruidError> {
        self.clob.set_character_stream(position).await
    }

    /// Truncates the `NClob`. Corresponds to `Clob#truncate(long)`.
    pub async fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.clob.truncate(length).await
    }

    /// Releases the `NClob`. Corresponds to `Clob#free()`.
    pub async fn free(&self) -> Result<(), DruidError> {
        self.clob.free().await
    }

    /// Returns whether the `NClob` was released.
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.clob.is_freed()
    }

    /// Opens a character stream over a range. Corresponds to `Clob#getCharacterStream(long,long)`.
    pub async fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        self.clob.get_character_stream_range(position, length).await
    }

    /// Returns the driver-neutral resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &RdbcResourceId {
        self.clob.resource_id()
    }

    /// Returns the current shared resource state.
    #[must_use]
    pub fn state(&self) -> RdbcResourceState {
        self.clob.state()
    }

    /// Returns the operations enabled for this `NClob` instance.
    #[must_use]
    pub fn capabilities(&self) -> RdbcResourceCapabilities {
        self.clob.capabilities()
    }
}

impl fmt::Debug for RdbcNClob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcNClob")
            .field("resource_id", &self.resource_id())
            .field("state", &self.state())
            .field("capabilities", &self.capabilities())
            .finish()
    }
}

impl PartialEq for RdbcNClob {
    fn eq(&self, other: &Self) -> bool {
        self.clob == other.clob
    }
}

impl Eq for RdbcNClob {}

/// Mapping of an SQL `NCLOB` national-character large object.
pub type NClob = RdbcNClob;
