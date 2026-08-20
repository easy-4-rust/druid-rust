use super::RdbcResourceAccess;
use crate::core::{DruidError, RdbcObject, RdbcTypeMap};

/// Driver access contract for the operations defined by Java `java.sql.Ref`.
#[async_trait::async_trait]
pub trait RdbcRefAccess: RdbcResourceAccess {
    /// Returns the fully qualified SQL name of the referenced structured type.
    async fn base_type_name(&self) -> Result<String, DruidError>;
    /// Reads the referenced object using the driver's default type map.
    async fn object(&self) -> Result<RdbcObject, DruidError>;
    /// Reads the referenced object using an explicit type map.
    async fn object_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, DruidError> {
        Err(DruidError::feature_not_supported("Ref#getObject(Map)"))
    }
    /// Replaces the referenced object.
    async fn set_object(&self, _value: RdbcObject) -> Result<(), DruidError> {
        Err(DruidError::feature_not_supported("Ref#setObject"))
    }
}
