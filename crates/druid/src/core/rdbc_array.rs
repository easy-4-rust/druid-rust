//! RDBC `Array` platform resource.
//!
//! Corresponds to Java: `java.sql.Array`.

use super::{DruidError, RdbcObject, RdbcResultSet, RdbcTypeMap};
use std::fmt;
use std::sync::Arc;

/// Physical RDBC `Array` SPI for values, ranges, type maps, result sets, and release.
pub trait PhysicalArray: fmt::Debug + Send + Sync {
    /// Returns the SQL type name of array elements.
    fn base_type_name(&self) -> Result<String, DruidError>;

    /// Returns the `java.sql.Types` number of array elements.
    fn base_type(&self) -> Result<i32, DruidError>;

    /// Reads all elements with the default type map.
    fn values(&self) -> Result<Vec<RdbcObject>, DruidError>;

    /// Reads all elements with an explicit type map.
    fn values_with_type_map(&self, type_map: &RdbcTypeMap) -> Result<Vec<RdbcObject>, DruidError>;

    /// Reads `count` elements from a 1-based index.
    fn values_range(&self, index: i64, count: i32) -> Result<Vec<RdbcObject>, DruidError>;

    /// Reads a range with an explicit type map.
    fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError>;

    /// Returns all elements as a result set using the default type map.
    fn result_set(&self) -> Result<RdbcResultSet, DruidError>;

    /// Returns all elements as a result set using an explicit type map.
    fn result_set_with_type_map(&self, type_map: &RdbcTypeMap)
        -> Result<RdbcResultSet, DruidError>;

    /// Returns a range as a result set.
    fn result_set_range(&self, index: i64, count: i32) -> Result<RdbcResultSet, DruidError>;

    /// Returns a range as a result set using an explicit type map.
    fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError>;

    /// Releases the array resource.
    fn free(&self) -> Result<(), DruidError>;

    /// Returns whether the array has been released.
    fn is_freed(&self) -> bool;
}

/// Driver-neutral RDBC `Array` handle.
#[derive(Clone)]
pub struct RdbcArray {
    physical: Arc<dyn PhysicalArray>,
}

impl RdbcArray {
    /// Wraps a physical array.
    pub fn new(physical: Arc<dyn PhysicalArray>) -> Self {
        Self { physical }
    }

    /// Returns the SQL type name of array elements.
    pub fn base_type_name(&self) -> Result<String, DruidError> {
        self.physical.base_type_name()
    }

    /// Snake_case getter corresponding to Java `Array#getBaseTypeName()`.
    pub fn get_base_type_name(&self) -> Result<String, DruidError> {
        self.base_type_name()
    }

    /// Returns the SQL type number of array elements.
    pub fn base_type(&self) -> Result<i32, DruidError> {
        self.physical.base_type()
    }

    /// Snake_case getter corresponding to Java `Array#getBaseType()`.
    pub fn get_base_type(&self) -> Result<i32, DruidError> {
        self.base_type()
    }

    /// Reads all elements.
    pub fn values(&self) -> Result<Vec<RdbcObject>, DruidError> {
        self.physical.values()
    }

    /// Snake_case getter corresponding to Java `Array#getArray()`.
    pub fn get_array(&self) -> Result<Vec<RdbcObject>, DruidError> {
        self.values()
    }

    /// Reads all elements with an explicit type map.
    pub fn values_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        self.physical.values_with_type_map(type_map)
    }

    /// Reads a specified range.
    pub fn values_range(&self, index: i64, count: i32) -> Result<Vec<RdbcObject>, DruidError> {
        self.physical.values_range(index, count)
    }

    /// Reads a specified range with an explicit type map.
    pub fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        self.physical
            .values_range_with_type_map(index, count, type_map)
    }

    /// Returns all elements as a result set.
    pub fn result_set(&self) -> Result<RdbcResultSet, DruidError> {
        self.physical.result_set()
    }

    /// Returns all elements as a result set with an explicit type map.
    pub fn result_set_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        self.physical.result_set_with_type_map(type_map)
    }

    /// Returns a specified range as a result set.
    pub fn result_set_range(&self, index: i64, count: i32) -> Result<RdbcResultSet, DruidError> {
        self.physical.result_set_range(index, count)
    }

    /// Returns a specified range as a result set with an explicit type map.
    pub fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        self.physical
            .result_set_range_with_type_map(index, count, type_map)
    }

    /// Releases the array.
    pub fn free(&self) -> Result<(), DruidError> {
        self.physical.free()
    }

    /// Returns whether the array has been released.
    pub fn is_freed(&self) -> bool {
        self.physical.is_freed()
    }

    /// Returns the physical array SPI.
    pub fn physical(&self) -> &dyn PhysicalArray {
        self.physical.as_ref()
    }
}

impl fmt::Debug for RdbcArray {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcArray")
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for RdbcArray {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcArray {}
