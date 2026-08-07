//! RDBC `Ref` platform resource.
//!
//! Corresponds to Java: `java.sql.Ref`.

use super::{DruidError, RdbcObject, RdbcTypeMap};
use std::fmt;
use std::sync::Arc;

/// Physical RDBC `Ref` SPI covering the operations defined by `java.sql.Ref`.
pub trait PhysicalRef: fmt::Debug + Send + Sync {
    /// Returns the fully qualified SQL name of the referenced structured type.
    fn base_type_name(&self) -> Result<String, DruidError>;

    /// Reads the referenced object using the driver's default type map.
    fn object(&self) -> Result<RdbcObject, DruidError>;

    /// Reads the referenced object using an explicit type map.
    fn object_with_type_map(&self, type_map: &RdbcTypeMap) -> Result<RdbcObject, DruidError>;

    /// Replaces the referenced object.
    fn set_object(&self, value: RdbcObject) -> Result<(), DruidError>;
}

/// Driver-neutral RDBC `Ref` handle.
#[derive(Clone)]
pub struct RdbcRef {
    physical: Arc<dyn PhysicalRef>,
}

impl RdbcRef {
    /// Wraps a physical `Ref`.
    pub fn new(physical: Arc<dyn PhysicalRef>) -> Self {
        Self { physical }
    }

    /// Returns the referenced SQL type name. Corresponds to Java `Ref#getBaseTypeName()`.
    pub fn base_type_name(&self) -> Result<String, DruidError> {
        self.physical.base_type_name()
    }

    /// Snake_case getter corresponding to Java `Ref#getBaseTypeName()`.
    pub fn get_base_type_name(&self) -> Result<String, DruidError> {
        self.base_type_name()
    }

    /// Reads the referenced object. Corresponds to Java `Ref#getObject()`.
    pub fn object(&self) -> Result<RdbcObject, DruidError> {
        self.physical.object()
    }

    /// Snake_case getter corresponding to Java `Ref#getObject()`.
    pub fn get_object(&self) -> Result<RdbcObject, DruidError> {
        self.object()
    }

    /// Reads the referenced object with a type map. Corresponds to Java `Ref#getObject(Map)`.
    pub fn object_with_type_map(&self, type_map: &RdbcTypeMap) -> Result<RdbcObject, DruidError> {
        self.physical.object_with_type_map(type_map)
    }

    /// Snake_case getter corresponding to Java `Ref#getObject(Map)`.
    pub fn get_object_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, DruidError> {
        self.object_with_type_map(type_map)
    }

    /// Replaces the referenced object. Corresponds to Java `Ref#setObject(Object)`.
    pub fn set_object(&self, value: RdbcObject) -> Result<(), DruidError> {
        self.physical.set_object(value)
    }

    /// Returns the physical `Ref` SPI.
    pub fn physical(&self) -> &dyn PhysicalRef {
        self.physical.as_ref()
    }
}

impl fmt::Debug for RdbcRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcRef")
            .field("physical", &self.physical)
            .finish()
    }
}

impl PartialEq for RdbcRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcRef {}
