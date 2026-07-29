//! JDBC `Ref` 平台资源。
//!
//! 对应 Java 平台对象：`java.sql.Ref`。

use super::{DruidError, JdbcObject, JdbcTypeMap};
use std::fmt;
use std::sync::Arc;

/// 物理 JDBC `Ref` SPI，覆盖 `java.sql.Ref` 的四个操作。
pub trait PhysicalRef: fmt::Debug + Send + Sync {
    /// 返回引用所指 SQL structured type 的完全限定名称。
    fn base_type_name(&self) -> Result<String, DruidError>;

    /// 使用驱动默认类型映射读取引用对象。
    fn object(&self) -> Result<JdbcObject, DruidError>;

    /// 使用显式类型映射读取引用对象。
    fn object_with_type_map(&self, type_map: &JdbcTypeMap) -> Result<JdbcObject, DruidError>;

    /// 替换引用所指对象。
    fn set_object(&self, value: JdbcObject) -> Result<(), DruidError>;
}

/// 不泄漏具体驱动类型的 JDBC `Ref` 句柄。
#[derive(Clone)]
pub struct JdbcRef {
    physical: Arc<dyn PhysicalRef>,
}

impl JdbcRef {
    /// 包装物理 `Ref`。
    pub fn new(physical: Arc<dyn PhysicalRef>) -> Self {
        Self { physical }
    }

    /// 对应 Java `Ref#getBaseTypeName()`。
    pub fn base_type_name(&self) -> Result<String, DruidError> {
        self.physical.base_type_name()
    }

    /// 对应 Java `Ref#getObject()`。
    pub fn object(&self) -> Result<JdbcObject, DruidError> {
        self.physical.object()
    }

    /// 对应 Java `Ref#getObject(Map)`。
    pub fn object_with_type_map(&self, type_map: &JdbcTypeMap) -> Result<JdbcObject, DruidError> {
        self.physical.object_with_type_map(type_map)
    }

    /// 对应 Java `Ref#setObject(Object)`。
    pub fn set_object(&self, value: JdbcObject) -> Result<(), DruidError> {
        self.physical.set_object(value)
    }

    /// 返回物理 `Ref` SPI。
    pub fn physical(&self) -> &dyn PhysicalRef {
        self.physical.as_ref()
    }
}

impl fmt::Debug for JdbcRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcRef")
            .field("physical", &self.physical)
            .finish()
    }
}

impl PartialEq for JdbcRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcRef {}
