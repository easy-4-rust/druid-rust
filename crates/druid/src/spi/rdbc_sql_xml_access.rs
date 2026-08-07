use super::RdbcResourceAccess;
use crate::core::{
    DruidError, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcString, RdbcWriter,
    RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
};

/// Driver access contract for the operations defined by Java `java.sql.SQLXML`.
#[async_trait::async_trait]
pub trait RdbcSqlXmlAccess: RdbcResourceAccess {
    /// Returns a binary input stream.
    async fn binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        Err(DruidError::feature_not_supported("SQLXML#getBinaryStream"))
    }
    /// Returns a binary output stream for writing XML.
    async fn set_binary_stream(&self) -> Result<RdbcOutputStream, DruidError> {
        Err(DruidError::feature_not_supported("SQLXML#setBinaryStream"))
    }
    /// Returns a character reader.
    async fn character_stream(&self) -> Result<RdbcReader, DruidError> {
        Err(DruidError::feature_not_supported(
            "SQLXML#getCharacterStream",
        ))
    }
    /// Returns a character writer for writing XML.
    async fn set_character_stream(&self) -> Result<RdbcWriter, DruidError> {
        Err(DruidError::feature_not_supported(
            "SQLXML#setCharacterStream",
        ))
    }
    /// Returns the XML string.
    async fn string(&self) -> Result<RdbcString, DruidError>;
    /// Sets the XML string.
    async fn set_string(&self, _value: &RdbcString) -> Result<(), DruidError> {
        Err(DruidError::feature_not_supported("SQLXML#setString"))
    }
    /// Returns an XML source in the requested representation.
    async fn source(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, DruidError> {
        Err(DruidError::feature_not_supported("SQLXML#getSource"))
    }
    /// Returns an XML result in the requested representation.
    async fn result(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, DruidError> {
        Err(DruidError::feature_not_supported("SQLXML#setResult"))
    }
}
