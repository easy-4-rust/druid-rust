use crate::error::RdbcError;
use crate::resource::{
    Array, RdbcArray, Blob, RdbcBlob, RdbcClob, Clob, RdbcNClob,
    NClob, RdbcRef, Ref, RdbcStruct, Struct, RdbcResourceAccess, RdbcSqlXml, RdbcSqlXmlAccess,
    ResourceLifecycle, ResourceType,
};
use std::sync::Arc;

/// Creates public RDBC handles without exposing their driver access implementations.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResourceHandleFactory;

#[allow(dead_code)] // Context-bearing constructors are reserved for Task 7 standard resources.
impl ResourceHandleFactory {
    /// 将唯一标准Array对象装入共享值句柄，参数为独立拥有的驱动对象，返回保留身份及释放状态的句柄。
    /// 对应 Java: java.sql.Array；范围、映射和游标直接委托，不增加驱动端口。
    #[must_use]
    pub fn array(value: Box<dyn Array>) -> RdbcArray {
        RdbcArray::from_standard(value)
    }

    /// 将唯一标准Blob对象装入共享值句柄，参数为独立拥有的驱动对象，返回保留身份与释放状态的句柄。
    /// 对应 Java: java.sql.Blob；不提前读取内容，不增加驱动端口。
    #[must_use]
    pub fn blob(value: Box<dyn Blob>) -> RdbcBlob {
        RdbcBlob::from_standard(value)
    }

    /// 接管参数value唯一标准Clob，返回保留身份及释放状态的共享句柄。对应 Java: java.sql.Clob。
    #[must_use] pub fn clob(value:Box<dyn Clob>)->RdbcClob {RdbcClob::from_standard(value)}
    /// 接管参数value唯一标准NClob，返回保留国家字符身份及共享Clob视图的句柄。对应 Java: java.sql.NClob。
    #[must_use] pub fn n_clob(value:Box<dyn NClob>)->RdbcNClob {RdbcNClob::from_standard(value)}

    /// 将独立拥有的标准Ref装入共享值句柄，参数为唯一驱动对象，返回保持引用身份的句柄。
    /// 对应 Java: java.sql.Ref；不物化属性，不新增free或连接创建接口。
    #[must_use]
    pub fn reference(value: Box<dyn Ref>) -> RdbcRef {
        RdbcRef::from_standard(value)
    }

    /// 将独立拥有的标准Struct装入共享值句柄，参数为唯一驱动对象，返回延迟读取属性的句柄。
    /// 对应 Java: java.sql.Struct；用于getObject及嵌套属性，不新增驱动端口。
    #[must_use]
    pub fn structure(value: Box<dyn Struct>) -> RdbcStruct {
        RdbcStruct::from_standard(value)
    }

    /// Creates a detached `SQLXML` handle.
    #[must_use]
    pub fn sql_xml(access: Arc<dyn RdbcSqlXmlAccess>) -> RdbcSqlXml {
        let context = Self::detached_context(ResourceType::SqlXml, access.as_ref());
        RdbcSqlXml::from_parts(access, context)
    }

    /// Creates a connection-bound `SQLXML` handle.
    pub(crate) fn sql_xml_with_context(
        context: Arc<ResourceLifecycle>,
        access: Arc<dyn RdbcSqlXmlAccess>,
    ) -> Result<RdbcSqlXml, RdbcError> {
        Self::validate_context(&context, ResourceType::SqlXml, access.as_ref())?;
        Ok(RdbcSqlXml::from_parts(access, context))
    }

    fn detached_context<A>(kind: ResourceType, access: &A) -> Arc<ResourceLifecycle>
    where
        A: RdbcResourceAccess + ?Sized,
    {
        Arc::new(ResourceLifecycle::detached(kind, access.capabilities()))
    }

    fn validate_context<A>(
        context: &ResourceLifecycle,
        expected_kind: ResourceType,
        access: &A,
    ) -> Result<(), RdbcError>
    where
        A: RdbcResourceAccess + ?Sized,
    {
        if context.kind() != expected_kind {
            return Err(RdbcError::InvalidArgument(format!(
                "{} resource context cannot create a {} handle",
                context.kind().standard_name(),
                expected_kind.standard_name()
            )));
        }
        if !access.capabilities().contains(context.capabilities()) {
            return Err(RdbcError::InvalidArgument(format!(
                "{} resource context enables operations that its access implementation does not support",
                expected_kind.standard_name()
            )));
        }
        context.ensure_open()
    }
}
