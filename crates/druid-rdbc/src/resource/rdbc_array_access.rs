use crate::error::RdbcError;
use crate::resource::RdbcResourceAccess;
use crate::result_set::ResultSet;
use crate::types::{RdbcObject, RdbcTypeMap};

/// Driver access contract for the operations defined by Java `java.sql.Array`.
#[async_trait::async_trait]
pub trait RdbcArrayAccess: RdbcResourceAccess {
    /// Returns the SQL type name of array elements.
    async fn base_type_name(&self) -> Result<String, RdbcError>;
    /// Returns the `java.sql.Types` number of array elements.
    async fn base_type(&self) -> Result<i32, RdbcError>;
    /// Reads all elements with the default type map.
    async fn values(&self) -> Result<Vec<RdbcObject>, RdbcError>;
    /// Reads all elements with an explicit type map.
    async fn values_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(RdbcError::feature_not_supported("Array#getArray(Map)"))
    }
    /// Reads `count` elements from a 1-based index.
    async fn values_range(&self, _index: i64, _count: i32) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(RdbcError::feature_not_supported("Array#getArray(long,int)"))
    }
    /// Reads a range with an explicit type map.
    async fn values_range_with_type_map(
        &self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Array#getArray(long,int,Map)",
        ))
    }
    /// Returns all elements as a result set using the default type map.
    async fn result_set(&self) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported("Array#getResultSet"))
    }
    /// Returns all elements as a result set using an explicit type map.
    async fn result_set_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported("Array#getResultSet(Map)"))
    }
    /// Returns a range as a result set.
    async fn result_set_range(
        &self,
        _index: i64,
        _count: i32,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Array#getResultSet(long,int)",
        ))
    }
    /// Returns a range as a result set using an explicit type map.
    async fn result_set_range_with_type_map(
        &self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Array#getResultSet(long,int,Map)",
        ))
    }
}
