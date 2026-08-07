//! RDBC `SQLXML` platform resource.
//!
//! Corresponds to Java: `java.sql.SQLXML`.

use super::{
    DruidError, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcString, RdbcWriter,
    RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
};
use std::fmt;
use std::sync::Arc;

/// Physical RDBC `SQLXML` SPI covering the Java operation families.
pub trait PhysicalSqlXml: fmt::Debug + Send + Sync {
    /// Releases the XML resource.
    fn free(&self) -> Result<(), DruidError>;

    /// Returns whether the resource has been released.
    fn is_freed(&self) -> bool;

    /// Returns a binary input stream.
    fn binary_stream(&self) -> Result<RdbcInputStream, DruidError>;

    /// Returns a binary output stream for writing XML.
    fn set_binary_stream(&self) -> Result<RdbcOutputStream, DruidError>;

    /// Returns a character reader.
    fn character_stream(&self) -> Result<RdbcReader, DruidError>;

    /// Returns a character writer for writing XML.
    fn set_character_stream(&self) -> Result<RdbcWriter, DruidError>;

    /// Returns the XML string.
    fn string(&self) -> Result<RdbcString, DruidError>;

    /// Sets the XML string.
    fn set_string(&self, value: &RdbcString) -> Result<(), DruidError>;

    /// Returns an XML source in the requested representation.
    fn source(
        &self,
        representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, DruidError>;

    /// Returns an XML result in the requested representation.
    fn result(
        &self,
        representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, DruidError>;
}

/// Driver-neutral RDBC `SQLXML` handle.
#[derive(Clone)]
pub struct RdbcSqlXml {
    physical: Arc<dyn PhysicalSqlXml>,
}

impl RdbcSqlXml {
    /// Wraps a physical `SQLXML` value.
    pub fn new(physical: Arc<dyn PhysicalSqlXml>) -> Self {
        Self { physical }
    }

    /// Releases the XML resource.
    pub fn free(&self) -> Result<(), DruidError> {
        self.physical.free()
    }

    /// Returns whether the resource has been released.
    pub fn is_freed(&self) -> bool {
        self.physical.is_freed()
    }

    /// Returns a binary input stream.
    pub fn binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.physical.binary_stream()
    }

    /// Snake_case getter corresponding to Java `SQLXML#getBinaryStream()`.
    pub fn get_binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.binary_stream()
    }

    /// Returns a binary output stream.
    pub fn set_binary_stream(&self) -> Result<RdbcOutputStream, DruidError> {
        self.physical.set_binary_stream()
    }

    /// Returns a character reader.
    pub fn character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.physical.character_stream()
    }

    /// Snake_case getter corresponding to Java `SQLXML#getCharacterStream()`.
    pub fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.character_stream()
    }

    /// Returns a character writer.
    pub fn set_character_stream(&self) -> Result<RdbcWriter, DruidError> {
        self.physical.set_character_stream()
    }

    /// Returns the XML string.
    pub fn string(&self) -> Result<RdbcString, DruidError> {
        self.physical.string()
    }

    /// Snake_case getter corresponding to Java `SQLXML#getString()`.
    pub fn get_string(&self) -> Result<RdbcString, DruidError> {
        self.string()
    }

    /// Sets the XML string.
    pub fn set_string(&self, value: &RdbcString) -> Result<(), DruidError> {
        self.physical.set_string(value)
    }

    /// Returns an XML source in the requested representation.
    pub fn source(
        &self,
        representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, DruidError> {
        self.physical.source(representation)
    }

    /// Returns an XML result in the requested representation.
    pub fn result(
        &self,
        representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, DruidError> {
        self.physical.result(representation)
    }

    /// Returns the physical `SQLXML` SPI.
    pub fn physical(&self) -> &dyn PhysicalSqlXml {
        self.physical.as_ref()
    }
}

impl fmt::Debug for RdbcSqlXml {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcSqlXml")
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for RdbcSqlXml {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcSqlXml {}
