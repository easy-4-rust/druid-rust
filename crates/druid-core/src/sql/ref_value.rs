//! Standard Rust mapping for Java `java.sql.Ref`.

use crate::core::{DruidError, RdbcObject, RdbcTypeMap};
use crate::spi::{
    RdbcRefAccess, RdbcResourceCapabilities, RdbcResourceContext, RdbcResourceId, RdbcResourceState,
};
use std::fmt;
use std::sync::Arc;

/// Driver-neutral RDBC `Ref` handle.
#[derive(Clone)]
pub struct RdbcRef {
    access: Arc<dyn RdbcRefAccess>,
    context: Arc<RdbcResourceContext>,
}

impl RdbcRef {
    pub(crate) fn from_parts(
        access: Arc<dyn RdbcRefAccess>,
        context: Arc<RdbcResourceContext>,
    ) -> Self {
        Self { access, context }
    }

    /// Returns the fully qualified SQL type name. Corresponds to `Ref#getBaseTypeName()`.
    pub async fn base_type_name(&self) -> Result<String, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "Ref#getBaseTypeName")?;
        self.context.observe(self.access.base_type_name().await)
    }

    /// Returns the fully qualified SQL type name. Corresponds to `Ref#getBaseTypeName()`.
    pub async fn get_base_type_name(&self) -> Result<String, DruidError> {
        self.base_type_name().await
    }

    /// Reads the referenced object using the driver's default type map.
    pub async fn object(&self) -> Result<RdbcObject, DruidError> {
        self.context
            .require(RdbcResourceCapabilities::READ, "Ref#getObject")?;
        self.context.observe(self.access.object().await)
    }

    /// Reads the referenced object using the driver's default type map.
    pub async fn get_object(&self) -> Result<RdbcObject, DruidError> {
        self.object().await
    }

    /// Reads the referenced object using an explicit type map.
    pub async fn object_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, DruidError> {
        self.context.require(
            RdbcResourceCapabilities::READ | RdbcResourceCapabilities::TYPE_MAP,
            "Ref#getObject(Map)",
        )?;
        self.context
            .observe(self.access.object_with_type_map(type_map).await)
    }

    /// Reads the referenced object using an explicit type map.
    pub async fn get_object_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, DruidError> {
        self.object_with_type_map(type_map).await
    }

    /// Replaces the referenced object. Corresponds to `Ref#setObject(Object)`.
    pub async fn set_object(&self, value: RdbcObject) -> Result<(), DruidError> {
        self.context
            .require(RdbcResourceCapabilities::WRITE, "Ref#setObject")?;
        self.context.observe(self.access.set_object(value).await)
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

    /// Returns the operations enabled for this Ref instance.
    #[must_use]
    pub fn capabilities(&self) -> RdbcResourceCapabilities {
        self.context.capabilities()
    }
}

impl fmt::Debug for RdbcRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcRef")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RdbcRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.context, &other.context)
    }
}

impl Eq for RdbcRef {}

/// Mapping of an SQL `REF` that refers to a structured value in the database.
pub type Ref = RdbcRef;
