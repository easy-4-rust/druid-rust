use crate::error::RdbcError;
use crate::resource::{
    RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcResourceAccess, RdbcWriter,
    RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
};
use crate::types::RdbcString;

/// Driver access contract for the operations defined by Java `java.sql.SQLXML`.
#[async_trait::async_trait]
pub trait RdbcSqlXmlAccess: RdbcResourceAccess {
    /// Returns a binary input stream.
    async fn binary_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("SQLXML#getBinaryStream"))
    }
    /// Returns a binary output stream for writing XML.
    async fn set_binary_stream(&self) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("SQLXML#setBinaryStream"))
    }
    /// Returns a character reader.
    async fn character_stream(&self) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "SQLXML#getCharacterStream",
        ))
    }
    /// Returns a character writer for writing XML.
    async fn set_character_stream(&self) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "SQLXML#setCharacterStream",
        ))
    }
    /// Returns the XML string.
    async fn string(&self) -> Result<RdbcString, RdbcError>;
    /// Sets the XML string.
    async fn set_string(&self, _value: &RdbcString) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("SQLXML#setString"))
    }
    /// Returns an XML source in the requested representation.
    async fn source(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, RdbcError> {
        Err(RdbcError::feature_not_supported("SQLXML#getSource"))
    }
    /// Returns an XML result in the requested representation.
    async fn result(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, RdbcError> {
        Err(RdbcError::feature_not_supported("SQLXML#setResult"))
    }
}
