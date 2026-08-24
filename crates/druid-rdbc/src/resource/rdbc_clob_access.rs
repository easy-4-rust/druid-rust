use crate::error::RdbcError;
use crate::resource::{
    RdbcClob, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcResourceAccess, RdbcWriter,
};
use crate::types::RdbcString;

/// Driver access contract for the operations defined by Java `java.sql.Clob`.
#[async_trait::async_trait]
pub trait RdbcClobAccess: RdbcResourceAccess {
    /// Returns the character length.
    async fn length(&self) -> Result<i64, RdbcError>;
    /// Returns a substring.
    async fn get_sub_string(&self, position: i64, length: i32) -> Result<RdbcString, RdbcError>;
    /// Opens the character stream.
    async fn get_character_stream(&self) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#getCharacterStream"))
    }
    /// Opens the ASCII stream.
    async fn get_ascii_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#getAsciiStream"))
    }
    /// Finds a string.
    async fn position_string(
        &self,
        _pattern: &RdbcString,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#position(String,long)",
        ))
    }
    /// Finds another Clob.
    async fn position_clob(
        &self,
        _pattern: &RdbcClob,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#position(Clob,long)"))
    }
    /// Writes a string.
    async fn set_string(&self, _position: i64, _value: &RdbcString) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#setString"))
    }
    /// Writes a string range.
    async fn set_string_range(
        &self,
        _position: i64,
        _value: &RdbcString,
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#setString(long,String,int,int)",
        ))
    }
    /// Opens a positioned ASCII output stream.
    async fn set_ascii_stream(&self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#setAsciiStream"))
    }
    /// Opens a positioned character writer.
    async fn set_character_stream(&self, _position: i64) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#setCharacterStream"))
    }
    /// Truncates the Clob.
    async fn truncate(&self, _length: i64) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#truncate"))
    }
    /// Opens a character stream over a range.
    async fn get_character_stream_range(
        &self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#getCharacterStream(long,long)",
        ))
    }
}
