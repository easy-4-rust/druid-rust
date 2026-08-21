use super::{
    RdbcArrayAccess, RdbcBlobAccess, RdbcClobAccess, RdbcNClobAccess, RdbcRefAccess,
    RdbcResourceAccess, RdbcResourceContext, RdbcResourceKind, RdbcSqlXmlAccess,
};
use crate::core::DruidError;
use crate::sql::{RdbcArray, RdbcBlob, RdbcClob, RdbcNClob, RdbcRef, RdbcSqlXml};
use std::sync::Arc;

/// Creates public RDBC handles without exposing their driver access implementations.
#[derive(Clone, Copy, Debug, Default)]
pub struct RdbcResourceFactory;

impl RdbcResourceFactory {
    /// Creates a detached `Array` handle.
    #[must_use]
    pub fn array(access: Arc<dyn RdbcArrayAccess>) -> RdbcArray {
        let context = Self::detached_context(RdbcResourceKind::Array, access.as_ref());
        RdbcArray::from_parts(access, context)
    }

    /// Creates a connection-bound `Array` handle.
    pub fn array_with_context(
        context: Arc<RdbcResourceContext>,
        access: Arc<dyn RdbcArrayAccess>,
    ) -> Result<RdbcArray, DruidError> {
        Self::validate_context(&context, RdbcResourceKind::Array, access.as_ref())?;
        Ok(RdbcArray::from_parts(access, context))
    }

    /// Creates a detached `Blob` handle.
    #[must_use]
    pub fn blob(access: Arc<dyn RdbcBlobAccess>) -> RdbcBlob {
        let context = Self::detached_context(RdbcResourceKind::Blob, access.as_ref());
        RdbcBlob::from_parts(access, context)
    }

    /// Creates a connection-bound `Blob` handle.
    pub fn blob_with_context(
        context: Arc<RdbcResourceContext>,
        access: Arc<dyn RdbcBlobAccess>,
    ) -> Result<RdbcBlob, DruidError> {
        Self::validate_context(&context, RdbcResourceKind::Blob, access.as_ref())?;
        Ok(RdbcBlob::from_parts(access, context))
    }

    /// Creates a detached `Clob` handle.
    #[must_use]
    pub fn clob(access: Arc<dyn RdbcClobAccess>) -> RdbcClob {
        let context = Self::detached_context(RdbcResourceKind::Clob, access.as_ref());
        RdbcClob::from_parts(access, context)
    }

    /// Creates a connection-bound `Clob` handle.
    pub fn clob_with_context(
        context: Arc<RdbcResourceContext>,
        access: Arc<dyn RdbcClobAccess>,
    ) -> Result<RdbcClob, DruidError> {
        Self::validate_context(&context, RdbcResourceKind::Clob, access.as_ref())?;
        Ok(RdbcClob::from_parts(access, context))
    }

    /// Creates a detached `NClob` handle.
    #[must_use]
    pub fn n_clob(access: Arc<dyn RdbcNClobAccess>) -> RdbcNClob {
        let context = Self::detached_context(RdbcResourceKind::NClob, access.as_ref());
        RdbcNClob::from_parts(access, context)
    }

    /// Creates a connection-bound `NClob` handle.
    pub fn n_clob_with_context(
        context: Arc<RdbcResourceContext>,
        access: Arc<dyn RdbcNClobAccess>,
    ) -> Result<RdbcNClob, DruidError> {
        Self::validate_context(&context, RdbcResourceKind::NClob, access.as_ref())?;
        Ok(RdbcNClob::from_parts(access, context))
    }

    /// Creates a detached `Ref` handle.
    #[must_use]
    pub fn reference(access: Arc<dyn RdbcRefAccess>) -> RdbcRef {
        let context = Self::detached_context(RdbcResourceKind::Ref, access.as_ref());
        RdbcRef::from_parts(access, context)
    }

    /// Creates a connection-bound `Ref` handle.
    pub fn reference_with_context(
        context: Arc<RdbcResourceContext>,
        access: Arc<dyn RdbcRefAccess>,
    ) -> Result<RdbcRef, DruidError> {
        Self::validate_context(&context, RdbcResourceKind::Ref, access.as_ref())?;
        Ok(RdbcRef::from_parts(access, context))
    }

    /// Creates a detached `SQLXML` handle.
    #[must_use]
    pub fn sql_xml(access: Arc<dyn RdbcSqlXmlAccess>) -> RdbcSqlXml {
        let context = Self::detached_context(RdbcResourceKind::SqlXml, access.as_ref());
        RdbcSqlXml::from_parts(access, context)
    }

    /// Creates a connection-bound `SQLXML` handle.
    pub fn sql_xml_with_context(
        context: Arc<RdbcResourceContext>,
        access: Arc<dyn RdbcSqlXmlAccess>,
    ) -> Result<RdbcSqlXml, DruidError> {
        Self::validate_context(&context, RdbcResourceKind::SqlXml, access.as_ref())?;
        Ok(RdbcSqlXml::from_parts(access, context))
    }

    fn detached_context<A>(kind: RdbcResourceKind, access: &A) -> Arc<RdbcResourceContext>
    where
        A: RdbcResourceAccess + ?Sized,
    {
        Arc::new(RdbcResourceContext::detached(kind, access.capabilities()))
    }

    fn validate_context<A>(
        context: &RdbcResourceContext,
        expected_kind: RdbcResourceKind,
        access: &A,
    ) -> Result<(), DruidError>
    where
        A: RdbcResourceAccess + ?Sized,
    {
        if context.kind() != expected_kind {
            return Err(DruidError::InvalidArgument(format!(
                "{} resource context cannot create a {} handle",
                context.kind().standard_name(),
                expected_kind.standard_name()
            )));
        }
        if !access.capabilities().contains(context.capabilities()) {
            return Err(DruidError::InvalidArgument(format!(
                "{} resource context enables operations that its access implementation does not support",
                expected_kind.standard_name()
            )));
        }
        context.ensure_open()
    }
}
