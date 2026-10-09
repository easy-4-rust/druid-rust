//! Standard Rust mapping for Java `java.sql.Ref`.

use crate::error::RdbcError;
use crate::resource::{RdbcRefAccess, ResourceLifecycle, ResourceOperations};
use crate::types::{RdbcObject, RdbcTypeMap};
use std::fmt;
use std::sync::Arc;

/// Driver-neutral RDBC `Ref` handle.
#[derive(Clone)]
pub struct RdbcRef {
    access: Arc<dyn RdbcRefAccess>,
    context: Arc<ResourceLifecycle>,
}

impl RdbcRef {
    pub(crate) fn from_parts(
        access: Arc<dyn RdbcRefAccess>,
        context: Arc<ResourceLifecycle>,
    ) -> Self {
        Self { access, context }
    }

    /// Returns the fully qualified SQL type name. Corresponds to `Ref#getBaseTypeName()`.
    pub async fn base_type_name(&self) -> Result<String, RdbcError> {
        self.context
            .require(ResourceOperations::READ, "Ref#getBaseTypeName")?;
        self.context.observe(self.access.base_type_name().await)
    }

    /// Returns the fully qualified SQL type name. Corresponds to `Ref#getBaseTypeName()`.
    pub async fn get_base_type_name(&self) -> Result<String, RdbcError> {
        self.base_type_name().await
    }

    /// Reads the referenced object using the driver's default type map.
    pub async fn object(&self) -> Result<RdbcObject, RdbcError> {
        self.context
            .require(ResourceOperations::READ, "Ref#getObject")?;
        self.context.observe(self.access.object().await)
    }

    /// Reads the referenced object using the driver's default type map.
    pub async fn get_object(&self) -> Result<RdbcObject, RdbcError> {
        self.object().await
    }

    /// Reads the referenced object using an explicit type map.
    pub async fn object_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, RdbcError> {
        self.context.require(
            ResourceOperations::READ | ResourceOperations::TYPE_MAP,
            "Ref#getObject(Map)",
        )?;
        self.context
            .observe(self.access.object_with_type_map(type_map).await)
    }

    /// Reads the referenced object using an explicit type map.
    pub async fn get_object_with_type_map(
        &self,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, RdbcError> {
        self.object_with_type_map(type_map).await
    }

    /// Replaces the referenced object. Corresponds to `Ref#setObject(Object)`.
    pub async fn set_object(&self, value: RdbcObject) -> Result<(), RdbcError> {
        self.context
            .require(ResourceOperations::WRITE, "Ref#setObject")?;
        self.context.observe(self.access.set_object(value).await)
    }

    /// Returns the driver-neutral resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &str {
        self.context.resource_id().as_str()
    }

    /// Returns the operations enabled for this Ref instance.
    #[must_use]
    pub fn capabilities(&self) -> ResourceOperations {
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
