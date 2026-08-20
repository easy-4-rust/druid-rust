//! Standard Rust mapping for Java `java.sql.Array`.

use crate::core::{DruidError, RdbcObject, RdbcResultSet, RdbcTypeMap};
use crate::spi::{
    RdbcArrayAccess, RdbcResourceCapabilities, RdbcResourceContext, RdbcResourceId,
    RdbcResourceState,
};
use std::fmt;
use std::sync::Arc;

/// Driver-neutral RDBC `Array` handle.
///
/// The handle preserves the SQL base type, type-map conversion, range, result-set, and release
/// semantics defined by `java.sql.Array`. Clones share identity and lifecycle state.
#[derive(Clone)]
pub struct RdbcArray {
    access: Arc<dyn RdbcArrayAccess>,
    context: Arc<RdbcResourceContext>,
}

impl RdbcArray {
    pub(crate) fn from_parts(
        access: Arc<dyn RdbcArrayAccess>,
        context: Arc<RdbcResourceContext>,
    ) -> Self {
        Self { access, context }
    }

    /// Returns the SQL type name of array elements. Corresponds to `Array#getBaseTypeName()`.
    pub async fn base_type_name(&self) -> Result<String, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "Array#getBaseTypeName")?;
        self.context.observe(self.access.base_type_name().await)
    }

    /// Returns the SQL type name of array elements. Corresponds to `Array#getBaseTypeName()`.
    pub async fn get_base_type_name(&self) -> Result<String, DruidError> {
        self.base_type_name().await
    }

    /// Returns the `java.sql.Types` number of array elements. Corresponds to `Array#getBaseType()`.
    pub async fn base_type(&self) -> Result<i32, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "Array#getBaseType")?;
        self.context.observe(self.access.base_type().await)
    }

    /// Returns the `java.sql.Types` number of array elements. Corresponds to `Array#getBaseType()`.
    pub async fn get_base_type(&self) -> Result<i32, DruidError> {
        self.base_type().await
    }

    /// Reads all elements using the driver's default type map. Corresponds to `Array#getArray()`.
    pub async fn values(&self) -> Result<Vec<RdbcObject>, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "Array#getArray")?;
        self.context.observe(self.access.values().await)
    }

    /// Reads all elements using the driver's default type map. Corresponds to `Array#getArray()`.
    pub async fn get_array(&self) -> Result<Vec<RdbcObject>, DruidError> {
        self.values().await
    }

    /// Reads all elements using an explicit type map. Corresponds to `Array#getArray(Map)`.
    pub async fn values_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::TYPE_MAP,
            "Array#getArray(Map)",
        )?;
        self.context
            .observe(self.access.values_with_type_map(type_map).await)
    }

    /// Reads a 1-based range. Corresponds to `Array#getArray(long,int)`.
    pub async fn values_range(
        &self,
        index: i64,
        count: i32,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::RANGE,
            "Array#getArray(long,int)",
        )?;
        self.context
            .observe(self.access.values_range(index, count).await)
    }

    /// Reads a 1-based range with an explicit type map.
    pub async fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ
                | RdbcResourceCapabilities::RANGE
                | RdbcResourceCapabilities::TYPE_MAP,
            "Array#getArray(long,int,Map)",
        )?;
        self.context.observe(
            self.access
                .values_range_with_type_map(index, count, type_map)
                .await,
        )
    }

    /// Returns all elements as a result set. Corresponds to `Array#getResultSet()`.
    pub async fn result_set(&self) -> Result<RdbcResultSet, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::RESULT_SET,
            "Array#getResultSet",
        )?;
        self.context.observe(self.access.result_set().await)
    }

    /// Returns all elements as a result set using an explicit type map.
    pub async fn result_set_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ
                | RdbcResourceCapabilities::RESULT_SET
                | RdbcResourceCapabilities::TYPE_MAP,
            "Array#getResultSet(Map)",
        )?;
        self.context
            .observe(self.access.result_set_with_type_map(type_map).await)
    }

    /// Returns a 1-based range as a result set.
    pub async fn result_set_range(
        &self,
        index: i64,
        count: i32,
    ) -> Result<RdbcResultSet, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ
                | RdbcResourceCapabilities::RANGE
                | RdbcResourceCapabilities::RESULT_SET,
            "Array#getResultSet(long,int)",
        )?;
        self.context
            .observe(self.access.result_set_range(index, count).await)
    }

    /// Returns a 1-based range as a result set using an explicit type map.
    pub async fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ
                | RdbcResourceCapabilities::RANGE
                | RdbcResourceCapabilities::RESULT_SET
                | RdbcResourceCapabilities::TYPE_MAP,
            "Array#getResultSet(long,int,Map)",
        )?;
        self.context.observe(
            self.access
                .result_set_range_with_type_map(index, count, type_map)
                .await,
        )
    }

    /// Releases the array resource. Corresponds to `Array#free()`.
    pub async fn free(&self) -> Result<(), DruidError> {
        self.context
            .require_capability(RdbcResourceCapabilities::FREE, "Array#free")?;
        self.context.free(self.access.as_ref()).await
    }

    /// Returns whether the array was released.
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.context.is_freed()
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

    /// Returns the operations enabled for this array instance.
    #[must_use]
    pub fn capabilities(&self) -> RdbcResourceCapabilities {
        self.context.capabilities()
    }
}

impl fmt::Debug for RdbcArray {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcArray")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RdbcArray {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.context, &other.context)
    }
}

impl Eq for RdbcArray {}

/// Standard Rust mapping for an SQL `ARRAY` value.
pub type Array = RdbcArray;
