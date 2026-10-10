use crate::resource::RdbcInputStream;
use crate::types::{OpaqueObjectValue, RdbcObject, RdbcOpaqueObject};
use std::any::Any;
use std::sync::Arc;

/// SQLInput/SQLOutput内部属性，保留字节流身份而不复制流内容。对应 Java: SQLInput/SQLOutput的属性流。
#[derive(Clone, Debug)]
pub(crate) enum SqlAttribute {
    Object(RdbcObject),
    AsciiStream(RdbcInputStream),
    BinaryStream(RdbcInputStream),
}
impl SqlAttribute {
    /// 从标准对象恢复内部流标签，返回同一原始句柄。
    pub(crate) fn from_object(value: RdbcObject) -> Self {
        if let RdbcObject::Custom(object) = &value {
            if let Some(attribute) = object.downcast_ref::<Self>() {
                return attribute.clone();
            }
        }
        Self::Object(value)
    }
    /// 返回是否为SQL NULL，不消费属性。
    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Self::Object(value) if value.is_null())
    }
    /// 转为标准Object表示，流通过原有opaque平台合同保留引用而不引入第二资源树。
    pub(crate) fn into_object(self) -> RdbcObject {
        match self {
            Self::Object(value) => value,
            stream => RdbcObject::Custom(RdbcOpaqueObject::new(Arc::new(stream))),
        }
    }
}
impl OpaqueObjectValue for SqlAttribute {
    fn class_name(&self) -> &str {
        match self {
            Self::Object(_) => "java.lang.Object",
            Self::AsciiStream(_) | Self::BinaryStream(_) => "java.io.InputStream",
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
