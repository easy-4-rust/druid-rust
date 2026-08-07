//! Standard Rust mapping for Java `java.sql.SQLXML`.

use crate::core::{
    DruidError, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcString, RdbcWriter,
    RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
};
use crate::spi::{
    RdbcResourceCapabilities, RdbcResourceContext, RdbcResourceId, RdbcResourceState,
    RdbcSqlXmlAccess,
};
use std::fmt;
use std::sync::Arc;

/// Driver-neutral RDBC `SQLXML` handle.
#[derive(Clone)]
pub struct RdbcSqlXml {
    access: Arc<dyn RdbcSqlXmlAccess>,
    context: Arc<RdbcResourceContext>,
}

impl RdbcSqlXml {
    pub(crate) fn from_parts(
        access: Arc<dyn RdbcSqlXmlAccess>,
        context: Arc<RdbcResourceContext>,
    ) -> Self {
        Self { access, context }
    }

    /// Releases the XML resource. Corresponds to `SQLXML#free()`.
    pub async fn free(&self) -> Result<(), DruidError> {
        self.context
            .require_capability(RdbcResourceCapabilities::FREE, "SQLXML#free")?;
        self.context.free(self.access.as_ref()).await
    }

    /// Returns whether the XML resource was released.
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.context.is_freed()
    }

    /// Returns a binary input stream. Corresponds to `SQLXML#getBinaryStream()`.
    pub async fn binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::STREAM,
            "SQLXML#getBinaryStream",
        )?;
        self.context.observe(self.access.binary_stream().await)
    }

    /// Returns a binary input stream. Corresponds to `SQLXML#getBinaryStream()`.
    pub async fn get_binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.binary_stream().await
    }

    /// Returns a binary output stream. Corresponds to `SQLXML#setBinaryStream()`.
    pub async fn set_binary_stream(&self) -> Result<RdbcOutputStream, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::STREAM,
            "SQLXML#setBinaryStream",
        )?;
        self.context.observe(self.access.set_binary_stream().await)
    }

    /// Returns a character reader. Corresponds to `SQLXML#getCharacterStream()`.
    pub async fn character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::STREAM,
            "SQLXML#getCharacterStream",
        )?;
        self.context.observe(self.access.character_stream().await)
    }

    /// Returns a character reader. Corresponds to `SQLXML#getCharacterStream()`.
    pub async fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.character_stream().await
    }

    /// Returns a character writer. Corresponds to `SQLXML#setCharacterStream()`.
    pub async fn set_character_stream(&self) -> Result<RdbcWriter, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::WRITE | RdbcResourceCapabilities::STREAM,
            "SQLXML#setCharacterStream",
        )?;
        self.context
            .observe(self.access.set_character_stream().await)
    }

    /// Returns the XML string. Corresponds to `SQLXML#getString()`.
    pub async fn string(&self) -> Result<RdbcString, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "SQLXML#getString")?;
        self.context.observe(self.access.string().await)
    }

    /// Returns the XML string. Corresponds to `SQLXML#getString()`.
    pub async fn get_string(&self) -> Result<RdbcString, DruidError> {
        self.string().await
    }

    /// Sets the XML string. Corresponds to `SQLXML#setString(String)`.
    pub async fn set_string(&self, value: &RdbcString) -> Result<(), DruidError> {
        self.context
            .require(RdbcResourceCapabilities::WRITE, "SQLXML#setString")?;
        self.context.observe(self.access.set_string(value).await)
    }

    /// Returns an XML source in the requested representation. Corresponds to `SQLXML#getSource`.
    pub async fn source(
        &self,
        representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "SQLXML#getSource")?;
        self.context
            .observe(self.access.source(representation).await)
    }

    /// Returns an XML result in the requested representation. Corresponds to `SQLXML#setResult`.
    pub async fn result(
        &self,
        representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::WRITE, "SQLXML#setResult")?;
        self.context
            .observe(self.access.result(representation).await)
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

    /// Returns the operations enabled for this SQLXML instance.
    #[must_use]
    pub fn capabilities(&self) -> RdbcResourceCapabilities {
        self.context.capabilities()
    }
}

impl fmt::Debug for RdbcSqlXml {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcSqlXml")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RdbcSqlXml {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.context, &other.context)
    }
}

impl Eq for RdbcSqlXml {}

/// Standard mapping of an SQL `XML` value.
pub type SqlXml = RdbcSqlXml;
