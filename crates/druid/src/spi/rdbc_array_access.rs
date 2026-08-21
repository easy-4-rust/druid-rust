use super::RdbcResourceAccess;
use crate::core::{DruidError, RdbcObject, RdbcResultSet, RdbcTypeMap};

/// Driver access contract for the operations defined by Java `java.sql.Array`.
#[async_trait::async_trait]
pub trait RdbcArrayAccess: RdbcResourceAccess {
    /// Returns the SQL type name of array elements.
    async fn base_type_name(&self) -> Result<String, DruidError>;
    /// Returns the `java.sql.Types` number of array elements.
    async fn base_type(&self) -> Result<i32, DruidError>;
    /// Reads all elements with the default type map.
    async fn values(&self) -> Result<Vec<RdbcObject>, DruidError>;
    /// Reads all elements with an explicit type map.
    async fn values_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        Err(DruidError::feature_not_supported("Array#getArray(Map)"))
    }
    /// Reads `count` elements from a 1-based index.
    async fn values_range(&self, _index: i64, _count: i32) -> Result<Vec<RdbcObject>, DruidError> {
        Err(DruidError::feature_not_supported(
            "Array#getArray(long,int)",
        ))
    }
    /// Reads a range with an explicit type map.
    async fn values_range_with_type_map(
        &self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        Err(DruidError::feature_not_supported(
            "Array#getArray(long,int,Map)",
        ))
    }
    /// Returns all elements as a result set using the default type map.
    async fn result_set(&self) -> Result<RdbcResultSet, DruidError> {
        Err(DruidError::feature_not_supported("Array#getResultSet"))
    }
    /// Returns all elements as a result set using an explicit type map.
    async fn result_set_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        Err(DruidError::feature_not_supported("Array#getResultSet(Map)"))
    }
    /// Returns a range as a result set.
    async fn result_set_range(
        &self,
        _index: i64,
        _count: i32,
    ) -> Result<RdbcResultSet, DruidError> {
        Err(DruidError::feature_not_supported(
            "Array#getResultSet(long,int)",
        ))
    }
    /// Returns a range as a result set using an explicit type map.
    async fn result_set_range_with_type_map(
        &self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        Err(DruidError::feature_not_supported(
            "Array#getResultSet(long,int,Map)",
        ))
    }
}
